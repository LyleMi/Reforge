mod baseline;
mod cli;
mod detectors;
mod lang;
mod model;
mod output;
mod scan;
mod scoring;

mod agent_drift {
    pub(crate) use crate::detectors::drift::*;
}

mod documentation {
    pub(crate) use crate::detectors::documentation::*;
}

mod language {
    pub(crate) use crate::lang::*;
}

mod report {
    pub(crate) use crate::output::*;
}

mod scanner {
    pub(crate) use crate::model::*;
    pub(crate) use crate::scan::*;
    pub(crate) use crate::scoring::*;
}

mod similar_functions {
    pub(crate) use crate::detectors::similarity::*;
}

mod structural {
    pub(crate) use crate::detectors::structure::*;
}

mod unused_functions {
    pub(crate) use crate::detectors::unused_functions::*;
}

use anyhow::{Context, Result, bail};
use clap::Parser;
use std::fs::File;
use std::io::{BufWriter, ErrorKind, IsTerminal, Write};

use crate::cli::{Cli, Command, OutputFormat, ScanArgs};
use crate::model::ScanReport;
use crate::scan::{NoopProgress, StderrProgress};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan(args) => run_scan(args)?,
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct OutputSettings {
    format: OutputFormat,
    color: bool,
}

fn run_scan(args: ScanArgs) -> Result<()> {
    let stderr_is_tty = std::io::stderr().is_terminal();
    let settings = output_settings(&args);
    let baseline_report = args
        .ci
        .baseline
        .as_ref()
        .map(|path| baseline::load_baseline(path))
        .transpose()?;
    let report = scan_with_progress(&args, stderr_is_tty)?;
    let selected = baseline::selected_findings(
        &report.findings,
        baseline_report.as_ref(),
        args.ci.baseline_mode,
    );
    let gate_failures = args
        .ci
        .fail_on
        .map(|threshold| baseline::gate_failures(selected, threshold))
        .unwrap_or_default();

    if args.output_file.is_some() {
        write_report_file(&args, &report, settings)?;
    } else {
        print_report(&report, settings)?;
    }

    if !gate_failures.is_empty() {
        bail!(
            "scan failed: {} selected findings met --fail-on {:?}",
            gate_failures.len(),
            args.ci.fail_on.expect("gate failures require fail-on")
        );
    }

    Ok(())
}

fn output_settings(args: &ScanArgs) -> OutputSettings {
    let format = args.output_format();
    let stdout_is_tty = std::io::stdout().is_terminal();
    let color = matches!(format, OutputFormat::Human)
        && args
            .color
            .enabled(args.output_file.is_none() && stdout_is_tty);

    OutputSettings { format, color }
}

fn scan_with_progress(args: &ScanArgs, stderr_is_tty: bool) -> Result<ScanReport> {
    if args.progress.enabled(stderr_is_tty) {
        let mut progress = StderrProgress::new(stderr_is_tty);
        scanner::scan_report(args, &mut progress)
    } else {
        let mut progress = NoopProgress;
        scanner::scan_report(args, &mut progress)
    }
}

fn write_report_file(args: &ScanArgs, report: &ScanReport, settings: OutputSettings) -> Result<()> {
    let output_file = args
        .output_file
        .as_ref()
        .expect("output file should be present before writing");
    let file = File::create(output_file)
        .with_context(|| format!("failed to create output file {}", output_file.display()))?;
    write_report(BufWriter::new(file), report, settings)
}

fn write_report(writer: impl Write, report: &ScanReport, settings: OutputSettings) -> Result<()> {
    match settings.format {
        OutputFormat::Human if settings.color => {
            report::write_human_report_colored(writer, report, true)?;
        }
        OutputFormat::Human => report::write_human_report(writer, report)?,
        OutputFormat::Html => report::write_html_report(writer, report)?,
        OutputFormat::Json => report::write_json_report(writer, report)?,
        OutputFormat::Sarif => report::write_sarif_report(writer, report)?,
        OutputFormat::Yaml => report::write_yaml_report(writer, report)?,
    }

    Ok(())
}

fn print_report(report: &ScanReport, settings: OutputSettings) -> Result<()> {
    match settings.format {
        OutputFormat::Human if settings.color => {
            handle_output_result(report::print_human_report_colored(report, true))
        }
        OutputFormat::Human => handle_output_result(report::print_human_report(report)),
        OutputFormat::Html => handle_output_result(report::print_html_report(report)),
        OutputFormat::Json => handle_output_result(report::print_json_report(report)),
        OutputFormat::Sarif => handle_output_result(report::print_sarif_report(report)),
        OutputFormat::Yaml => handle_output_result(report::print_yaml_report(report)),
    }
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
