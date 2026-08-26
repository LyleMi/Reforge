use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use reforge_schema::{
    ANALYSIS_CODEBASE, ANALYSIS_DATAFLOW, AnalysisCoverage, CoverageLimitation, CoverageStatus,
    Evidence, Issue, LanguageCoverage, Location, Measurement, Producer, Report, RuleExecution,
    Subject, SuppressionSummary, Target,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Analysis {
    Codebase,
    Dataflow,
}

impl Analysis {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Codebase => ANALYSIS_CODEBASE,
            Self::Dataflow => ANALYSIS_DATAFLOW,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ScopeConfig {
    include_hidden: bool,
    include_generated: bool,
    no_gitignore: bool,
    exclude_tests: bool,
    ignore_paths: Vec<String>,
}

/// Validated public configuration for one analyzer request.
#[derive(Debug, Clone)]
pub struct Config {
    engine: crate::scan::config::ConfigFile,
    enabled: BTreeSet<Analysis>,
    scope: ScopeConfig,
    rules: RuleSelection,
    source: toml::Value,
}

#[derive(Debug, Clone, Default)]
struct RuleSelection {
    enabled: BTreeSet<String>,
    disabled: BTreeSet<String>,
    enforced: BTreeSet<String>,
}

impl Config {
    pub const DEFAULT_TOML: &'static str = r#"version = 2

[analysis]
enabled = ["codebase"]

[scope]
include-hidden = false
include-generated = false
no-gitignore = false
exclude-tests = false
ignore-paths = []

[rules]
enable = []
disable = []
enforce = []

[codebase]
preset = "balanced"
churn = "auto"
churn-window-days = 90
max-file-lines = 600

[dataflow.search]
max-path-steps = 24
max-function-hops = 8
max-module-hops = 8
max-paths-per-source = 100
max-sinks-per-source = 100
work-budget = 100000

[dataflow.relay]
min-function-hops = 4
min-module-hops = 2
min-relay-percent = 90

[dataflow.fan-out]
min-sinks = 4
min-modules = 3
"#;

    pub fn parse_toml(input: &str) -> Result<Self> {
        let value: toml::Value =
            toml::from_str(input).context("failed to parse Reforge engine configuration")?;
        let table = value
            .as_table()
            .context("reforge.toml root must be a table")?;
        let version = table
            .get("version")
            .and_then(toml::Value::as_integer)
            .context("reforge.toml must declare `version = 2`")?;
        if version != 2 {
            bail!("unsupported reforge.toml version {version}; expected version 2");
        }
        validate_public_keys(&value)?;
        validate_public_values(&value)?;
        if value_at(&value, "analysis.lenses").is_some() {
            bail!("`analysis.lenses` was removed; use `analysis.enabled`");
        }
        let enabled = parse_enabled(value_at(&value, "analysis.enabled"))?;
        let scope = parse_scope(&value)?;
        validate_suppressions(&value)?;
        let rules = parse_rule_selection(&value)?;
        Ok(Self {
            engine: crate::scan::config::parse_config_value(&value)?,
            enabled,
            scope,
            rules,
            source: value,
        })
    }

    pub fn defaults() -> Self {
        Self::parse_toml(Self::DEFAULT_TOML).expect("built-in engine configuration must be valid")
    }

    pub fn enabled(&self) -> &BTreeSet<Analysis> {
        &self.enabled
    }

    pub fn set_enabled(&mut self, enabled: BTreeSet<Analysis>) -> Result<()> {
        if enabled.is_empty() {
            bail!("configuration must enable at least one analysis");
        }
        self.enabled = enabled;
        Ok(())
    }

    pub fn apply_scope_overrides(
        &mut self,
        include_hidden: bool,
        include_generated: bool,
        no_gitignore: bool,
        exclude_tests: bool,
        ignore_paths: &[String],
    ) {
        self.scope.include_hidden |= include_hidden;
        self.scope.include_generated |= include_generated;
        self.scope.no_gitignore |= no_gitignore;
        self.scope.exclude_tests |= exclude_tests;
        if !ignore_paths.is_empty() {
            self.scope.ignore_paths = ignore_paths.to_vec();
        }
    }

    fn rule_enabled(&self, rule: &str, default_enabled: bool) -> bool {
        !self.rules.disabled.contains(rule)
            && (self.rules.enabled.contains(rule)
                || self.rules.enforced.contains(rule)
                || default_enabled)
    }

    fn rule_enforced(&self, rule: &str) -> bool {
        self.rules.enforced.contains(rule)
    }
}

fn parse_rule_selection(value: &toml::Value) -> Result<RuleSelection> {
    let registered = crate::detectors::manifest::rule_registry()
        .iter()
        .map(|entry| (entry.rule.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let enabled = parse_rule_set(value, "rules.enable", &registered)?;
    let disabled = parse_rule_set(value, "rules.disable", &registered)?;
    let enforced = parse_rule_set(value, "rules.enforce", &registered)?;
    validate_disjoint_rule_sets(&enabled, &disabled, &enforced)?;
    for rule in &enforced {
        let entry = registered
            .get(rule.as_str())
            .expect("enforced rule was validated against registry");
        if entry.maturity != crate::model::RuleMaturity::Stable {
            bail!("rules.enforce only accepts stable rules; `{rule}` is preview");
        }
    }
    Ok(RuleSelection {
        enabled,
        disabled,
        enforced,
    })
}

fn parse_rule_set(
    value: &toml::Value,
    key: &str,
    registered: &BTreeMap<&str, &crate::model::RuleSpec>,
) -> Result<BTreeSet<String>> {
    let Some(values) = value_at(value, key) else {
        return Ok(BTreeSet::new());
    };
    let values = values
        .as_array()
        .with_context(|| format!("{key} must be an array"))?;
    let mut parsed = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let rule = value
            .as_str()
            .with_context(|| format!("{key}[{index}] must be a string"))?;
        if !rule.starts_with("reforge.") || rule.matches('.').count() < 2 {
            bail!("{key}[{index}] must use a complete rule ID");
        }
        if !registered.contains_key(rule) {
            bail!("unknown rule `{rule}` in {key}");
        }
        if !parsed.insert(rule.to_owned()) {
            bail!("duplicate rule `{rule}` in {key}");
        }
    }
    Ok(parsed)
}

fn validate_disjoint_rule_sets(
    enabled: &BTreeSet<String>,
    disabled: &BTreeSet<String>,
    enforced: &BTreeSet<String>,
) -> Result<()> {
    for (left, left_name, right, right_name) in [
        (enabled, "rules.enable", disabled, "rules.disable"),
        (enforced, "rules.enforce", disabled, "rules.disable"),
        (enabled, "rules.enable", enforced, "rules.enforce"),
    ] {
        if let Some(rule) = left.intersection(right).next() {
            bail!("rule `{rule}` appears in both {left_name} and {right_name}");
        }
    }
    Ok(())
}

fn validate_public_values(value: &toml::Value) -> Result<()> {
    validate_enum(value, "codebase.preset", &["strict", "balanced", "relaxed"])?;
    validate_enum(value, "codebase.churn", &["auto", "on", "off"])?;
    for key in [
        "codebase.max-file-lines",
        "codebase.max-dir-files",
        "codebase.min-similar-functions",
        "codebase.min-function-tokens",
        "codebase.max-function-lines",
        "codebase.max-function-complexity",
        "codebase.max-nesting-depth",
        "codebase.max-function-parameters",
        "codebase.max-type-lines",
        "codebase.max-type-members",
        "codebase.max-imports",
        "codebase.max-public-items",
        "codebase.max-functions-per-file",
        "codebase.max-functions-per-100-lines",
        "codebase.min-module-functions",
        "codebase.min-repeated-literal-occurrences",
        "codebase.min-data-clump-occurrences",
        "codebase.churn-window-days",
        "codebase.churn-max-commit-lines",
        "dataflow.search.max-function-hops",
        "dataflow.search.max-path-steps",
        "dataflow.search.max-module-hops",
        "dataflow.search.max-paths-per-source",
        "dataflow.search.max-sinks-per-source",
        "dataflow.search.work-budget",
        "dataflow.relay.min-function-hops",
        "dataflow.relay.min-module-hops",
        "dataflow.fan-out.min-sinks",
        "dataflow.fan-out.min-modules",
    ] {
        if let Some(candidate) = value_at(value, key)
            && candidate.as_integer().is_none_or(|number| number <= 0)
        {
            bail!("{key} must be a positive integer");
        }
    }
    validate_percentage(value, "codebase.max-small-function-ratio")?;
    validate_percentage(value, "codebase.min-clustered-function-percent")?;
    validate_percentage(value, "dataflow.relay.min-relay-percent")?;
    if let Some(candidate) = value_at(value, "codebase.function-similarity")
        && candidate
            .as_float()
            .or_else(|| candidate.as_integer().map(|number| number as f64))
            .is_none_or(|number| !(0.0..=1.0).contains(&number))
    {
        bail!("codebase.function-similarity must be between 0 and 1");
    }
    Ok(())
}

fn validate_enum(value: &toml::Value, key: &str, allowed: &[&str]) -> Result<()> {
    if let Some(candidate) = value_at(value, key)
        && candidate
            .as_str()
            .is_none_or(|candidate| !allowed.contains(&candidate))
    {
        bail!("{key} must be one of {}", allowed.join(", "));
    }
    Ok(())
}

fn validate_percentage(value: &toml::Value, key: &str) -> Result<()> {
    if let Some(candidate) = value_at(value, key)
        && candidate
            .as_integer()
            .is_none_or(|number| !(0..=100).contains(&number))
    {
        bail!("{key} must be an integer between 0 and 100");
    }
    Ok(())
}

fn value_at<'a>(root: &'a toml::Value, path: &str) -> Option<&'a toml::Value> {
    path.split('.').try_fold(root, |value, key| value.get(key))
}

mod config;
use config::{parse_enabled, parse_scope, validate_public_keys, validate_suppressions};

use crate::execution::{EffectiveConfig, ProgressMode};
use crate::model::{DetectedEvidence, IssueFamily, Rule, RunResult};
use crate::scan::NoopProgress;

mod report;
use report::build_report;

fn owner_selected(analyses: &BTreeSet<Analysis>, kind: Rule) -> bool {
    match crate::detectors::manifest::analysis_name(kind) {
        ANALYSIS_CODEBASE => analyses.contains(&Analysis::Codebase),
        ANALYSIS_DATAFLOW => analyses.contains(&Analysis::Dataflow),
        _ => false,
    }
}

#[derive(Debug, Clone)]
pub struct AnalyzeOptions {
    pub root: PathBuf,
    pub config: Config,
    pub reproducible: bool,
    pub metrics_output: Option<PathBuf>,
    pub flow_ir_output: Option<PathBuf>,
}

pub fn analyze(options: &AnalyzeOptions) -> Result<Report> {
    analyze_selected(options)
}

fn analyze_selected(options: &AnalyzeOptions) -> Result<Report> {
    let root = options
        .root
        .canonicalize()
        .with_context(|| format!("failed to resolve analysis root {}", options.root.display()))?;
    let mut args = EffectiveConfig::defaults_for_path(root.clone());
    args.reproducible = options.reproducible;
    args.progress = ProgressMode::Never;
    args.filters.include_hidden = options.config.scope.include_hidden;
    args.filters.include_generated = options.config.scope.include_generated;
    args.filters.no_gitignore = options.config.scope.no_gitignore;
    args.filters.exclude_tests = options.config.scope.exclude_tests;
    args.filters.ignore_paths = options.config.scope.ignore_paths.clone();
    let mut progress = NoopProgress;
    let scan_plan = crate::scan::ExecutionPlan {
        codebase: options.config.enabled.contains(&Analysis::Codebase),
        dataflow: options.config.enabled.contains(&Analysis::Dataflow),
        materialize_flow_ir: options.flow_ir_output.is_some(),
    };
    let run = crate::scan::run_with_plan_and_config(
        &args,
        &mut progress,
        scan_plan,
        &options.config.engine,
    )?;
    if let Some(path) = &options.metrics_output {
        write_debug_output(path, &run.raw_metrics)?;
    }
    if let Some(path) = &options.flow_ir_output {
        let program = run
            .flow_analysis
            .program
            .as_ref()
            .context("--flow-ir-output requires the dataflow analysis")?;
        write_debug_output(path, program)?;
    }
    Ok(build_report(run, &root, &options.config))
}

fn write_debug_output(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)
        .with_context(|| format!("failed to create debug output {}", path.display()))?;
    serde_json::to_writer_pretty(file, value)?;
    Ok(())
}

