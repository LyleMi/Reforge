use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

use crate::model::{DetectedEvidence, Rule, SuppressionSummary, serialized_rule};

use super::config::ConfigSuppression;

const DIRECTIVE_PREFIX: &str = "reforge:";

#[derive(Debug, Default)]
pub(super) struct DetectionControlTelemetry {
    pub suppression_summary: SuppressionSummary,
    pub suppressed_by_kind: BTreeMap<Rule, usize>,
}

pub(super) fn apply_detection_controls(
    detections: &mut Vec<DetectedEvidence>,
    root: &Path,
    config_suppressions: &[ConfigSuppression],
) -> Result<DetectionControlTelemetry> {
    let suppressions = Suppressions::load(root, detections, config_suppressions)?;
    let mut telemetry = DetectionControlTelemetry::default();
    let mut retained = Vec::with_capacity(detections.len());

    for detection in detections.drain(..) {
        if suppressions.matches(&detection) {
            telemetry.suppression_summary.record(&detection);
            *telemetry
                .suppressed_by_kind
                .entry(detection.kind)
                .or_insert(0) += 1;
        } else {
            retained.push(detection);
        }
    }

    *detections = retained;
    Ok(telemetry)
}

#[derive(Debug, Default)]
struct Suppressions {
    root: String,
    rules: Vec<SuppressionRule>,
}

impl Suppressions {
    fn load(
        root: &Path,
        detections: &[DetectedEvidence],
        config_suppressions: &[ConfigSuppression],
    ) -> Result<Self> {
        let suppression_root = if root.is_file() {
            root.parent().unwrap_or(root)
        } else {
            root
        };
        let root = normalize_control_path(&display_path(suppression_root));
        let mut rules = Vec::new();

        for suppression in config_suppressions {
            let kinds = suppression
                .kind
                .as_deref()
                .map(parse_required_kind_list)
                .transpose()?;
            let reason = suppression.reason.trim();
            if reason.is_empty() {
                bail!(
                    "suppression for {} must include a non-empty reason",
                    suppression.path
                );
            }
            rules.push(SuppressionRule {
                path: normalize_control_path(&suppression.path),
                kinds,
                line: suppression.line,
                scope: SuppressionScope::Config,
            });
        }

        let mut inline_rules = InlineRuleLoader::new(root.clone());
        rules.extend(inline_rules.load(detections)?);

        Ok(Self { root, rules })
    }

    fn matches(&self, detection: &DetectedEvidence) -> bool {
        self.rules
            .iter()
            .any(|rule| rule.matches(detection, self.root.as_str()))
    }
}

#[derive(Debug)]
struct InlineRuleLoader {
    root: String,
    by_path: BTreeMap<String, Vec<SuppressionRule>>,
}

impl InlineRuleLoader {
    fn new(root: String) -> Self {
        Self {
            root,
            by_path: BTreeMap::new(),
        }
    }

    fn load(&mut self, detections: &[DetectedEvidence]) -> Result<Vec<SuppressionRule>> {
        for detection in detections {
            let path = normalize_control_path(&detection.path);
            if self.by_path.contains_key(&path) {
                continue;
            }

            let source_path = source_path_for_detection(&self.root, &path);
            if !source_path.is_file() {
                self.by_path.insert(path, Vec::new());
                continue;
            }

            let source = fs::read_to_string(&source_path).with_context(|| {
                format!(
                    "failed to read source file {} while loading suppressions",
                    source_path.display()
                )
            })?;
            let path_rules = parse_inline_suppressions(&path, &source)?;
            self.by_path.insert(path, path_rules);
        }

        Ok(std::mem::take(&mut self.by_path)
            .into_values()
            .flatten()
            .collect())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SuppressionScope {
    SameLine,
    NextLine,
    File,
    Config,
}

#[derive(Debug)]
struct SuppressionRule {
    path: String,
    kinds: Option<BTreeSet<Rule>>,
    line: Option<usize>,
    scope: SuppressionScope,
}

impl SuppressionRule {
    fn matches(&self, detection: &DetectedEvidence, root: &str) -> bool {
        if let Some(kinds) = &self.kinds
            && !kinds.contains(&detection.kind)
        {
            return false;
        }
        if !path_matches(self.path.as_str(), detection.path.as_str(), root) {
            return false;
        }

        match self.scope {
            SuppressionScope::File => true,
            SuppressionScope::Config => self.line.is_none() || self.line == detection.line,
            SuppressionScope::SameLine => self.line == detection.line,
            SuppressionScope::NextLine => self
                .line
                .and_then(|line| line.checked_add(1))
                .is_some_and(|line| Some(line) == detection.line),
        }
    }
}

fn parse_inline_suppressions(path: &str, source: &str) -> Result<Vec<SuppressionRule>> {
    let mut rules = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let Some(directive) = parse_directive(line)? else {
            continue;
        };
        let line_number = index + 1;
        rules.push(SuppressionRule {
            path: path.to_string(),
            kinds: directive.kinds,
            line: Some(line_number),
            scope: directive.scope,
        });
    }
    Ok(rules)
}

#[derive(Debug)]
struct InlineDirective {
    scope: SuppressionScope,
    kinds: Option<BTreeSet<Rule>>,
}

fn parse_directive(line: &str) -> Result<Option<InlineDirective>> {
    let Some(start) = line.find(DIRECTIVE_PREFIX) else {
        return Ok(None);
    };
    let after_prefix = &line[start + DIRECTIVE_PREFIX.len()..];

    for (name, scope) in [
        ("ignore-next-line", SuppressionScope::NextLine),
        ("ignore-file", SuppressionScope::File),
        ("ignore", SuppressionScope::SameLine),
    ] {
        if let Some(body) = directive_body(after_prefix, name) {
            return Ok(Some(InlineDirective {
                scope,
                kinds: parse_optional_kind_list(body)?,
            }));
        }
    }

    Ok(None)
}

fn directive_body<'a>(text: &'a str, directive: &str) -> Option<&'a str> {
    let body = text.strip_prefix(directive)?;
    if body.is_empty() || body.starts_with(char::is_whitespace) {
        Some(body.trim())
    } else {
        None
    }
}

