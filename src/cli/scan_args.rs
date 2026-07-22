use super::*;

#[derive(Debug, Clone, Args)]
pub struct ScanArgs {
    /// Directory or file to scan.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    #[arg(skip)]
    pub(crate) threshold_overrides: ThresholdOverrideFlags,

    /// Built-in threshold preset to use before per-threshold overrides.
    #[arg(long, value_enum)]
    pub preset: Option<ThresholdPreset>,

    /// Unity project analysis mode. Auto enables it at Unity project roots.
    #[arg(long, value_enum, default_value_t = UnityMode::Auto)]
    pub unity: UnityMode,

    /// Report Unity assemblies with more direct dependencies than this threshold.
    #[arg(long, default_value_t = 8)]
    pub max_unity_assembly_dependencies: usize,

    /// Report Unity scenes with more serialized objects than this threshold.
    #[arg(long, default_value_t = 1_000)]
    pub max_unity_scene_objects: usize,

    /// Report Unity prefabs with more serialized objects than this threshold.
    #[arg(long, default_value_t = 250)]
    pub max_unity_prefab_objects: usize,

    /// Report Unity behaviours with more serializable fields than this threshold.
    #[arg(long, default_value_t = 16)]
    pub max_unity_serialized_fields: usize,

    /// Report Unity behaviours with more lifecycle methods than this threshold.
    #[arg(long, default_value_t = 7)]
    pub max_unity_lifecycle_methods: usize,

    /// Report files whose total line count is above this threshold.
    #[arg(long, default_value_t = DEFAULT_MAX_FILE_LINES)]
    pub max_file_lines: usize,

    /// Report directories whose direct source file count is above this threshold.
    #[arg(long, default_value_t = DEFAULT_MAX_DIR_FILES)]
    pub max_dir_files: usize,

    #[command(flatten)]
    pub filters: ScanFilterArgs,

    #[command(flatten)]
    pub finding_controls: FindingControlArgs,

    #[command(flatten)]
    pub analysis_thresholds: AnalysisThresholdArgs,

    /// Optional configuration file. When omitted, reforge.toml is discovered from the scan root.
    #[arg(long)]
    pub config: Option<PathBuf>,

    #[command(flatten)]
    pub ci: CiArgs,

    /// Git churn collection mode.
    #[arg(long, value_enum)]
    pub churn: Option<ChurnMode>,

    /// Number of days of git history to include in churn metrics.
    #[arg(long)]
    pub churn_window_days: Option<usize>,

    /// Skip commits whose numstat added+deleted line count exceeds this value.
    #[arg(long)]
    pub churn_max_commit_lines: Option<usize>,

    /// Output format.
    #[arg(long, value_enum)]
    pub output: Option<OutputFormat>,

    /// Write the report to this file instead of stdout.
    #[arg(long)]
    pub output_file: Option<PathBuf>,

    /// Zero volatile runtime measurements for reproducible serialized reports.
    #[arg(long)]
    pub reproducible: bool,

    /// Progress reporting mode. Auto writes to stderr only when stderr is a TTY.
    #[arg(long, value_enum, default_value_t = ProgressMode::Auto)]
    pub progress: ProgressMode,

    /// Colorize human output. Auto writes colors only when stdout is a TTY.
    #[arg(long, value_enum, default_value_t = ColorMode::Auto)]
    pub color: ColorMode,
}

#[derive(Debug, Clone, Args)]
pub struct AnalysisThresholdArgs {
    /// Report groups with at least this many structurally similar functions.
    #[arg(long, default_value_t = DEFAULT_MIN_SIMILAR_FUNCTIONS)]
    pub min_similar_functions: usize,

    /// Ignore functions whose normalized body has fewer tokens than this threshold.
    #[arg(long, default_value_t = DEFAULT_MIN_FUNCTION_TOKENS)]
    pub min_function_tokens: usize,

    /// Minimum normalized token similarity for functions to be grouped.
    #[arg(long, default_value_t = DEFAULT_FUNCTION_SIMILARITY)]
    pub function_similarity: f64,

    /// Include test files in similar-function analysis.
    #[arg(long)]
    pub include_test_similarity: bool,

    /// Report functions whose line span is above this threshold.
    #[arg(long, default_value_t = DEFAULT_MAX_FUNCTION_LINES)]
    pub max_function_lines: usize,

    /// Report functions whose estimated cyclomatic complexity is above this threshold.
    #[arg(long, default_value_t = DEFAULT_MAX_FUNCTION_COMPLEXITY)]
    pub max_function_complexity: usize,

