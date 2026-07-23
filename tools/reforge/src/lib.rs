use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use reforge_engine::api::{self, Analysis, AnalyzeOptions, Config};
use reforge_output::{OutputFormat, load_report, write_report};
use reforge_schema::Report;
use serde::Serialize;

mod configuration;
use configuration::*;

const CONFIG_NAME: &str = "reforge.toml";

#[derive(Debug, Parser)]
#[command(
    name = "reforge",
    version,
    about = "Analyze code through complementary Codebase and Dataflow analyses"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Analyze a source tree and emit one combined report.
    Analyze(AnalyzeCommand),
    /// List rules and their analysis ownership.
    Rules(RulesCommand),
    /// Write a default versioned reforge.toml.
    Init(InitCommand),
    /// Inspect or validate configuration.
    Config(ConfigCommand),
}

#[derive(Debug, Args)]
struct AnalyzeCommand {
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Select a core analysis. Repeat to combine analyses.
    #[arg(long, value_enum)]
    analysis: Vec<AnalysisArg>,
    #[arg(long)]
    config: Option<PathBuf>,
    /// Override a nested configuration key. Repeat as needed.
    #[arg(long = "set", value_name = "KEY=VALUE")]
    overrides: Vec<String>,
    #[arg(long, value_enum)]
    output: Option<FormatArg>,
    #[arg(long)]
    output_file: Option<PathBuf>,
    #[arg(long)]
    baseline: Option<PathBuf>,
    #[arg(long, value_enum)]
    gate: Option<GateArg>,
    #[arg(long)]
    reproducible: bool,
    #[arg(long)]
    include_hidden: bool,
    #[arg(long)]
    include_generated: bool,
    #[arg(long)]
    no_gitignore: bool,
    #[arg(long)]
    exclude_tests: bool,
    #[arg(long = "ignore-path")]
    ignore_paths: Vec<String>,
    /// Write Codebase raw metrics to a separate JSON sidecar.
    #[arg(long)]
    metrics_output: Option<PathBuf>,
    /// Write the complete Dataflow IR to a separate JSON sidecar.
    #[arg(long)]
    flow_ir_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct RulesCommand {
    #[arg(long, value_enum)]
    analysis: Vec<AnalysisArg>,
    #[arg(long, value_enum, default_value_t = TextFormatArg::Human)]
    output: TextFormatArg,
}

#[derive(Debug, Args)]
struct InitCommand {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct ConfigCommand {
    #[command(subcommand)]
    command: ConfigSubcommand,
}

#[derive(Debug, Subcommand)]
enum ConfigSubcommand {
    /// Validate a discovered or explicit configuration.
    Validate(ConfigPath),
    /// Show final effective values and their sources.
    Show(ConfigShow),
}

#[derive(Debug, Args)]
struct ConfigPath {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long)]
    config: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ConfigShow {
    #[command(flatten)]
    source: ConfigPath,
    #[arg(long = "set", value_name = "KEY=VALUE")]
    overrides: Vec<String>,
    #[arg(long, value_enum, default_value_t = TextFormatArg::Human)]
    output: TextFormatArg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum AnalysisArg {
    Codebase,
    Dataflow,
}

impl From<AnalysisArg> for Analysis {
    fn from(value: AnalysisArg) -> Self {
        match value {
            AnalysisArg::Codebase => Analysis::Codebase,
            AnalysisArg::Dataflow => Analysis::Dataflow,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FormatArg {
    Human,
    Html,
    Json,
    Yaml,
    Sarif,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TextFormatArg {
    Human,
    Json,
    Yaml,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GateArg {
    New,
    All,
}

#[derive(Debug, Serialize)]
struct EffectiveConfigView {
    config_file: Option<String>,
    values: toml::Value,
    sources: BTreeMap<String, String>,
}

#[allow(dead_code)]
fn main() -> Result<()> {
    run_from(std::env::args_os())
}

pub fn run_from<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            error.print()?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    match cli.command {
        Command::Analyze(command) => analyze(command),
        Command::Rules(command) => rules(command),
        Command::Init(command) => init(command),
        Command::Config(command) => config(command),
    }
}

fn analyze(command: AnalyzeCommand) -> Result<()> {
    let (_config_path, mut config) = load_config(command.config.as_deref(), &command.path)?;
    for value in &command.overrides {
        apply_override(&mut config, value)?;
    }
    validate_config(&config)?;
    let mut report = run_analysis(&command, &config)?;
    attach_baseline(&mut report, command.baseline.as_deref())?;
    let failures = gate_failures(&report, command.gate)?;
    let format = OutputFormat::infer(
        command.output.map(Into::into),
        command.output_file.as_deref(),
    );
    write_destination(command.output_file.as_deref(), &report, format)?;
    if failures > 0 {
        bail!("analysis gate failed with {failures} issue(s)");
    }
    Ok(())
}

fn run_analysis(command: &AnalyzeCommand, config: &toml::Value) -> Result<Report> {
    let mut config = Config::parse_toml(&toml::to_string(config)?)?;
    if !command.analysis.is_empty() {
        config.set_enabled(command.analysis.iter().copied().map(Into::into).collect())?;
    }
    config.apply_scope_overrides(
        command.include_hidden,
        command.include_generated,
        command.no_gitignore,
        command.exclude_tests,
        &command.ignore_paths,
    );
    api::analyze(&AnalyzeOptions {
        root: command.path.clone(),
        config,
        reproducible: command.reproducible,
        metrics_output: command.metrics_output.clone(),
        flow_ir_output: command.flow_ir_output.clone(),
    })
}

fn attach_baseline(report: &mut Report, baseline: Option<&Path>) -> Result<()> {
    if let Some(path) = baseline {
        let baseline = load_report(path)?;
        report.validate_baseline(&baseline)?;
        let downgrades = report.coverage_downgrades(&baseline);
        if !downgrades.is_empty() {
            bail!(
                "baseline coverage degraded for {}; missing issues cannot be classified as resolved",
                downgrades.join(", ")
            );
        }
        report.baseline_comparison = Some(report.compare_to(&baseline));
    }
    Ok(())
}

fn gate_failures(report: &Report, gate: Option<GateArg>) -> Result<usize> {
    Ok(match gate {
        None => 0,
        Some(GateArg::All) => report.issues.len(),
        Some(GateArg::New) => report
            .baseline_comparison
            .as_ref()
            .context("--gate new requires --baseline")?
            .new_issue_ids
            .len(),
    })
}

fn rules(command: RulesCommand) -> Result<()> {
    let analyses = if command.analysis.is_empty() {
        BTreeSet::from([Analysis::Codebase, Analysis::Dataflow])
    } else {
        command.analysis.iter().copied().map(Into::into).collect()
    };
    let entries = api::rules(&analyses);
    render_values(&entries, command.output)
}

fn init(command: InitCommand) -> Result<()> {
    let path = config_destination(&command.path);
    if path.exists() && !command.force {
        bail!(
            "{} already exists; use --force to replace it",
            path.display()
        );
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, default_config())?;
    println!("Wrote {}", path.display());
    Ok(())
}

fn write_destination(path: Option<&Path>, report: &Report, format: OutputFormat) -> Result<()> {
    let result = if let Some(path) = path {
        if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        write_report(File::create(path)?, report, format)
    } else {
        write_report(std::io::stdout().lock(), report, format)
    };
    match result {
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == ErrorKind::BrokenPipe) =>
        {
            Ok(())
        }
        result => result,
    }
}

impl From<FormatArg> for OutputFormat {
    fn from(value: FormatArg) -> Self {
        match value {
            FormatArg::Human => Self::Human,
            FormatArg::Html => Self::Html,
            FormatArg::Json => Self::Json,
            FormatArg::Yaml => Self::Yaml,
            FormatArg::Sarif => Self::Sarif,
        }
    }
}

#[cfg(test)]
#[path = "unit_tests.rs"]
mod tests;
