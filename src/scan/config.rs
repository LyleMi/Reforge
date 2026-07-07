use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cli::{
    ChurnMode, DEFAULT_CHURN_MAX_COMMIT_LINES, DEFAULT_CHURN_WINDOW_DAYS,
    DEFAULT_FUNCTION_SIMILARITY, DEFAULT_MAX_DIR_FILES, DEFAULT_MAX_FILE_LINES,
    DEFAULT_MAX_FUNCTION_COMPLEXITY, DEFAULT_MAX_FUNCTION_LINES, DEFAULT_MAX_FUNCTION_PARAMETERS,
    DEFAULT_MAX_FUNCTIONS_PER_100_LINES, DEFAULT_MAX_FUNCTIONS_PER_FILE, DEFAULT_MAX_IMPORTS,
    DEFAULT_MAX_NESTING_DEPTH, DEFAULT_MAX_PUBLIC_ITEMS, DEFAULT_MAX_SMALL_FUNCTION_RATIO,
    DEFAULT_MAX_TYPE_LINES, DEFAULT_MAX_TYPE_MEMBERS, DEFAULT_MIN_DATA_CLUMP_OCCURRENCES,
    DEFAULT_MIN_FUNCTION_TOKENS, DEFAULT_MIN_REPEATED_LITERAL_OCCURRENCES,
    DEFAULT_MIN_SIMILAR_FUNCTIONS, HotspotModel, ScanArgs,
};

