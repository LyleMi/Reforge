use super::*;

#[derive(Debug, Parser)]
#[command(name = "reforge")]
#[command(about = "Detect refactoring signals across a codebase")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Write a default reforge.toml configuration file.
    Init(InitArgs),
    /// Inspect and validate configuration.
    Config(ConfigArgs),
    /// Scan a directory for basic refactoring signals.
    Scan(Box<ScanArgs>),
    /// Prepare, validate, fit, or evaluate anonymous local calibration data.
    Calibrate(CalibrateArgs),
}

#[derive(Debug, Clone, Args)]
pub struct CalibrateArgs {
    #[command(subcommand)]
    pub command: CalibrateCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum CalibrateCommand {
    Prepare(CalibratePrepareArgs),
    Validate(CalibrateArtifactArgs),
    Fit(CalibrateArtifactArgs),
    Evaluate(CalibrateArtifactArgs),
}

#[derive(Debug, Clone, Args)]
pub struct CalibratePrepareArgs {
    #[arg(long)]
    pub samples_root: PathBuf,
    #[arg(long, default_value = "target/calibration")]
    pub output_dir: PathBuf,
    #[arg(long, default_value_t = 300)]
    pub max_issues: usize,
}

#[derive(Debug, Clone, Args)]
pub struct CalibrateArtifactArgs {
    #[arg(long, default_value = "target/calibration")]
    pub calibration_dir: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct InitArgs {
    /// Directory to receive reforge.toml, or an exact .toml file path.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Overwrite an existing configuration file.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigCommand {
    /// Validate discovered or explicit configuration without scanning.
    Validate(ConfigValidateArgs),
    /// Print effective scan defaults after applying configuration.
    Show(ConfigShowArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ConfigValidateArgs {
    /// Directory or file used for reforge.toml discovery.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Optional configuration file. When omitted, reforge.toml is discovered from PATH.
    #[arg(long)]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct ConfigShowArgs {
    /// Directory or file used for reforge.toml discovery.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Optional configuration file. When omitted, reforge.toml is discovered from PATH.
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = ConfigOutputFormat::Human)]
    pub output: ConfigOutputFormat,
}
