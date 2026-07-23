use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use reforge_schema::{Issue, Report};

mod artifact;
use artifact::*;
mod checks;
mod storage;
use storage::*;
mod validation;
mod verification;

const DEFAULT_RUN: &str = ".reforge-workflow";

#[derive(Parser)]
#[command(
    name = "reforge-workflow",
    version,
    about = "Manage a five-stage, approval-gated Reforge workflow"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Consume schema 26 reports and create an Imported run.
    Start(StartArgs),
    /// Import and validate a complete plan artifact.
    Plan(PlanArgs),
    /// Freeze the plan hash, write set, and workspace snapshot.
    Approve(RunArgs),
    /// Record changes after enforcing the approved write set.
    MarkApplied(RunArgs),
    /// Execute a check directly, without a shell.
    Check(CheckArgs),
    /// Consume fresh reports and finish the run.
    Verify(VerifyArgs),
    /// Show phase and artifact summary.
    Status(RunArgs),
    /// Validate all current artifacts without mutation.
    Validate(RunArgs),
}

#[derive(Args)]
struct StartArgs {
    #[arg(long = "report", required = true)]
    reports: Vec<PathBuf>,
    #[arg(long)]
    goal: String,
    #[arg(long, default_value = DEFAULT_RUN)]
    run: PathBuf,
}

#[derive(Args)]
struct PlanArgs {
    #[arg(long)]
    artifact: PathBuf,
    #[arg(long, default_value = DEFAULT_RUN)]
    run: PathBuf,
}
#[derive(Args)]
struct RunArgs {
    #[arg(long, default_value = DEFAULT_RUN)]
    run: PathBuf,
}
#[derive(Args)]
struct VerifyArgs {
    #[arg(long = "report", required = true)]
    reports: Vec<PathBuf>,
    #[arg(long, default_value = DEFAULT_RUN)]
    run: PathBuf,
}

#[derive(Args)]
struct CheckArgs {
    #[arg(long, default_value = DEFAULT_RUN)]
    run: PathBuf,
    #[arg(long, value_enum)]
    kind: CheckKind,
    #[arg(long, default_value_t = 900)]
    timeout_seconds: u64,
    #[arg(last = true, required = true, num_args = 1..)]
    command: Vec<String>,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Start(args) => start(args),
        Command::Plan(args) => import_plan(args),
        Command::Approve(args) => approve(args),
        Command::MarkApplied(args) => mark_applied(args),
        Command::Check(args) => check(args),
        Command::Verify(args) => verify(args),
        Command::Status(args) => status(args),
        Command::Validate(args) => validate_run(args),
    }
}

