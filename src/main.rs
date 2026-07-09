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
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, ErrorKind, IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::cli::{
    Cli, Command, ConfigArgs, ConfigCommand, ConfigOutputFormat, ConfigShowArgs,
    ConfigValidateArgs, InitArgs, OutputFormat, ScanArgs,
};
use crate::model::ScanReport;
use crate::scan::{NoopProgress, StderrProgress};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init(args) => run_init(args)?,
        Command::Config(args) => run_config(args)?,
        Command::Scan(args) => run_scan(*args)?,
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct OutputSettings {
    format: OutputFormat,
    color: bool,
}

fn run_init(args: InitArgs) -> Result<()> {
    let output_path = init_output_path(&args.path);
    if output_path.exists() && !args.force {
        bail!(
            "configuration file {} already exists; pass --force to overwrite it",
            output_path.display()
        );
    }

    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let mut file = if args.force {
        OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&output_path)
    } else {
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&output_path)
    }
    .with_context(|| format!("failed to write config file {}", output_path.display()))?;

    file.write_all(scan::default_config_toml()?.as_bytes())
        .with_context(|| format!("failed to write config file {}", output_path.display()))?;
    println!("Wrote {}", output_path.display());
    Ok(())
}

fn init_output_path(path: &Path) -> PathBuf {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
    {
        path.to_path_buf()
    } else {
        path.join(scan::CONFIG_FILE_NAME)
    }
}

fn run_config(args: ConfigArgs) -> Result<()> {
    match args.command {
        ConfigCommand::Validate(args) => run_config_validate(args),
        ConfigCommand::Show(args) => run_config_show(args),
    }
}

fn run_config_validate(args: ConfigValidateArgs) -> Result<()> {
    match scan::validate_config(args.config.as_deref(), &args.path)? {
        Some(path) => println!("Config valid: {}", path.display()),
        None => println!("No reforge.toml found; defaults are valid."),
    }
    Ok(())
}

fn run_config_show(args: ConfigShowArgs) -> Result<()> {
    let mut scan_args = ScanArgs::defaults_for_path(args.path);
    scan_args.config = args.config;
    let effective = scan::effective_config_output(&scan_args, &scan_args.path)?;

    match args.output {
        ConfigOutputFormat::Human => print_effective_config_human(&effective),
        ConfigOutputFormat::Json => {
            let mut stdout = std::io::stdout().lock();
            serde_json::to_writer_pretty(&mut stdout, &effective)?;
            writeln!(stdout)?;
            Ok(())
        }
        ConfigOutputFormat::Yaml => {
            let mut stdout = std::io::stdout().lock();
            serde_yaml::to_writer(&mut stdout, &effective)?;
            Ok(())
        }
    }
}

fn print_effective_config_human(config: &impl serde::Serialize) -> Result<()> {
    let value = serde_json::to_value(config)?;
    let Some(fields) = value.as_object() else {
        bail!("effective config did not serialize to an object");
    };

    println!("Effective Reforge config");
    for (key, value) in fields {
        println!("  {key}: {}", render_config_value(value)?);
    }
    Ok(())
}

fn render_config_value(value: &serde_json::Value) -> Result<String> {
    Ok(match value {
        serde_json::Value::String(value) => value.clone(),
        _ => serde_json::to_string(value)?,
    })
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
    let baseline_diff = baseline_report
        .as_ref()
        .map(|baseline| baseline::diff_findings(&report.findings, baseline, args.ci.show));
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
        write_report_file(&args, &report, baseline_diff.as_ref(), settings)?;
    } else {
        print_report(&report, baseline_diff.as_ref(), settings)?;
    }

    if !gate_failures.is_empty() {
        bail!(
            "scan failed: {} selected unsuppressed findings met --fail-on {:?}",
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

fn write_report_file(
    args: &ScanArgs,
    report: &ScanReport,
    baseline_diff: Option<&baseline::BaselineDiff<'_>>,
    settings: OutputSettings,
) -> Result<()> {
    let output_file = args
        .output_file
        .as_ref()
        .expect("output file should be present before writing");
    let file = File::create(output_file)
        .with_context(|| format!("failed to create output file {}", output_file.display()))?;
    write_report(BufWriter::new(file), report, baseline_diff, settings)
}

fn write_report(
    writer: impl Write,
    report: &ScanReport,
    baseline_diff: Option<&baseline::BaselineDiff<'_>>,
    settings: OutputSettings,
) -> Result<()> {
    match settings.format {
        OutputFormat::Human if settings.color && baseline_diff.is_some() => {
            report::write_human_report_with_baseline_colored(
                writer,
                report,
                baseline_diff.expect("checked above"),
                true,
            )?;
        }
        OutputFormat::Human if settings.color => {
            report::write_human_report_colored(writer, report, true)?;
        }
        OutputFormat::Human if baseline_diff.is_some() => {
            report::write_human_report_with_baseline(
                writer,
                report,
                baseline_diff.expect("checked above"),
            )?;
        }
        OutputFormat::Human => report::write_human_report(writer, report)?,
        OutputFormat::Html => report::write_html_report(writer, report)?,
        OutputFormat::Json => report::write_json_report(writer, report)?,
        OutputFormat::Sarif => report::write_sarif_report(writer, report)?,
        OutputFormat::Yaml => report::write_yaml_report(writer, report)?,
    }

    Ok(())
}

fn print_report(
    report: &ScanReport,
    baseline_diff: Option<&baseline::BaselineDiff<'_>>,
    settings: OutputSettings,
) -> Result<()> {
    match settings.format {
        OutputFormat::Human if settings.color && baseline_diff.is_some() => {
            handle_output_result(report::print_human_report_with_baseline_colored(
                report,
                baseline_diff.expect("checked above"),
                true,
            ))
        }
        OutputFormat::Human if settings.color => {
            handle_output_result(report::print_human_report_colored(report, true))
        }
        OutputFormat::Human if baseline_diff.is_some() => handle_output_result(
            report::print_human_report_with_baseline(report, baseline_diff.expect("checked above")),
        ),
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
