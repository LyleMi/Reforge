use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

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
    #[arg(long, default_value_t = 40)]
    pub min_function_tokens: usize,

    /// Minimum normalized token similarity for functions to be grouped.
    #[arg(long, default_value_t = 0.80)]
    pub function_similarity: f64,
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
}