fn start(args: StartArgs) -> Result<()> {
    if args.run.exists() {
        bail!("run directory {} already exists", args.run.display());
    }
    let reports = load_and_merge(&args.reports)?;
    let first = reports.first().context("at least one report is required")?;
    std::fs::create_dir_all(args.run.join("reports"))?;
    let mut stored = Vec::new();
    for (index, report) in reports.iter().enumerate() {
        let name = format!("initial-{index}-{}.json", safe_name(&report.producer.name));
        copy_report(&args.run.join("reports").join(&name), report)?;
        stored.push(name);
    }
    let artifact = RunArtifact {
        artifact_schema_version: SCHEMA_VERSION,
        phase: Phase::Imported,
        goal: args.goal,
        workspace_root: first.target.root.clone(),
        workspace_identity: first.target.workspace_identity.clone(),
        source_revision: first.target.source_revision.clone(),
        initial_reports: stored,
        initial_issue_ids: reports
            .iter()
            .flat_map(|report| report.issues.iter().map(|issue| issue.id.clone()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        initial_producers: reports
            .iter()
            .map(|report| report.producer.name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    };
    write_json(&args.run.join("run.json"), &artifact)?;
    write_json(
        &args.run.join("checks.json"),
        &ChecksArtifact {
            artifact_schema_version: SCHEMA_VERSION,
            checks: vec![],
        },
    )?;
    println!("Created Imported workflow at {}", args.run.display());
    Ok(())
}

fn import_plan(args: PlanArgs) -> Result<()> {
    let mut run = load_run(&args.run)?;
    validation::require_phase(&run, Phase::Imported)?;
    let plan: PlanArtifact = read_json(&args.artifact)?;
    validate_artifact_version(plan.artifact_schema_version)?;
    validation::plan(&run, &plan)?;
    write_json(&args.run.join("plan.json"), &plan)?;
    run.phase = Phase::Planned;
    write_json(&args.run.join("run.json"), &run)?;
    println!("Plan imported; workflow is Planned.");
    Ok(())
}

fn approve(args: RunArgs) -> Result<()> {
    let mut run = load_run(&args.run)?;
    validation::require_phase(&run, Phase::Planned)?;
    let plan: PlanArtifact = read_json(&args.run.join("plan.json"))?;
    validation::plan(&run, &plan)?;
    let root = Path::new(&run.workspace_root);
    let approval = ApprovalArtifact {
        artifact_schema_version: SCHEMA_VERSION,
        plan_hash: json_hash(&plan)?,
        write_set: plan
            .write_set
            .iter()
            .map(|path| normalize_write_path(path))
            .collect::<Result<_>>()?,
        workspace_snapshot: snapshot(root, &args.run)?,
    };
    write_json(&args.run.join("approval.json"), &approval)?;
    run.phase = Phase::Approved;
    write_json(&args.run.join("run.json"), &run)?;
    println!("Plan approved; workflow is Approved.");
    Ok(())
}

fn mark_applied(args: RunArgs) -> Result<()> {
    let mut run = load_run(&args.run)?;
    validation::require_phase(&run, Phase::Approved)?;
    let plan: PlanArtifact = read_json(&args.run.join("plan.json"))?;
    let approval: ApprovalArtifact = read_json(&args.run.join("approval.json"))?;
    if approval.plan_hash != json_hash(&plan)? {
        bail!("plan.json changed after approval");
    }
    let current = snapshot(Path::new(&run.workspace_root), &args.run)?;
    let changed = changed_paths(&approval.workspace_snapshot, &current);
    enforce_write_set(&changed, &approval.write_set)?;
    write_json(
        &args.run.join("application.json"),
        &ApplicationArtifact {
            artifact_schema_version: SCHEMA_VERSION,
            changed_paths: changed,
            workspace_snapshot: current,
        },
    )?;
    run.phase = Phase::Applied;
    write_json(&args.run.join("run.json"), &run)?;
    println!("Application recorded; workflow is Applied.");
    Ok(())
}

fn check(args: CheckArgs) -> Result<()> {
    let run = load_run(&args.run)?;
    validation::require_phase(&run, Phase::Applied)?;
    let result = checks::execute(
        args.kind,
        args.timeout_seconds,
        &args.command,
        Path::new(&run.workspace_root),
    )?;
    let success = result.success;
    let mut checks: ChecksArtifact = read_json(&args.run.join("checks.json"))?;
    checks.checks.push(result);
    write_json(&args.run.join("checks.json"), &checks)?;
    if !success {
        bail!("check failed");
    }
    println!("Check passed.");
    Ok(())
}

fn verify(args: VerifyArgs) -> Result<()> {
    let mut run = load_run(&args.run)?;
    validation::require_phase(&run, Phase::Applied)?;
    validate_application(&run, &args.run)?;
    let fresh = load_and_merge(&args.reports)?;
    for report in &fresh {
        if report.target.workspace_identity != run.workspace_identity {
            bail!("verification report workspace identity mismatch");
        }
    }
    let initial = load_stored_reports(&args.run, &run.initial_reports)?;
    let plan: PlanArtifact = read_json(&args.run.join("plan.json"))?;
    let checks: ChecksArtifact = read_json(&args.run.join("checks.json"))?;
    let changes = verification::issue_changes(&run, &plan, &fresh);
    let mut failed = Vec::new();
    let mut needs_input = Vec::new();
    if !changes.remaining.is_empty() {
        failed.push(format!(
            "{} selected issue(s) remain",
            changes.remaining.len()
        ));
    }
    if !changes.new_issues.is_empty() {
        failed.push(format!(
            "{} new issue(s) appeared",
            changes.new_issues.len()
        ));
    }
    checks::evaluate(&plan, &checks, &mut failed, &mut needs_input);
    verification::evaluate_coverage(&initial, &fresh, &plan, &mut failed, &mut needs_input);
    needs_input.extend(verification::missing_producers(&run, &fresh));
    let (outcome, reasons) = verification::outcome(failed, needs_input);
    let mut stored = Vec::new();
    for (index, report) in fresh.iter().enumerate() {
        let name = format!(
            "verification-{index}-{}.json",
            safe_name(&report.producer.name)
        );
        copy_report(&args.run.join("reports").join(&name), report)?;
        stored.push(name);
    }
    write_json(
        &args.run.join("verification.json"),
        &VerificationArtifact {
            artifact_schema_version: SCHEMA_VERSION,
            outcome,
            reasons: reasons.clone(),
            reports: stored,
            resolved_issue_ids: changes.resolved,
            remaining_issue_ids: changes.remaining,
            new_issue_ids: changes.new_issues,
        },
    )?;
    run.phase = Phase::Verified;
    write_json(&args.run.join("run.json"), &run)?;
    println!("Verification outcome: {:?}", outcome);
    for reason in reasons {
        println!("  - {reason}");
    }
    Ok(())
}

fn status(args: RunArgs) -> Result<()> {
    let run = load_run(&args.run)?;
    println!(
        "Phase: {:?}\nGoal: {}\nWorkspace: {}\nInitial issues: {}",
        run.phase,
        run.goal,
        run.workspace_identity,
        run.initial_issue_ids.len()
    );
    if run.phase == Phase::Verified {
        let verification: VerificationArtifact = read_json(&args.run.join("verification.json"))?;
        println!("Outcome: {:?}", verification.outcome);
    }
    Ok(())
}

fn validate_run(args: RunArgs) -> Result<()> {
    let run = load_run(&args.run)?;
    let _ = load_stored_reports(&args.run, &run.initial_reports)?;
    if !matches!(run.phase, Phase::Imported) {
        let plan: PlanArtifact = read_json(&args.run.join("plan.json"))?;
        validation::plan(&run, &plan)?;
    }
    if matches!(
        run.phase,
        Phase::Approved | Phase::Applied | Phase::Verified
    ) {
        let _: ApprovalArtifact = read_json(&args.run.join("approval.json"))?;
    }
    if matches!(run.phase, Phase::Applied | Phase::Verified) {
        let _: ApplicationArtifact = read_json(&args.run.join("application.json"))?;
    }
    if run.phase == Phase::Verified {
        let _: VerificationArtifact = read_json(&args.run.join("verification.json"))?;
    }
    let _: ChecksArtifact = read_json(&args.run.join("checks.json"))?;
    println!("Workflow artifacts are valid (phase {:?}).", run.phase);
    Ok(())
}

fn load_and_merge(paths: &[PathBuf]) -> Result<Vec<Report>> {
    let mut reports = Vec::new();
    let mut identity = None;
    let mut revision: Option<Option<String>> = None;
    let mut issues = BTreeMap::<String, Issue>::new();
    for path in paths {
        let report = load_report(path)?;
        if identity
            .as_ref()
            .is_some_and(|value| value != &report.target.workspace_identity)
        {
            bail!("reports have different workspace identities");
        }
        if revision
            .as_ref()
            .is_some_and(|value| value != &report.target.source_revision)
        {
            bail!("reports have different source revisions");
        }
        identity = Some(report.target.workspace_identity.clone());
        revision = Some(report.target.source_revision.clone());
        for issue in &report.issues {
            if let Some(previous) = issues.insert(issue.id.clone(), issue.clone())
                && previous != *issue
            {
                bail!("issue {} has conflicting contents across reports", issue.id);
            }
        }
        reports.push(report);
    }
    Ok(reports)
}

fn load_report(path: &Path) -> Result<Report> {
    reforge_output::load_report(path)
}

fn load_stored_reports(run: &Path, names: &[String]) -> Result<Vec<Report>> {
    names
        .iter()
        .map(|name| load_report(&run.join("reports").join(name)))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use reforge_schema::{
        AnalysisCoverage, CoverageStatus, Evidence, Producer, Subject, SuppressionSummary, Target,
    };
    #[test]
    fn rejects_path_escape() {
        assert!(normalize_write_path("../outside").is_err());
        assert!(normalize_write_path("/absolute").is_err());
    }
    #[test]
    fn write_set_accepts_files_and_children() {
        assert!(path_allowed("src/lib.rs", "src"));
        assert!(!path_allowed("src-old/lib.rs", "src"));
    }

    #[test]
    fn workflow_guide_names_the_current_artifact_schema() {
        let guide = include_str!("../../../docs/agent-workflows.md");
        assert!(guide.contains(&format!("Artifact v{SCHEMA_VERSION}")));
    }

    fn report(workspace: &str, message: &str, status: CoverageStatus) -> Report {
        let issue = Issue::new(
            "codebase",
            "reforge.codebase.large_file",
            Subject::File {
                path: "src/lib.rs".into(),
            },
            ("Large file", "Split it"),
            vec![Evidence::new(
                "reforge.codebase.large_file",
                "src/lib.rs",
                message,
            )],
        );
        Report::new(
            Producer {
                name: "reforge.analyze".into(),
                version: "test".into(),
                revision: None,
            },
            Target {
                root: "/tmp/work".into(),
                workspace_identity: workspace.into(),
                source_revision: Some("revision".into()),
            },
            SuppressionSummary::default(),
            BTreeMap::from([(
                "codebase".into(),
                AnalysisCoverage {
                    status,
                    scanned_files: 1,
                    languages: BTreeMap::new(),
                    rules: BTreeMap::new(),
                    limitations: Vec::new(),
                },
            )]),
            vec![issue],
        )
    }

    #[test]
    fn coverage_downgrade_fails_verification() {
        let initial = report("rw5-one", "before", CoverageStatus::Observed);
        let fresh = report("rw5-one", "before", CoverageStatus::Partial);
        let plan = PlanArtifact {
            artifact_schema_version: SCHEMA_VERSION,
            goal: "goal".into(),
            selected_issue_ids: vec![initial.issues[0].id.clone()],
            notes: vec![],
            changes: vec![],
            write_set: vec!["src/lib.rs".into()],
            required_checks: vec![RequiredCheck {
                kind: CheckKind::Test,
                description: String::new(),
            }],
        };
        let mut failed = Vec::new();
        let mut needs_input = Vec::new();
        verification::evaluate_coverage(&[initial], &[fresh], &plan, &mut failed, &mut needs_input);
        assert!(failed.iter().any(|reason| reason.contains("degraded")));
    }

    #[test]
    fn duplicate_issue_content_must_match() {
        let root =
            std::env::temp_dir().join(format!("reforge-workflow-merge-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let first = root.join("first.json");
        let second = root.join("second.json");
        write_json(
            &first,
            &report("rw5-one", "first", CoverageStatus::Observed),
        )
        .unwrap();
        write_json(
            &second,
            &report("rw5-one", "conflict", CoverageStatus::Observed),
        )
        .unwrap();
        assert!(load_and_merge(&[first, second]).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn combined_analyze_coverage_is_consumed_as_one_producer() {
        let mut initial = report("rw5-one", "before", CoverageStatus::Observed);
        initial.coverage.insert(
            "dataflow".into(),
            AnalysisCoverage {
                status: CoverageStatus::Partial,
                scanned_files: 3,
                languages: BTreeMap::new(),
                rules: BTreeMap::new(),
                limitations: Vec::new(),
            },
        );
        let fresh = initial.clone();
        let plan = PlanArtifact {
            artifact_schema_version: SCHEMA_VERSION,
            goal: "goal".into(),
            selected_issue_ids: vec![initial.issues[0].id.clone()],
            notes: vec![],
            changes: vec![],
            write_set: vec!["src/lib.rs".into()],
            required_checks: vec![RequiredCheck {
                kind: CheckKind::Test,
                description: String::new(),
            }],
        };
        let mut failed = Vec::new();
        let mut needs_input = Vec::new();
        verification::evaluate_coverage(&[initial], &[fresh], &plan, &mut failed, &mut needs_input);
        assert!(failed.is_empty());
        assert!(needs_input.is_empty());
    }
}