fn parse_optional_kind_list(text: &str) -> Result<Option<BTreeSet<Rule>>> {
    let Some(first_token) = text.split_whitespace().next() else {
        return Ok(None);
    };

    match parse_kind_list(first_token) {
        Ok(kinds) => Ok(Some(kinds)),
        Err(error) if looks_like_kind_list(first_token) => Err(error),
        Err(_) => Ok(None),
    }
}

fn parse_required_kind_list(text: &str) -> Result<BTreeSet<Rule>> {
    parse_kind_list(text.trim())
}

fn parse_kind_list(text: &str) -> Result<BTreeSet<Rule>> {
    let mut kinds = BTreeSet::new();
    for raw_kind in text.split(',') {
        let kind = raw_kind.trim();
        if kind.is_empty() {
            bail!("empty detection kind in list '{text}'");
        }
        kinds.insert(parse_detection_kind(kind)?);
    }
    if kinds.is_empty() {
        bail!("detection kind list cannot be empty");
    }
    Ok(kinds)
}

fn parse_detection_kind(kind: &str) -> Result<Rule> {
    serde_json::from_value::<Rule>(serde_json::Value::String(kind.to_string())).map_err(|_| {
        anyhow!(
            "unknown detection kind '{kind}'; expected one of: {}",
            known_detection_kinds().join(", ")
        )
    })
}

fn looks_like_kind_list(text: &str) -> bool {
    text.contains(',') || text.contains('_')
}

fn path_matches(rule_path: &str, detection_path: &str, root: &str) -> bool {
    let rule_path = normalize_control_path(rule_path);
    let detection_path = normalize_control_path(detection_path);
    let detection_relative = relative_to_root(detection_path.as_str(), root);

    rule_path == detection_path || rule_path == detection_relative
}

fn source_path_for_detection(root: &str, detection_path: &str) -> PathBuf {
    if !Path::new(detection_path).is_absolute() {
        return Path::new(root).join(detection_path);
    }
    let relative = relative_to_root(detection_path, root);
    if relative == detection_path {
        PathBuf::from(detection_path)
    } else {
        Path::new(root).join(relative)
    }
}

fn relative_to_root<'a>(path: &'a str, root: &str) -> &'a str {
    path.strip_prefix(root)
        .and_then(|suffix| suffix.strip_prefix('/'))
        .unwrap_or(path)
}

fn normalize_control_path(path: &str) -> String {
    crate::pathing::normalize_path_text(path)
        .trim_start_matches("./")
        .trim_end_matches('/')
        .to_string()
}

fn display_path(path: &Path) -> String {
    crate::pathing::display_path(path)
}

fn known_detection_kinds() -> Vec<String> {
    [
        Rule::LargeFile,
        Rule::LargeDirectory,
        Rule::DebtMarker,
        Rule::SimilarFunctions,
        Rule::LongFunction,
        Rule::ComplexFunction,
        Rule::DeepNesting,
        Rule::ManyParameters,
        Rule::LargeType,
        Rule::LargePublicSurface,
        Rule::ImportHeavyFile,
        Rule::FunctionProliferation,
        Rule::UnusedFunction,
        Rule::RepeatedLiteral,
        Rule::RepeatedErrorPattern,
        Rule::TestDuplication,
        Rule::HappyPathOnlyTests,
        Rule::FileNamingDrift,
        Rule::DirectoryDrift,
        Rule::DataClump,
        Rule::ParallelImplementation,
        Rule::ShadowedAbstraction,
        Rule::DuplicateTypeShape,
        Rule::ConfigKeyDrift,
        Rule::FixtureFactoryDrift,
        Rule::GenericBucketDrift,
        Rule::AdapterBoundaryBypass,
        Rule::AdapterFlowBypass,
        Rule::StaleCompatibilityPath,
        Rule::MissingUserGuide,
        Rule::MissingReportSchemaDocs,
        Rule::MissingMetricsModelDocs,
        Rule::MissingArchitectureDocs,
        Rule::StaleCliDocumentation,
        Rule::StaleSchemaDocumentation,
        Rule::DependencyCycle,
        Rule::DependencyHub,
    ]
    .into_iter()
    .map(serialized_rule)
    .collect()
}
