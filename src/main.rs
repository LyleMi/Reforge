mod cli;
mod report;
mod scanner;
mod similar_functions;
mod structural;

use anyhow::{Context, Result};
use clap::Parser;
use std::fs::File;
use std::io::BufWriter;
use std::io::ErrorKind;
use std::io::IsTerminal;

use crate::cli::{Cli, Command, OutputFormat};
use crate::scanner::{NoopProgress, StderrProgress};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan(args) => {
            let output_format = args.output_format();
            let stderr_is_tty = std::io::stderr().is_terminal();
            let stdout_is_tty = std::io::stdout().is_terminal();
            let progress_enabled = args.progress.enabled(stderr_is_tty);
            let color_enabled = matches!(output_format, OutputFormat::Human)
                && args
                    .color
                    .enabled(args.output_file.is_none() && stdout_is_tty);

            let report = if progress_enabled {
                let mut progress = StderrProgress::new(stderr_is_tty);
                scanner::scan_report(&args, &mut progress)?
            } else {
                let mut progress = NoopProgress;
                scanner::scan_report(&args, &mut progress)?
            };

            if let Some(output_file) = &args.output_file {
                let file = File::create(output_file).with_context(|| {
                    format!("failed to create output file {}", output_file.display())
                })?;
                let writer = BufWriter::new(file);

                match output_format {
                    OutputFormat::Human => {
                        if color_enabled {
                            report::write_human_report_colored(writer, &report, true)?
                        } else {
                            report::write_human_report(writer, &report)?
                        }
                    }
                    OutputFormat::Json => report::write_json_report(writer, &report)?,
                }
            } else {
                match output_format {
                    OutputFormat::Human => {
                        if color_enabled {
                            handle_output_result(report::print_human_report_colored(&report, true))?
                        } else {
                            handle_output_result(report::print_human_report(&report))?
                        }
                    }
                    OutputFormat::Json => handle_output_result(report::print_json_report(&report))?,
                }
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
