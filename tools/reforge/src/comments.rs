use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use reforge_engine::api::{Config, comments as engine};

use super::{load_config, validate_config};

#[derive(Debug, Args)]
pub(super) struct CommentsCommand {
    #[command(subcommand)]
    command: CommentCommand,
}

#[derive(Debug, Subcommand)]
enum CommentCommand {
    /// Extract Rust/JS/TS comments with context, protections and review hints.
    Pick(PickCommand),
    /// Preview selected removals; write sources only with --apply.
    Clean(CleanCommand),
}

#[derive(Debug, Args)]
struct SourceArgs {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    include_hidden: bool,
    #[arg(long)]
    include_generated: bool,
    #[arg(long)]
    no_gitignore: bool,
    #[arg(long)]
    exclude_tests: bool,
    /// Exclude a relative path or directory prefix. Repeat as needed.
    #[arg(long = "ignore-path")]
    ignore_paths: Vec<String>,
}

impl SourceArgs {
    fn config(&self) -> Result<Config> {
        let (_, value) = load_config(self.config.as_deref(), &self.path)?;
        validate_config(&value)?;
        let mut config = Config::parse_toml(&toml::to_string(&value)?)?;
        config.apply_scope_overrides(
            self.include_hidden,
            self.include_generated,
            self.no_gitignore,
            self.exclude_tests,
            &self.ignore_paths,
        );
        Ok(config)
    }
}

#[derive(Debug, Args)]
struct SelectionArgs {
    /// Select exact comment IDs from pick. IDs become stale when a file changes.
    #[arg(long)]
    id: Vec<String>,
    /// Match a case-sensitive literal substring. Repeated values are ORed.
    #[arg(long)]
    contains: Vec<String>,
    /// Match the complete trimmed comment body without comment delimiters.
    #[arg(long)]
    text: Vec<String>,
    #[arg(long, value_enum)]
    kind: Option<KindArg>,
    /// Only show review candidates. Clean still requires a text or ID selector.
    #[arg(long)]
    candidates: bool,
}