    /// Report functions whose nested control-flow depth is above this threshold.
    #[arg(long, default_value_t = DEFAULT_MAX_NESTING_DEPTH)]
    pub max_nesting_depth: usize,

    /// Report functions with more parameters than this threshold.
    #[arg(long, default_value_t = DEFAULT_MAX_FUNCTION_PARAMETERS)]
    pub max_function_parameters: usize,

    /// Report types whose line span is above this threshold.
    #[arg(long, default_value_t = DEFAULT_MAX_TYPE_LINES)]
    pub max_type_lines: usize,

    /// Report types whose member count is above this threshold.
    #[arg(long, default_value_t = DEFAULT_MAX_TYPE_MEMBERS)]
    pub max_type_members: usize,

    /// Report files with more imports than this threshold.
    #[arg(long, default_value_t = DEFAULT_MAX_IMPORTS)]
    pub max_imports: usize,

    /// Report files with more public/exported items than this threshold.
    #[arg(long, default_value_t = DEFAULT_MAX_PUBLIC_ITEMS)]
    pub max_public_items: usize,

    #[command(flatten)]
    pub function_proliferation: FunctionProliferationArgs,

    /// Report repeated literals seen at least this many times.
    #[arg(long, default_value_t = DEFAULT_MIN_REPEATED_LITERAL_OCCURRENCES)]
    pub min_repeated_literal_occurrences: usize,

    /// Report repeated parameter groups seen at least this many times.
    #[arg(long, default_value_t = DEFAULT_MIN_DATA_CLUMP_OCCURRENCES)]
    pub min_data_clump_occurrences: usize,

    /// Include test files in general structural analysis.
    #[arg(long)]
    pub include_test_structure: bool,
}

impl std::ops::Deref for ScanArgs {
    type Target = AnalysisThresholdArgs;

    fn deref(&self) -> &Self::Target {
        &self.analysis_thresholds
    }
}

impl std::ops::DerefMut for ScanArgs {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.analysis_thresholds
    }
}

#[derive(Debug, Clone, Args)]
pub struct FunctionProliferationArgs {
    /// Report files with more functions than this threshold when density signals also match.
    #[arg(long, default_value_t = DEFAULT_MAX_FUNCTIONS_PER_FILE)]
    pub max_functions_per_file: usize,

    /// Report files above this function density per 100 lines when other proliferation signals match.
    #[arg(long, default_value_t = DEFAULT_MAX_FUNCTIONS_PER_100_LINES)]
    pub max_functions_per_100_lines: usize,

    /// Report files whose small-function percentage exceeds this threshold when other proliferation signals match.
    #[arg(long, default_value_t = DEFAULT_MAX_SMALL_FUNCTION_RATIO)]
    pub max_small_function_ratio: usize,
}

#[derive(Debug, Clone, Args)]
pub struct CiArgs {
    /// Compare current findings against a prior JSON/YAML Reforge report.
    #[arg(long)]
    pub baseline: Option<PathBuf>,

    /// Which findings are considered when a baseline is present.
    #[arg(long, value_enum, default_value_t = BaselineMode::New)]
    pub baseline_mode: BaselineMode,

    /// Which baseline comparison findings to show in human output.
    #[arg(long, value_enum, default_value_t = BaselineShow::All)]
    pub show: BaselineShow,

    /// Exit with a failure when unsuppressed findings match the selected schema 23 baseline mode.
    #[arg(long)]
    pub fail_on_findings: bool,

    /// Accept engine, detector-policy, or effective-configuration provenance changes.
    #[arg(long)]
    pub accept_baseline_provenance_change: bool,
}

#[derive(Debug, Clone, Default, Args)]
pub struct ScanFilterArgs {
    /// Include hidden files and directories.
    #[arg(long)]
    pub include_hidden: bool,

    /// Include dependency and generated output directories.
    #[arg(long)]
    pub include_generated: bool,

    /// Do not apply .gitignore rules during scanning.
    #[arg(long)]
    pub no_gitignore: bool,

    /// Exclude test files and test directories from scanning.
    #[arg(long)]
    pub exclude_tests: bool,

    /// Additional path to skip during scanning. Can be repeated.
    #[arg(long = "ignore-path", value_name = "PATH")]
    pub ignore_paths: Vec<String>,
}

#[derive(Debug, Clone, Default, Args)]
pub struct FindingControlArgs {
    /// Only report findings from these detector kinds.
    #[arg(long, value_name = "KIND[,KIND...]")]
    pub only: Option<String>,

    /// Exclude findings from these detector kinds.
    #[arg(long = "exclude-detector", value_name = "KIND[,KIND...]")]
    pub exclude_detector: Option<String>,
}
