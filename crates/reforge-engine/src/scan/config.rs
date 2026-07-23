use anyhow::{Context, Result, bail};
use globset::Glob;
use serde::{Deserialize, Serialize};

use crate::execution::{
    ChurnMode, DEFAULT_CHURN_MAX_COMMIT_LINES, DEFAULT_CHURN_WINDOW_DAYS, EffectiveConfig,
    ThresholdPreset,
};

use super::thresholds::{
    ConfigFileThresholdDefaults, ConfigRepetitionThresholdDefaults,
    ConfigSimilarityThresholdDefaults, ConfigStructureThresholdDefaults, ConfigThresholdDefaults,
    apply_threshold_defaults,
};

mod data_flow;
pub(crate) use data_flow::{DataFlowBoundaryConfig, DataFlowConfig};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct ConfigFile {
    preset: Option<ThresholdPreset>,
    max_file_lines: Option<usize>,
    max_dir_files: Option<usize>,
    min_similar_functions: Option<usize>,
    min_function_tokens: Option<usize>,
    function_similarity: Option<f64>,
    max_function_lines: Option<usize>,
    max_function_complexity: Option<usize>,
    max_nesting_depth: Option<usize>,
    max_function_parameters: Option<usize>,
    max_type_lines: Option<usize>,
    max_type_members: Option<usize>,
    max_imports: Option<usize>,
    max_public_items: Option<usize>,
    max_functions_per_file: Option<usize>,
    max_functions_per_100_lines: Option<usize>,
    max_small_function_ratio: Option<usize>,
    min_repeated_literal_occurrences: Option<usize>,
    min_data_clump_occurrences: Option<usize>,
    churn: Option<ChurnMode>,
    churn_window_days: Option<usize>,
    churn_max_commit_lines: Option<usize>,
    ignore_paths: Vec<String>,
    suppressions: Vec<ConfigSuppression>,
    #[serde(rename = "dataflow")]
    data_flow: DataFlowConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) struct ConfigSuppression {
    pub kind: Option<String>,
    pub path: String,
    pub line: Option<usize>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedConfig {
    pub args: EffectiveConfig,
    pub suppressions: Vec<ConfigSuppression>,
    pub data_flow: DataFlowConfig,
}

impl From<&ConfigFile> for ConfigThresholdDefaults {
    fn from(config: &ConfigFile) -> Self {
        Self {
            preset: config.preset,
            file: ConfigFileThresholdDefaults {
                max_file_lines: config.max_file_lines,
                max_dir_files: config.max_dir_files,
            },
            similarity: ConfigSimilarityThresholdDefaults {
                min_similar_functions: config.min_similar_functions,
                min_function_tokens: config.min_function_tokens,
                function_similarity: config.function_similarity,
            },
            structure: ConfigStructureThresholdDefaults {
                max_function_lines: config.max_function_lines,
                max_function_complexity: config.max_function_complexity,
                max_nesting_depth: config.max_nesting_depth,
                max_function_parameters: config.max_function_parameters,
                max_type_lines: config.max_type_lines,
                max_type_members: config.max_type_members,
                max_imports: config.max_imports,
                max_public_items: config.max_public_items,
                max_functions_per_file: config.max_functions_per_file,
                max_functions_per_100_lines: config.max_functions_per_100_lines,
                max_small_function_ratio: config.max_small_function_ratio,
            },
            repetition: ConfigRepetitionThresholdDefaults {
                min_repeated_literal_occurrences: config.min_repeated_literal_occurrences,
                min_data_clump_occurrences: config.min_data_clump_occurrences,
            },
        }
    }
}

pub(crate) fn parse_config_value(config: &toml::Value) -> Result<ConfigFile> {
    let mut output = toml::map::Map::new();
    if let Some(codebase) = config
        .get(reforge_schema::ANALYSIS_CODEBASE)
        .and_then(toml::Value::as_table)
    {
        output.extend(codebase.clone());
    }
    if let Some(paths) = config
        .get("scope")
        .and_then(|value| value.get("ignore-paths"))
    {
        output.insert("ignore-paths".into(), paths.clone());
    }
    let root = config
        .get(reforge_schema::ANALYSIS_DATAFLOW)
        .and_then(toml::Value::as_table);
    let mut dataflow = root
        .and_then(|value| value.get("search"))
        .and_then(toml::Value::as_table)
        .cloned()
        .unwrap_or_default();
    for section in ["relay", "fan-out"] {
        if let Some(values) = root
            .and_then(|value| value.get(section))
            .and_then(toml::Value::as_table)
        {
            dataflow.extend(values.clone());
        }
    }
    if let Some(policies) = root.and_then(|value| value.get("policies")).cloned() {
        dataflow.insert("boundaries".into(), policies);
    }
    output.insert(
        reforge_schema::ANALYSIS_DATAFLOW.into(),
        toml::Value::Table(dataflow),
    );
    if let Some(suppressions) = config.get("suppressions").and_then(toml::Value::as_array) {
        let mapped = suppressions
            .iter()
            .map(|suppression| {
                let mut table = suppression
                    .as_table()
                    .cloned()
                    .context("suppressions entries must be tables")?;
                if let Some(rule) = table.remove("rule") {
                    let rule = rule.as_str().context("suppression rule must be a string")?;
                    table.insert(
                        "kind".into(),
                        toml::Value::String(rule.rsplit('.').next().unwrap_or(rule).into()),
                    );
                }
                Ok(toml::Value::Table(table))
            })
            .collect::<Result<Vec<_>>>()?;
        output.insert("suppressions".into(), toml::Value::Array(mapped));
    }
    let config: ConfigFile = toml::Value::Table(output)
        .try_into()
        .context("failed to parse typed Reforge configuration")?;
    validate_data_flow_config(&config.data_flow)?;
    Ok(config)
}

pub(super) fn effective_scan_config_with(
    args: &EffectiveConfig,
    config: Option<&ConfigFile>,
) -> Result<ResolvedConfig> {
    let mut effective = args.clone();
    let suppressions = config
        .as_ref()
        .map(|config| config.suppressions.clone())
        .unwrap_or_default();
    let data_flow = config
        .as_ref()
        .map(|config| config.data_flow.clone())
        .unwrap_or_default();

    apply_config_defaults(&mut effective, config);

    effective.churn = Some(
        args.churn
            .unwrap_or(effective.churn.unwrap_or(ChurnMode::Auto)),
    );
    effective.churn_window_days = Some(
        args.churn_window_days.unwrap_or(
            effective
                .churn_window_days
                .unwrap_or(DEFAULT_CHURN_WINDOW_DAYS),
        ),
    );
    effective.churn_max_commit_lines = Some(
        args.churn_max_commit_lines.unwrap_or(
            effective
                .churn_max_commit_lines
                .unwrap_or(DEFAULT_CHURN_MAX_COMMIT_LINES),
        ),
    );

    Ok(ResolvedConfig {
        args: effective,
        suppressions,
        data_flow,
    })
}

fn validate_data_flow_config(config: &DataFlowConfig) -> Result<()> {
    if config.max_function_hops == 0 {
        bail!("dataflow.max-function-hops must be greater than zero");
    }
    for (name, value) in [
        ("max-path-steps", config.max_path_steps),
        ("max-module-hops", config.max_module_hops),
        ("max-paths-per-source", config.max_paths_per_source),
        ("max-sinks-per-source", config.max_sinks_per_source),
        ("work-budget", config.work_budget),
    ] {
        if value == 0 {
            bail!("dataflow.{name} must be greater than zero");
        }
    }
    let mut names = std::collections::BTreeSet::new();
    for boundary in &config.boundaries {
        validate_data_flow_boundary(boundary)?;
        if !names.insert(boundary.name.as_str()) {
            bail!("duplicate dataflow boundary name {:?}", boundary.name);
        }
    }
    Ok(())
}

fn validate_data_flow_boundary(boundary: &DataFlowBoundaryConfig) -> Result<()> {
    if boundary.name.trim().is_empty() {
        bail!("dataflow boundary names must not be empty");
    }
    if boundary.protected_paths.is_empty()
        || boundary.adapter_paths.is_empty()
        || boundary.sink_symbols.is_empty()
    {
        bail!(
            "dataflow boundary {:?} requires protected-paths, adapter-paths, and sink-symbols",
            boundary.name
        );
    }
    for pattern in boundary
        .protected_paths
        .iter()
        .chain(&boundary.adapter_paths)
        .chain(&boundary.exempt_paths)
    {
        validate_boundary_pattern(&boundary.name, pattern)?;
    }
    for symbol in &boundary.sink_symbols {
        if !is_fully_qualified_rust_symbol(symbol) {
            bail!(
                "dataflow boundary {:?} sink symbol {:?} must be a fully qualified crate:: Rust function",
                boundary.name,
                symbol
            );
        }
    }
    Ok(())
}

fn validate_boundary_pattern(boundary: &str, pattern: &str) -> Result<()> {
    if pattern.trim().is_empty() {
        bail!("dataflow boundary {boundary:?} contains an empty path");
    }
    Glob::new(pattern).with_context(|| {
        format!("dataflow boundary {boundary:?} contains invalid glob {pattern:?}")
    })?;
    Ok(())
}

fn is_fully_qualified_rust_symbol(symbol: &str) -> bool {
    let mut segments = symbol.split("::");
    segments.next() == Some("crate")
        && segments.clone().count() >= 1
        && segments.all(|segment| {
            !segment.is_empty()
                && segment.chars().enumerate().all(|(index, ch)| {
                    ch == '_' || ch.is_ascii_alphanumeric() && (index > 0 || !ch.is_ascii_digit())
                })
        })
}

fn apply_config_defaults(args: &mut EffectiveConfig, config: Option<&ConfigFile>) {
    apply_threshold_defaults(args, config.map(ConfigThresholdDefaults::from));
    if let Some(config) = config {
        apply_churn_config_defaults(args, config);
        apply_ignore_path_defaults(args, config);
    }
}

fn apply_churn_config_defaults(args: &mut EffectiveConfig, config: &ConfigFile) {
    args.churn = args.churn.or(config.churn);
    args.churn_window_days = args.churn_window_days.or(config.churn_window_days);
    args.churn_max_commit_lines = args
        .churn_max_commit_lines
        .or(config.churn_max_commit_lines);
}

fn apply_ignore_path_defaults(args: &mut EffectiveConfig, config: &ConfigFile) {
    if config.ignore_paths.is_empty() {
        return;
    }

    let mut ignore_paths = config.ignore_paths.clone();
    for path in &args.filters.ignore_paths {
        if !ignore_paths.contains(path) {
            ignore_paths.push(path.clone());
        }
    }
    args.filters.ignore_paths = ignore_paths;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boundary() -> DataFlowBoundaryConfig {
        DataFlowBoundaryConfig {
            name: "http-client".into(),
            protected_paths: vec!["src/application/**".into()],
            adapter_paths: vec!["src/adapters/http/**".into()],
            sink_symbols: vec!["crate::transport::send".into()],
            exempt_paths: vec!["src/bin/**".into()],
        }
    }

    #[test]
    fn validates_complete_data_flow_policy() {
        let config = DataFlowConfig {
            max_function_hops: 4,
            max_path_steps: 12,
            max_module_hops: 4,
            max_paths_per_source: 100,
            max_sinks_per_source: 5,
            work_budget: 100_000,
            boundaries: vec![boundary()],
            ..DataFlowConfig::default()
        };
        validate_data_flow_config(&config).unwrap();
    }

    #[test]
    fn rejects_incomplete_or_ambiguous_data_flow_policy() {
        let mut config = DataFlowConfig {
            max_function_hops: 0,
            max_path_steps: 12,
            max_module_hops: 4,
            max_paths_per_source: 100,
            max_sinks_per_source: 5,
            work_budget: 100_000,
            boundaries: Vec::new(),
            ..DataFlowConfig::default()
        };
        assert!(
            validate_data_flow_config(&config)
                .unwrap_err()
                .to_string()
                .contains("max-function-hops")
        );
        config.max_function_hops = 4;
        validate_data_flow_config(&config).unwrap();
        config.boundaries = vec![boundary(), boundary()];
        assert!(
            validate_data_flow_config(&config)
                .unwrap_err()
                .to_string()
                .contains("duplicate")
        );
        config.boundaries.truncate(1);
        config.boundaries[0].sink_symbols = vec!["transport::send".into()];
        assert!(
            validate_data_flow_config(&config)
                .unwrap_err()
                .to_string()
                .contains("fully qualified")
        );
        config.boundaries[0] = boundary();
        config.boundaries[0].protected_paths = vec!["[invalid".into()];
        assert!(
            validate_data_flow_config(&config)
                .unwrap_err()
                .to_string()
                .contains("invalid glob")
        );
    }
}