pub(crate) const CONFIG_FILE_NAME: &str = "reforge.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
struct ReforgeConfig {
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
    hotspot_model: Option<HotspotModel>,
    churn_window_days: Option<usize>,
    churn_max_commit_lines: Option<usize>,
    ignore_paths: Vec<String>,
    suppressions: Vec<ConfigSuppression>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ConfigSuppression {
    pub kind: Option<String>,
    pub path: String,
    pub line: Option<usize>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub(super) struct EffectiveScanConfig {
    pub args: ScanArgs,
    pub suppressions: Vec<ConfigSuppression>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct EffectiveConfigOutput {
    max_file_lines: usize,
    max_dir_files: usize,
    include_hidden: bool,
    include_generated: bool,
    no_gitignore: bool,
    exclude_tests: bool,
    ignore_paths: Vec<String>,
    min_similar_functions: usize,
    min_function_tokens: usize,
    function_similarity: f64,
    include_test_similarity: bool,
    max_function_lines: usize,
    max_function_complexity: usize,
    max_nesting_depth: usize,
    max_function_parameters: usize,
    max_type_lines: usize,
    max_type_members: usize,
    max_imports: usize,
    max_public_items: usize,
    max_functions_per_file: usize,
    max_functions_per_100_lines: usize,
    max_small_function_ratio: usize,
    min_repeated_literal_occurrences: usize,
    min_data_clump_occurrences: usize,
    include_test_structure: bool,
    churn: ChurnMode,
    hotspot_model: HotspotModel,
    churn_window_days: usize,
    churn_max_commit_lines: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ReforgeConfigTemplate {
    max_file_lines: usize,
    max_dir_files: usize,
    min_similar_functions: usize,
    min_function_tokens: usize,
    function_similarity: f64,
    max_function_lines: usize,
    max_function_complexity: usize,
    max_nesting_depth: usize,
    max_function_parameters: usize,
    max_type_lines: usize,
    max_type_members: usize,
    max_imports: usize,
    max_public_items: usize,
    max_functions_per_file: usize,
    max_functions_per_100_lines: usize,
    max_small_function_ratio: usize,
    min_repeated_literal_occurrences: usize,
    min_data_clump_occurrences: usize,
    churn: ChurnMode,
    hotspot_model: HotspotModel,
    churn_window_days: usize,
    churn_max_commit_lines: usize,
    ignore_paths: Vec<String>,
}

pub(crate) fn effective_scan_config(args: &ScanArgs, root: &Path) -> Result<EffectiveScanConfig> {
    let mut effective = args.clone();
    let config = load_config(args, root)?;
    let suppressions = config
        .as_ref()
        .map(|config| config.suppressions.clone())
        .unwrap_or_default();

    if let Some(config) = config {
        apply_config_defaults(&mut effective, &config);
    }

    effective.churn = Some(
        args.churn
            .unwrap_or(effective.churn.unwrap_or(ChurnMode::Auto)),
    );
    effective.hotspot_model = Some(
        args.hotspot_model
            .unwrap_or(effective.hotspot_model.unwrap_or(HotspotModel::Hybrid)),
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

    Ok(EffectiveScanConfig {
        args: effective,
        suppressions,
    })
}

pub(crate) fn validate_config(config_path: Option<&Path>, root: &Path) -> Result<Option<PathBuf>> {
    let Some(path) = resolve_config_path(config_path, root) else {
        return Ok(None);
    };

    parse_config_file(&path)?;
    Ok(Some(path))
}

pub(crate) fn effective_config_output(
    args: &ScanArgs,
    root: &Path,
) -> Result<EffectiveConfigOutput> {
    let effective = effective_scan_config(args, root)?;
    Ok(EffectiveConfigOutput::from(&effective.args))
}

pub(crate) fn default_config_toml() -> Result<String> {
    let defaults = ScanArgs::default();
    let template = ReforgeConfigTemplate::from(&defaults);
    let mut output =
        toml::to_string_pretty(&template).context("failed to serialize default configuration")?;
    if !output.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

fn load_config(args: &ScanArgs, root: &Path) -> Result<Option<ReforgeConfig>> {
    let config_path = resolve_config_path(args.config.as_deref(), root);

    let Some(path) = config_path else {
        return Ok(None);
    };

    Ok(Some(parse_config_file(&path)?))
}

fn resolve_config_path(config_path: Option<&Path>, root: &Path) -> Option<PathBuf> {
    config_path
        .map(Path::to_path_buf)
        .or_else(|| discover_config_path(root))
}

fn parse_config_file(path: &Path) -> Result<ReforgeConfig> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;
    let config = toml::from_str(&contents)
        .with_context(|| format!("failed to parse config file {}", path.display()))?;
    Ok(config)
}

fn discover_config_path(root: &Path) -> Option<PathBuf> {
    let mut current = if root.is_file() {
        root.parent()?.to_path_buf()
    } else {
        root.to_path_buf()
    };

    loop {
        let candidate = current.join(CONFIG_FILE_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn apply_config_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    apply_file_config_defaults(args, config);
    apply_similarity_config_defaults(args, config);
    apply_structure_config_defaults(args, config);
    apply_repetition_config_defaults(args, config);
    apply_churn_config_defaults(args, config);
    apply_ignore_path_defaults(args, config);
}

fn apply_file_config_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    apply_usize_default(
        &mut args.max_file_lines,
        DEFAULT_MAX_FILE_LINES,
        config.max_file_lines,
    );
    apply_usize_default(
        &mut args.max_dir_files,
        DEFAULT_MAX_DIR_FILES,
        config.max_dir_files,
    );
}

fn apply_similarity_config_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    apply_usize_default(
        &mut args.min_similar_functions,
        DEFAULT_MIN_SIMILAR_FUNCTIONS,
        config.min_similar_functions,
    );
    apply_usize_default(
        &mut args.min_function_tokens,
        DEFAULT_MIN_FUNCTION_TOKENS,
        config.min_function_tokens,
    );
    if (args.function_similarity - DEFAULT_FUNCTION_SIMILARITY).abs() < f64::EPSILON
        && let Some(value) = config.function_similarity
    {
        args.function_similarity = value;
    }
}

fn apply_structure_config_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    apply_usize_default(
        &mut args.max_function_lines,
        DEFAULT_MAX_FUNCTION_LINES,
        config.max_function_lines,
    );
    apply_usize_default(
        &mut args.max_function_complexity,
        DEFAULT_MAX_FUNCTION_COMPLEXITY,
        config.max_function_complexity,
    );
    apply_usize_default(
        &mut args.max_nesting_depth,
        DEFAULT_MAX_NESTING_DEPTH,
        config.max_nesting_depth,
    );
    apply_usize_default(
        &mut args.max_function_parameters,
        DEFAULT_MAX_FUNCTION_PARAMETERS,
        config.max_function_parameters,
    );
    apply_usize_default(
        &mut args.max_type_lines,
        DEFAULT_MAX_TYPE_LINES,
        config.max_type_lines,
    );
    apply_usize_default(
        &mut args.max_type_members,
        DEFAULT_MAX_TYPE_MEMBERS,
        config.max_type_members,
    );
    apply_usize_default(
        &mut args.max_imports,
        DEFAULT_MAX_IMPORTS,
        config.max_imports,
    );
    apply_usize_default(
        &mut args.max_public_items,
        DEFAULT_MAX_PUBLIC_ITEMS,
        config.max_public_items,
    );
    apply_usize_default(
        &mut args.function_proliferation.max_functions_per_file,
        DEFAULT_MAX_FUNCTIONS_PER_FILE,
        config.max_functions_per_file,
    );
    apply_usize_default(
        &mut args.function_proliferation.max_functions_per_100_lines,
        DEFAULT_MAX_FUNCTIONS_PER_100_LINES,
        config.max_functions_per_100_lines,
    );
    apply_usize_default(
        &mut args.function_proliferation.max_small_function_ratio,
        DEFAULT_MAX_SMALL_FUNCTION_RATIO,
        config.max_small_function_ratio,
    );
}

fn apply_repetition_config_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    apply_usize_default(
        &mut args.min_repeated_literal_occurrences,
        DEFAULT_MIN_REPEATED_LITERAL_OCCURRENCES,
        config.min_repeated_literal_occurrences,
    );
    apply_usize_default(
        &mut args.min_data_clump_occurrences,
        DEFAULT_MIN_DATA_CLUMP_OCCURRENCES,
        config.min_data_clump_occurrences,
    );
}

fn apply_churn_config_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    args.churn = args.churn.or(config.churn);
    args.hotspot_model = args.hotspot_model.or(config.hotspot_model);
    args.churn_window_days = args.churn_window_days.or(config.churn_window_days);
    args.churn_max_commit_lines = args
        .churn_max_commit_lines
        .or(config.churn_max_commit_lines);
}

fn apply_ignore_path_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
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

fn apply_usize_default(target: &mut usize, default: usize, configured: Option<usize>) {
    if *target == default
        && let Some(value) = configured
    {
        *target = value;
    }
}

impl From<&ScanArgs> for EffectiveConfigOutput {
    fn from(args: &ScanArgs) -> Self {
        Self {
            max_file_lines: args.max_file_lines,
            max_dir_files: args.max_dir_files,
            include_hidden: args.filters.include_hidden,
            include_generated: args.filters.include_generated,
            no_gitignore: args.filters.no_gitignore,
            exclude_tests: args.filters.exclude_tests,
            ignore_paths: args.filters.ignore_paths.clone(),
            min_similar_functions: args.min_similar_functions,
            min_function_tokens: args.min_function_tokens,
            function_similarity: args.function_similarity,
            include_test_similarity: args.include_test_similarity,
            max_function_lines: args.max_function_lines,
            max_function_complexity: args.max_function_complexity,
            max_nesting_depth: args.max_nesting_depth,
            max_function_parameters: args.max_function_parameters,
            max_type_lines: args.max_type_lines,
            max_type_members: args.max_type_members,
            max_imports: args.max_imports,
            max_public_items: args.max_public_items,
            max_functions_per_file: args.function_proliferation.max_functions_per_file,
            max_functions_per_100_lines: args.function_proliferation.max_functions_per_100_lines,
            max_small_function_ratio: args.function_proliferation.max_small_function_ratio,
            min_repeated_literal_occurrences: args.min_repeated_literal_occurrences,
            min_data_clump_occurrences: args.min_data_clump_occurrences,
            include_test_structure: args.include_test_structure,
            churn: args.churn.expect("effective args should set churn mode"),
            hotspot_model: args
                .hotspot_model
                .expect("effective args should set hotspot model"),
            churn_window_days: args
                .churn_window_days
                .expect("effective args should set churn window"),
            churn_max_commit_lines: args
                .churn_max_commit_lines
                .expect("effective args should set churn max commit lines"),
        }
    }
}

impl From<&ScanArgs> for ReforgeConfigTemplate {
    fn from(args: &ScanArgs) -> Self {
        Self {
            max_file_lines: args.max_file_lines,
            max_dir_files: args.max_dir_files,
            min_similar_functions: args.min_similar_functions,
            min_function_tokens: args.min_function_tokens,
            function_similarity: args.function_similarity,
            max_function_lines: args.max_function_lines,
            max_function_complexity: args.max_function_complexity,
            max_nesting_depth: args.max_nesting_depth,
            max_function_parameters: args.max_function_parameters,
            max_type_lines: args.max_type_lines,
            max_type_members: args.max_type_members,
            max_imports: args.max_imports,
            max_public_items: args.max_public_items,
            max_functions_per_file: args.function_proliferation.max_functions_per_file,
            max_functions_per_100_lines: args.function_proliferation.max_functions_per_100_lines,
            max_small_function_ratio: args.function_proliferation.max_small_function_ratio,
            min_repeated_literal_occurrences: args.min_repeated_literal_occurrences,
            min_data_clump_occurrences: args.min_data_clump_occurrences,
            churn: ChurnMode::Auto,
            hotspot_model: HotspotModel::Hybrid,
            churn_window_days: DEFAULT_CHURN_WINDOW_DAYS,
            churn_max_commit_lines: DEFAULT_CHURN_MAX_COMMIT_LINES,
            ignore_paths: args.filters.ignore_paths.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("reforge-config-{name}-{suffix}"))
    }

    #[test]
    fn default_config_toml_is_valid() -> Result<()> {
        let config = default_config_toml()?;
        let parsed: ReforgeConfig = toml::from_str(&config)?;

        assert_eq!(parsed.max_file_lines, Some(DEFAULT_MAX_FILE_LINES));
        assert_eq!(
            parsed.min_repeated_literal_occurrences,
            Some(DEFAULT_MIN_REPEATED_LITERAL_OCCURRENCES)
        );
        assert_eq!(
            parsed.min_data_clump_occurrences,
            Some(DEFAULT_MIN_DATA_CLUMP_OCCURRENCES)
        );
        assert_eq!(parsed.churn, Some(ChurnMode::Auto));
        assert_eq!(parsed.hotspot_model, Some(HotspotModel::Hybrid));
        Ok(())
    }

    #[test]
    fn validate_config_rejects_unknown_keys() -> Result<()> {
        let root = test_root("unknown-key");
        fs::create_dir_all(&root)?;
        let config_path = root.join(CONFIG_FILE_NAME);
        fs::write(&config_path, "unknown-key = true\n")?;

        let result = validate_config(None, &root);

        fs::remove_dir_all(root)?;

        assert!(result.is_err());
        assert!(format!("{:#}", result.unwrap_err()).contains("unknown field"));
        Ok(())
    }

    #[test]
    fn effective_config_output_applies_discovered_config() -> Result<()> {
        let root = test_root("effective");
        fs::create_dir_all(&root)?;
        fs::write(
            root.join(CONFIG_FILE_NAME),
            "max-file-lines = 600\nchurn = \"off\"\nhotspot-model = \"static\"\nignore-paths = [\"vendor\"]\n",
        )?;
        let args = ScanArgs::defaults_for_path(root.clone());

        let output = effective_config_output(&args, &root)?;

        fs::remove_dir_all(root)?;

        assert_eq!(output.max_file_lines, 600);
        assert_eq!(output.churn, ChurnMode::Off);
        assert_eq!(output.hotspot_model, HotspotModel::Static);
        assert_eq!(output.ignore_paths, ["vendor"]);
        Ok(())
    }
}
