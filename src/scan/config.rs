use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::cli::{ChurnMode, HotspotModel, ScanArgs};

const DEFAULT_MAX_FILE_LINES: usize = 800;
const DEFAULT_MAX_DIR_FILES: usize = 40;
const DEFAULT_MIN_SIMILAR_FUNCTIONS: usize = 3;
const DEFAULT_MIN_FUNCTION_TOKENS: usize = 80;
const DEFAULT_FUNCTION_SIMILARITY: f64 = 0.85;
const DEFAULT_MAX_FUNCTION_LINES: usize = 80;
const DEFAULT_MAX_FUNCTION_COMPLEXITY: usize = 15;
const DEFAULT_MAX_NESTING_DEPTH: usize = 4;
const DEFAULT_MAX_FUNCTION_PARAMETERS: usize = 5;
const DEFAULT_MAX_TYPE_LINES: usize = 250;
const DEFAULT_MAX_TYPE_MEMBERS: usize = 30;
const DEFAULT_MAX_IMPORTS: usize = 35;
const DEFAULT_MAX_PUBLIC_ITEMS: usize = 30;
const DEFAULT_MIN_REPEATED_LITERAL_OCCURRENCES: usize = 4;
const DEFAULT_MIN_DATA_CLUMP_OCCURRENCES: usize = 3;
const DEFAULT_CHURN_WINDOW_DAYS: usize = 180;
const DEFAULT_CHURN_MAX_COMMIT_LINES: usize = 2_000;

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
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
    min_repeated_literal_occurrences: Option<usize>,
    min_data_clump_occurrences: Option<usize>,
    churn: Option<ChurnMode>,
    hotspot_model: Option<HotspotModel>,
    churn_window_days: Option<usize>,
    churn_max_commit_lines: Option<usize>,
    ignore_paths: Vec<String>,
}

pub(super) fn effective_scan_args(args: &ScanArgs, root: &Path) -> Result<ScanArgs> {
    let mut effective = args.clone();
    let config = load_config(args, root)?;

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

    Ok(effective)
}

fn load_config(args: &ScanArgs, root: &Path) -> Result<Option<ReforgeConfig>> {
    let config_path = if let Some(path) = &args.config {
        Some(path.clone())
    } else {
        discover_config_path(root)
    };

    let Some(path) = config_path else {
        return Ok(None);
    };

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;
    let config = toml::from_str(&contents)
        .with_context(|| format!("failed to parse config file {}", path.display()))?;
    Ok(Some(config))
}

fn discover_config_path(root: &Path) -> Option<PathBuf> {
    let mut current = if root.is_file() {
        root.parent()?.to_path_buf()
    } else {
        root.to_path_buf()
    };

    loop {
        let candidate = current.join("reforge.toml");
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
    for path in &args.ignore_paths {
        if !ignore_paths.contains(path) {
            ignore_paths.push(path.clone());
        }
    }
    args.ignore_paths = ignore_paths;
}

fn apply_usize_default(target: &mut usize, default: usize, configured: Option<usize>) {
    if *target == default
        && let Some(value) = configured
    {
        *target = value;
    }
}
