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
}
