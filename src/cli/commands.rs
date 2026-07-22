use super::*;

#[derive(Debug, Parser)]
#[command(name = "reforge")]
#[command(about = "Detect refactoring signals across a codebase")]
#[command(version = env!("CARGO_PKG_VERSION"), long_version = env!("REFORGE_LONG_VERSION"))]
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
    /// Manage a resumable, approval-gated refactoring workflow.
    Workflow(WorkflowArgs),
}

#[derive(Debug, Clone, Args)]
pub struct WorkflowArgs {
    #[command(subcommand)]
    pub command: WorkflowCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum WorkflowCommand {
    /// Scan a target and create a resumable run directory.
    Start(Box<WorkflowStartArgs>),
    /// Select report issue IDs and record the workflow goal.
    Select(WorkflowSelectArgs),
    /// Show the current run phase and artifact summary.
    Status(WorkflowRunArgs),
    /// Validate all artifacts without changing them.
    Validate(WorkflowRunArgs),
    /// Advance from selected to investigated or investigated to planned.
    Advance(WorkflowRunArgs),
    /// Approve the plan, write set, and current workspace snapshot.
    Approve(WorkflowRunArgs),
    /// Record workspace changes and enforce the approved write set.
    MarkApplied(WorkflowRunArgs),
    /// Run a verification command directly, without a shell.
    Check(WorkflowCheckArgs),
    /// Repeat the original effective scan and compare evidence IDs.
    Rescan(WorkflowRunArgs),
    /// Confirm proposed issue lineage or explicitly record selected issues as remediated.
    ConfirmLineage(WorkflowConfirmLineageArgs),
    /// Finish as verified, failed, or needs_input from recorded evidence.
    Finish(WorkflowRunArgs),
}

#[derive(Debug, Clone, Args)]
pub struct WorkflowStartArgs {
    #[command(flatten)]
    pub scan: ScanArgs,

    /// Exact directory for the new workflow run.
    #[arg(long)]
    pub run_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct WorkflowRunArgs {
    /// Workflow run directory containing run.json.
    pub run: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct WorkflowSelectArgs {
    /// Workflow run directory containing run.json.
    pub run: PathBuf,

    /// Stable schema 24 issue ID. Repeat to select multiple issues.
    #[arg(long = "issue", required = true)]
    pub issues: Vec<String>,

    /// User-visible outcome for the selected work.
    #[arg(long)]
    pub goal: String,
}

#[derive(Debug, Clone, Args)]
pub struct WorkflowConfirmLineageArgs {
    /// Workflow run directory containing rescan.json.
    pub run: PathBuf,

    /// Lineage candidate ID from rescan.json. Repeat to confirm multiple candidates.
    #[arg(long = "candidate")]
    pub candidates: Vec<String>,

    /// Selected issue that disappeared without an observable successor.
    #[arg(long = "remediated")]
    pub remediated: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCheckKind {
    Format,
    Build,
    Test,
    Custom,
}

#[derive(Debug, Clone, Args)]
pub struct WorkflowCheckArgs {
    /// Workflow run directory containing run.json.
    pub run: PathBuf,

    /// Verification check category.
    #[arg(long, value_enum)]
    pub kind: WorkflowCheckKind,

    /// Kill the command after this many seconds.
    #[arg(long, default_value_t = 900)]
    pub timeout_seconds: u64,

    /// Program and arguments. The program is executed directly, without a shell.
    #[arg(last = true, required = true, num_args = 1..)]
    pub command: Vec<String>,
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
