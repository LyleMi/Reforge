use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

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

#[derive(Debug, Args)]
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

    /// Include hidden files and directories.
    #[arg(long)]
    pub include_hidden: bool,

    /// Include dependency and generated output directories.
    #[arg(long)]
    pub include_generated: bool,

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

    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub output: OutputFormat,

    /// Write the report to this file instead of stdout.
    #[arg(long)]
    pub output_file: Option<PathBuf>,

    /// Progress reporting mode. Auto writes to stderr only when stderr is a TTY.
    #[arg(long, value_enum, default_value_t = ProgressMode::Auto)]
    pub progress: ProgressMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ProgressMode {
    Auto,
    Always,
    Never,
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
    fn parses_output_format() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output", "json"]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.output, OutputFormat::Json);
    }

    #[test]
    fn parses_output_file() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output-file", "report.json"]);

        let Command::Scan(args) = cli.command;
        assert_eq!(args.output_file, Some(PathBuf::from("report.json")));
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
}
