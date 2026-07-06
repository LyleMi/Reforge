use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(name = "reforge")]
#[command(about = "Detect refactoring signals across a codebase")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Scan a directory for basic refactoring signals.
    Scan(ScanArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ScanArgs {
    /// Directory or file to scan.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Report files whose total line count is above this threshold.
    #[arg(long, default_value_t = 800)]
    pub max_file_lines: usize,

    /// Report directories whose direct source file count is above this threshold.
    #[arg(long, default_value_t = 40)]
    pub max_dir_files: usize,

    #[command(flatten)]
    pub filters: ScanFilterArgs,

    /// Report groups with at least this many structurally similar functions.
    #[arg(long, default_value_t = 3)]
    pub min_similar_functions: usize,

    /// Ignore functions whose normalized body has fewer tokens than this threshold.
    #[arg(long, default_value_t = 80)]
    pub min_function_tokens: usize,

    /// Minimum normalized token similarity for functions to be grouped.
    #[arg(long, default_value_t = 0.85)]
    pub function_similarity: f64,

    /// Include test files in similar-function analysis.
    #[arg(long)]
    pub include_test_similarity: bool,

    /// Report functions whose line span is above this threshold.
    #[arg(long, default_value_t = 80)]
    pub max_function_lines: usize,

    /// Report functions whose estimated cyclomatic complexity is above this threshold.
    #[arg(long, default_value_t = 15)]
    pub max_function_complexity: usize,

    /// Report functions whose nested control-flow depth is above this threshold.
    #[arg(long, default_value_t = 4)]
    pub max_nesting_depth: usize,

    /// Report functions with more parameters than this threshold.
    #[arg(long, default_value_t = 5)]
    pub max_function_parameters: usize,

    /// Report types whose line span is above this threshold.
    #[arg(long, default_value_t = 250)]
    pub max_type_lines: usize,

    /// Report types whose member count is above this threshold.
    #[arg(long, default_value_t = 30)]
    pub max_type_members: usize,

    /// Report files with more imports than this threshold.
    #[arg(long, default_value_t = 35)]
    pub max_imports: usize,

    /// Report files with more public/exported items than this threshold.
    #[arg(long, default_value_t = 30)]
    pub max_public_items: usize,

    #[command(flatten)]
    pub function_proliferation: FunctionProliferationArgs,

    /// Report repeated literals seen at least this many times.
    #[arg(long, default_value_t = 4)]
    pub min_repeated_literal_occurrences: usize,

    /// Report repeated parameter groups seen at least this many times.
    #[arg(long, default_value_t = 3)]
    pub min_data_clump_occurrences: usize,

    /// Include test files in general structural analysis.
    #[arg(long)]
    pub include_test_structure: bool,

    /// Optional configuration file. When omitted, reforge.toml is discovered from the scan root.
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Git churn collection mode.
    #[arg(long, value_enum)]
    pub churn: Option<ChurnMode>,

    /// Hotspot ranking model.
    #[arg(long, value_enum)]
    pub hotspot_model: Option<HotspotModel>,

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

    /// Progress reporting mode. Auto writes to stderr only when stderr is a TTY.
    #[arg(long, value_enum, default_value_t = ProgressMode::Auto)]
    pub progress: ProgressMode,

    /// Colorize human output. Auto writes colors only when stdout is a TTY.
    #[arg(long, value_enum, default_value_t = ColorMode::Auto)]
    pub color: ColorMode,
}

#[derive(Debug, Clone, Args)]
pub struct FunctionProliferationArgs {
    /// Report files with more functions than this threshold when density signals also match.
    #[arg(long, default_value_t = 40)]
    pub max_functions_per_file: usize,

    /// Report files above this function density per 100 lines when other proliferation signals match.
    #[arg(long, default_value_t = 12)]
    pub max_functions_per_100_lines: usize,

    /// Report files whose small-function percentage exceeds this threshold when other proliferation signals match.
    #[arg(long, default_value_t = 70)]
    pub max_small_function_ratio: usize,
}

#[derive(Debug, Clone, Args)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
    Yaml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ProgressMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChurnMode {
    Auto,
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotspotModel {
    Static,
    Churn,
    Hybrid,
}

impl ScanArgs {
    pub fn output_format(&self) -> OutputFormat {
        self.output
            .unwrap_or_else(|| match self.output_file_extension() {
                Some(extension) if extension.eq_ignore_ascii_case("json") => OutputFormat::Json,
                Some(extension)
                    if extension.eq_ignore_ascii_case("yaml")
                        || extension.eq_ignore_ascii_case("yml") =>
                {
                    OutputFormat::Yaml
                }
                _ => OutputFormat::Human,
            })
    }

    fn output_file_extension(&self) -> Option<&str> {
        self.output_file
            .as_ref()
            .and_then(|path| path.extension())
            .and_then(|extension| extension.to_str())
    }
}

impl ProgressMode {
    pub fn enabled(self, stderr_is_tty: bool) -> bool {
        match self {
            ProgressMode::Auto => stderr_is_tty,
            ProgressMode::Always => true,
            ProgressMode::Never => false,
        }
    }
}

impl ColorMode {
    pub fn enabled(self, stdout_is_tty: bool) -> bool {
        match self {
            ColorMode::Auto => stdout_is_tty,
            ColorMode::Always => true,
            ColorMode::Never => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_similar_function_thresholds() {
        let cli = Cli::parse_from([
            "reforge",
            "scan",
            ".",
            "--min-similar-functions",
            "4",
            "--min-function-tokens",
            "25",
            "--function-similarity",
            "0.9",
        ]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.min_similar_functions, 4);
        assert_eq!(args.min_function_tokens, 25);
        assert_eq!(args.function_similarity, 0.9);
    }

    #[test]
    fn uses_stricter_default_similarity_thresholds() {
        let cli = Cli::parse_from(["reforge", "scan", "."]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.min_function_tokens, 80);
        assert_eq!(args.function_similarity, 0.85);
        assert!(!args.include_test_similarity);
    }

    #[test]
    fn parses_test_similarity_flag() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--include-test-similarity"]);

        let Command::Scan(args) = cli.command;
        assert!(args.include_test_similarity);
    }

    #[test]
    fn parses_structure_thresholds() {
        let cli = Cli::parse_from([
            "reforge",
            "scan",
            ".",
            "--max-function-lines",
            "60",
            "--max-function-complexity",
            "10",
            "--max-nesting-depth",
            "3",
            "--max-function-parameters",
            "4",
            "--max-type-lines",
            "120",
            "--max-type-members",
            "20",
            "--max-imports",
            "12",
            "--max-public-items",
            "8",
            "--max-functions-per-file",
            "24",
            "--max-functions-per-100-lines",
            "10",
            "--max-small-function-ratio",
            "65",
            "--min-repeated-literal-occurrences",
            "5",
            "--min-data-clump-occurrences",
            "4",
            "--include-test-structure",
        ]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.max_function_lines, 60);
        assert_eq!(args.max_function_complexity, 10);
        assert_eq!(args.max_nesting_depth, 3);
        assert_eq!(args.max_function_parameters, 4);
        assert_eq!(args.max_type_lines, 120);
        assert_eq!(args.max_type_members, 20);
        assert_eq!(args.max_imports, 12);
        assert_eq!(args.max_public_items, 8);
        assert_eq!(args.function_proliferation.max_functions_per_file, 24);
        assert_eq!(args.function_proliferation.max_functions_per_100_lines, 10);
        assert_eq!(args.function_proliferation.max_small_function_ratio, 65);
        assert_eq!(args.min_repeated_literal_occurrences, 5);
        assert_eq!(args.min_data_clump_occurrences, 4);
        assert!(args.include_test_structure);
    }

    #[test]
    fn uses_default_structure_thresholds() {
        let cli = Cli::parse_from(["reforge", "scan", "."]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.max_function_lines, 80);
        assert_eq!(args.max_function_complexity, 15);
        assert_eq!(args.max_nesting_depth, 4);
        assert_eq!(args.max_function_parameters, 5);
        assert_eq!(args.max_type_lines, 250);
        assert_eq!(args.max_type_members, 30);
        assert_eq!(args.max_imports, 35);
        assert_eq!(args.max_public_items, 30);
        assert_eq!(args.function_proliferation.max_functions_per_file, 40);
        assert_eq!(args.function_proliferation.max_functions_per_100_lines, 12);
        assert_eq!(args.function_proliferation.max_small_function_ratio, 70);
        assert_eq!(args.min_repeated_literal_occurrences, 4);
        assert_eq!(args.min_data_clump_occurrences, 3);
        assert!(!args.include_test_structure);
    }

    #[test]
    fn parses_scan_ignore_options() {
        let cli = Cli::parse_from([
            "reforge",
            "scan",
            ".",
            "--ignore-path",
            "vendor",
            "--ignore-path",
            "generated/snapshots",
            "--no-gitignore",
            "--exclude-tests",
        ]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.filters.ignore_paths, ["vendor", "generated/snapshots"]);
        assert!(args.filters.no_gitignore);
        assert!(args.filters.exclude_tests);
    }

    #[test]
    fn parses_output_format() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output", "json"]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.output, Some(OutputFormat::Json));
        assert_eq!(args.output_format(), OutputFormat::Json);
    }

    #[test]
    fn parses_yaml_output_format() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output", "yaml"]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.output, Some(OutputFormat::Yaml));
        assert_eq!(args.output_format(), OutputFormat::Yaml);
    }

    #[test]
    fn parses_output_file() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output-file", "report.json"]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.output_file, Some(PathBuf::from("report.json")));
    }

    #[test]
    fn infers_json_output_format_from_output_file_extension() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output-file", "report.json"]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.output_format(), OutputFormat::Json);
    }

    #[test]
    fn infers_json_output_format_from_uppercase_output_file_extension() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output-file", "REPORT.JSON"]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.output_format(), OutputFormat::Json);
    }

    #[test]
    fn infers_yaml_output_format_from_output_file_extension() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output-file", "report.yaml"]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.output_format(), OutputFormat::Yaml);
    }

    #[test]
    fn infers_yaml_output_format_from_yml_output_file_extension() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output-file", "REPORT.YML"]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.output_format(), OutputFormat::Yaml);
    }

    #[test]
    fn keeps_explicit_output_format_when_output_file_extension_is_json() {
        let cli = Cli::parse_from([
            "reforge",
            "scan",
            ".",
            "--output-file",
            "report.json",
            "--output",
            "human",
        ]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.output_format(), OutputFormat::Human);
    }

    #[test]
    fn keeps_explicit_output_format_when_output_file_extension_is_yaml() {
        let cli = Cli::parse_from([
            "reforge",
            "scan",
            ".",
            "--output-file",
            "report.yaml",
            "--output",
            "json",
        ]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.output_format(), OutputFormat::Json);
    }

    #[test]
    fn parses_progress_mode() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--progress", "never"]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.progress, ProgressMode::Never);
    }

    #[test]
    fn resolves_progress_modes() {
        assert!(ProgressMode::Auto.enabled(true));
        assert!(!ProgressMode::Auto.enabled(false));
        assert!(ProgressMode::Always.enabled(false));
        assert!(!ProgressMode::Never.enabled(true));
    }

    #[test]
    fn parses_color_mode() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--color", "never"]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.color, ColorMode::Never);
    }

    #[test]
    fn resolves_color_modes() {
        assert!(ColorMode::Auto.enabled(true));
        assert!(!ColorMode::Auto.enabled(false));
        assert!(ColorMode::Always.enabled(false));
        assert!(!ColorMode::Never.enabled(true));
    }

    #[test]
    fn parses_quality_model_options() {
        let cli = Cli::parse_from([
            "reforge",
            "scan",
            ".",
            "--churn",
            "on",
            "--hotspot-model",
            "static",
            "--churn-window-days",
            "90",
            "--churn-max-commit-lines",
            "1000",
        ]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.churn, Some(ChurnMode::On));
        assert_eq!(args.hotspot_model, Some(HotspotModel::Static));
        assert_eq!(args.churn_window_days, Some(90));
        assert_eq!(args.churn_max_commit_lines, Some(1000));
    }
}
