mod cli;
mod report;
mod scanner;
mod similar_functions;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan(args) => {
            let findings = scanner::scan_path(&args)?;
            report::print_findings(&findings);
        }
    }

    Ok(())
}
