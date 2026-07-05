mod cli;
mod report;
mod scanner;
mod similar_functions;

use anyhow::Result;
use clap::Parser;
use std::io::ErrorKind;
use std::io::IsTerminal;

use crate::cli::{Cli, Command, OutputFormat};
use crate::scanner::{NoopProgress, StderrProgress};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan(args) => {
            let progress_enabled = args.progress.enabled(std::io::stderr().is_terminal());

            let report = if progress_enabled {
                let mut progress = StderrProgress::new();
                scanner::scan_report(&args, &mut progress)?
            } else {
                let mut progress = NoopProgress;
                scanner::scan_report(&args, &mut progress)?
            };

            match args.output {
                OutputFormat::Human => handle_output_result(report::print_human_report(&report))?,
                OutputFormat::Json => handle_output_result(report::print_json_report(&report))?,
            }
        }
    }

    Ok(())
}

fn handle_output_result<E>(result: std::result::Result<(), E>) -> Result<()>
where
    E: Into<anyhow::Error>,
{
    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            let error = error.into();
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io_error| io_error.kind() == ErrorKind::BrokenPipe)
            {
                std::process::exit(0);
            } else {
                Err(error)
            }
        }
    }
}