pub fn rules(analyses: &BTreeSet<Analysis>) -> Vec<serde_json::Value> {
    let mut entries = crate::detectors::manifest::rule_registry()
        .iter()
        .filter(|entry| owner_selected(analyses, entry.kind))
        .map(|entry| {
            serde_json::json!({
                "rule": entry.rule,
                "analysis": entry.analysis,
                "family": entry.family.qualified(&entry.analysis),
                "subjects": entry.allowed_subjects.iter().map(|subject| subject_name(*subject)).collect::<Vec<_>>(),
                "observation": {
                    "source": observation_source_name(entry.observation_source),
                    "unit": observation_unit(entry.observation_source),
                },
                "description": entry.description,
                "guidance": entry.family.guidance(),
                "maturity": entry.maturity,
                "semantic_version": entry.semantic_version,
                "validation_basis": entry.validation_basis,
                "default_enabled": entry.default_enabled,
                "enforceable": entry.maturity == crate::model::RuleMaturity::Stable,
                "languages": entry.language_capabilities,
                "measurements": entry.measurements.iter().map(|metric| metric.to_string()).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left["rule"].as_str().cmp(&right["rule"].as_str()));
    entries
}

fn subject_name(scope: crate::model::SubjectKind) -> &'static str {
    use crate::model::SubjectKind;
    match scope {
        SubjectKind::Repository => "repository",
        SubjectKind::Directory => "directory",
        SubjectKind::File => "file",
        SubjectKind::Symbol => "symbol",
        SubjectKind::Group => "group",
    }
}

fn observation_source_name(source: crate::model::ObservationSource) -> &'static str {
    use crate::model::ObservationSource as O;
    match source {
        O::Repositories => "repositories",
        O::Directories => "directories",
        O::Files => "files",
        O::Functions => "functions",
        O::Types => "types",
        O::FunctionPairs => "function_pairs",
        O::DependencyNodes => "dependency_nodes",
        O::DataflowSources => "dataflow_sources",
    }
}

fn observation_unit(source: crate::model::ObservationSource) -> &'static str {
    use crate::model::ObservationSource as O;
    match source {
        O::Repositories => "repository",
        O::Directories => "directory",
        O::Files => "file",
        O::Functions => "function",
        O::Types => "type",
        O::FunctionPairs => "function_pair",
        O::DependencyNodes => "dependency_node",
        O::DataflowSources => "dataflow_source",
    }
}

struct RuleDefinition {
    rule: String,
    family: String,
    issue_family: IssueFamily,
}

fn rule_definition(kind: Rule) -> RuleDefinition {
    let metadata = crate::detectors::manifest::rule_registry()
        .iter()
        .find(|entry| entry.kind == kind)
        .expect("every executable rule must have metadata");
    RuleDefinition {
        rule: metadata.rule.clone(),
        family: metadata.family.qualified(&metadata.analysis),
        issue_family: metadata.family,
    }
}

#[cfg(test)]
#[path = "api/tests.rs"]
mod tests;