impl SelectionArgs {
    fn selection(&self) -> engine::Selection {
        engine::Selection {
            ids: self.id.clone(),
            contains: self.contains.clone(),
            text: self.text.clone(),
            kind: self.kind.map(|kind| match kind {
                KindArg::Line => engine::CommentKind::Line,
                KindArg::Block => engine::CommentKind::Block,
                KindArg::Documentation => engine::CommentKind::Documentation,
            }),
            candidates_only: self.candidates,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum KindArg {
    Line,
    Block,
    Documentation,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PickFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CleanFormat {
    Diff,
    Json,
}

#[derive(Debug, Args)]
struct PickCommand {
    #[command(flatten)]
    source: SourceArgs,
    #[command(flatten)]
    selection: SelectionArgs,
    #[arg(long, value_enum, default_value_t = PickFormat::Human)]
    output: PickFormat,
    #[arg(long)]
    output_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct CleanCommand {
    #[command(flatten)]
    source: SourceArgs,
    #[command(flatten)]
    selection: SelectionArgs,
    /// Remove every comment in scope, including documentation, licenses and tool directives.
    #[arg(long, conflicts_with_all = ["id", "contains", "text", "kind", "candidates", "plan"])]
    all: bool,
    /// Load a previously generated JSON cleanup plan; do not rescan/select.
    #[arg(long, conflicts_with_all = ["id", "contains", "text", "kind", "candidates", "config", "include_hidden", "include_generated", "no_gitignore", "exclude_tests", "ignore_paths"])]
    plan: Option<PathBuf>,
    /// Apply validated removals to the original files.
    #[arg(long)]
    apply: bool,
    #[arg(long, value_enum, default_value_t = CleanFormat::Diff)]
    output: CleanFormat,
    #[arg(long)]
    output_file: Option<PathBuf>,
}

pub(super) fn run(command: CommentsCommand) -> Result<()> {
    match command.command {
        CommentCommand::Pick(command) => {
            let inventory = engine::pick(
                &command.source.path,
                &command.source.config()?,
                &command.selection.selection(),
            )?;
            let output = match command.output {
                PickFormat::Json => serde_json::to_string_pretty(&inventory)? + "\n",
                PickFormat::Human => render_inventory(&inventory),
            };
            // An export must never replace any source, including filtered comments.
            write_output(
                command.output_file.as_deref(),
                &output,
                &command.source.path,
            )?;
        }
        CommentCommand::Clean(command) => clean(command)?,
    }
    Ok(())
}

fn clean(command: CleanCommand) -> Result<()> {
    let plan = if let Some(path) = &command.plan {
        load_plan(path, &command.source.path)?
    } else if command.all {
        engine::prepare_all(&command.source.path, &command.source.config()?)?
    } else {
        engine::prepare(
            &command.source.path,
            &command.source.config()?,
            &command.selection.selection(),
        )?
    };
    if plan.removal_mode == engine::RemovalMode::All {
        eprintln!("All-comments mode: includes documentation, licenses and tool directives.");
    }
    let output = match command.output {
        CleanFormat::Json => serde_json::to_string_pretty(&plan)? + "\n",
        CleanFormat::Diff => engine::diff(&plan)?,
    };
    write_output(
        command.output_file.as_deref(),
        &output,
        &command.source.path,
    )?;
    let count = if command.apply {
        engine::apply(&plan)?
    } else {
        plan.files.len()
    };
    eprintln!(
        "{} {count} file(s); selected {} comment group(s), protected {}, skipped {} file(s).",
        if command.apply {
            "Updated"
        } else {
            "Previewed"
        },
        plan.selected_comments,
        plan.protected_comments,
        plan.skipped.len()
    );
    for skipped in &plan.skipped {
        eprintln!("Skipped {}: {}", skipped.path, skipped.reason);
    }
    Ok(())
}

fn load_plan(path: &Path, root: &Path) -> Result<engine::CleanPlan> {
    let plan: engine::CleanPlan = serde_json::from_slice(&std::fs::read(path)?)?;
    let requested = root.canonicalize()?;
    let expected = if requested.is_file() {
        requested.parent().context("source has no parent")?
    } else {
        &requested
    };
    if expected != plan.root {
        bail!("plan root differs from requested path; pass the original workspace path");
    }
    if requested.is_file()
        && (plan.files.len() != 1 || plan.root.join(&plan.files[0].path) != requested)
    {
        bail!("plan contains files outside the requested source file");
    }
    engine::validate(&plan)?;
    Ok(plan)
}

fn render_inventory(inventory: &engine::Inventory) -> String {
    let mut output = format!(
        "{} comment group(s) selected / {} total; {} files parsed; {} skipped\n",
        inventory.comments.len(),
        inventory.total_comments,
        inventory.scanned_files,
        inventory.skipped.len()
    );
    for comment in &inventory.comments {
        output.push_str(&format!(
            "\n{}:{}-{} {:?}{}\n  ID: {}\n",
            comment.path,
            comment.line,
            comment.end_line,
            comment.kind,
            if comment.protected {
                " [protected]"
            } else {
                ""
            },
            comment.id
        ));
        if let Some(symbol) = &comment.symbol {
            output.push_str(&format!("  Symbol: {symbol}\n"));
        }
        if !comment.reasons.is_empty() {
            output.push_str(&format!("  Keep: {}\n", comment.reasons.join(", ")));
        }
        if !comment.suggestions.is_empty() {
            output.push_str(&format!("  Review: {}\n", comment.suggestions.join(", ")));
        }
        for (index, line) in comment.context.lines().enumerate() {
            output.push_str(&format!(
                "  {} | {line}\n",
                comment.context_start_line + index
            ));
        }
    }
    for skipped in &inventory.skipped {
        output.push_str(&format!("Skipped {}: {}\n", skipped.path, skipped.reason));
    }
    output
}

fn write_output(path: Option<&Path>, text: &str, source: &Path) -> Result<()> {
    if let Some(path) = path {
        // Output artifacts have dedicated extensions and cannot overwrite existing
        // files. This also prevents symlink/hard-link aliases of source files.
        if !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("json" | "diff" | "patch" | "txt")
        ) {
            bail!("comment output must use .json, .diff, .patch or .txt");
        }
        if source.is_file() && path == source {
            bail!("output cannot replace source");
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .with_context(|| {
                format!(
                    "cannot create {}; output must not already exist",
                    path.display()
                )
            })?;
        file.write_all(text.as_bytes())?;
    } else {
        std::io::stdout().lock().write_all(text.as_bytes())?;
    }
    Ok(())
}
