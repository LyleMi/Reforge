use super::*;

#[derive(Debug, Clone)]
pub struct EffectiveConfig {
    /// Directory or file to scan.
    pub path: PathBuf,

    /// Built-in threshold preset to use before per-threshold overrides.
    pub preset: Option<ThresholdPreset>,

    /// Report files whose total line count is above this threshold.
    pub max_file_lines: usize,

    /// Report directories whose direct source file count is above this threshold.
    pub max_dir_files: usize,

    pub filters: ScanFilterArgs,

    pub codebase_thresholds: CodebaseThresholds,

    /// Git churn collection mode.
    pub churn: Option<ChurnMode>,

    /// Number of days of git history to include in churn metrics.
    pub churn_window_days: Option<usize>,

    /// Skip commits whose numstat added+deleted line count exceeds this value.
    pub churn_max_commit_lines: Option<usize>,

    /// Zero volatile runtime measurements for reproducible serialized reports.
    pub reproducible: bool,

    /// Progress reporting mode. Auto writes to stderr only when stderr is a TTY.
    pub progress: ProgressMode,
}

#[derive(Debug, Clone)]
pub struct CodebaseThresholds {
    /// Report groups with at least this many structurally similar functions.
    pub min_similar_functions: usize,

    /// Ignore functions whose normalized body has fewer tokens than this threshold.
    pub min_function_tokens: usize,

    /// Minimum normalized token similarity for functions to be grouped.
    pub function_similarity: f64,

    /// Report functions whose line span is above this threshold.
    pub max_function_lines: usize,

    /// Report functions whose estimated cyclomatic complexity is above this threshold.
    pub max_function_complexity: usize,

    /// Report functions whose nested control-flow depth is above this threshold.
    pub max_nesting_depth: usize,

    /// Report functions with more parameters than this threshold.
    pub max_function_parameters: usize,

    /// Report types whose line span is above this threshold.
    pub max_type_lines: usize,

    /// Report types whose member count is above this threshold.
    pub max_type_members: usize,

    /// Report files with more imports than this threshold.
    pub max_imports: usize,

    /// Report files with more public/exported items than this threshold.
    pub max_public_items: usize,

    pub function_proliferation: FunctionProliferationArgs,

    /// Report repeated literals seen at least this many times.
    pub min_repeated_literal_occurrences: usize,

    /// Report repeated parameter groups seen at least this many times.
    pub min_data_clump_occurrences: usize,
}

impl std::ops::Deref for EffectiveConfig {
    type Target = CodebaseThresholds;

    fn deref(&self) -> &Self::Target {
        &self.codebase_thresholds
    }
}

impl std::ops::DerefMut for EffectiveConfig {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.codebase_thresholds
    }
}

#[derive(Debug, Clone)]
pub struct FunctionProliferationArgs {
    /// Report files with more functions than this threshold when density signals also match.
    pub max_functions_per_file: usize,

    /// Report files above this function density per 100 lines when other proliferation signals match.
    pub max_functions_per_100_lines: usize,

    /// Report files whose small-function percentage exceeds this threshold when other proliferation signals match.
    pub max_small_function_ratio: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ScanFilterArgs {
    /// Include hidden files and directories.
    pub include_hidden: bool,

    /// Include dependency and generated output directories.
    pub include_generated: bool,

    /// Do not apply .gitignore rules during scanning.
    pub no_gitignore: bool,

    /// Exclude test files and test directories from scanning.
    pub exclude_tests: bool,

    /// Additional path to skip during scanning. Can be repeated.
    pub ignore_paths: Vec<String>,
}
