fn start(args: WorkflowStartArgs) -> Result<()> {
    let root = args
        .scan
        .path
        .canonicalize()
        .with_context(|| format!("failed to resolve path {}", args.scan.path.display()))?;
    let workspace_root = if root.is_dir() {
        root.clone()
    } else {
        root.parent()
            .context("scan file has no parent directory")?
            .to_path_buf()
    };
    let effective = scan::effective_config_output(&args.scan, &root)?;
    let effective_config = serde_json::to_value(&effective)?;
    let config_path = scan::validate_config(args.scan.config.as_deref(), &root)?;
    let config_fingerprint = config_fingerprint(&effective_config, config_path.as_deref())?;

    let mut progress = NoopProgress;
    let report = scan::scan_report(&args.scan, &mut progress)?;
    ensure!(
        report.schema_version == SCAN_REPORT_SCHEMA_VERSION,
        "workflow start requires schema {}",
        SCAN_REPORT_SCHEMA_VERSION
    );
    let report_value = serde_json::to_value(&report)?;
    let report_fingerprint = fingerprint_json(&report_value);
    let source_fingerprint = snapshot_fingerprint(&workspace_snapshot(&workspace_root, None)?)?;
    let scan_command =
        effective_scan_command(&root, &effective_config, &args.scan, config_path.as_deref())?;

    let run_dir = resolve_new_run_dir(
        args.run_dir.as_deref(),
        &workspace_root,
        &report_fingerprint,
    )?;
    ensure!(
        !run_dir.join("run.json").exists(),
        "workflow run already exists at {}",
        run_dir.display()
    );
    fs::create_dir_all(run_dir.join("investigations"))
        .with_context(|| format!("failed to create run directory {}", run_dir.display()))?;

    let now = epoch_ms();
    let run = RunArtifact {
        artifact_schema_version: ARTIFACT_SCHEMA_VERSION,
        reforge_version: env!("CARGO_PKG_VERSION").to_string(),
        report_schema_version: report.schema_version,
        target_root: portable_path(&workspace_root),
        phase: WorkflowPhase::Scanned,
        scan_command,
        effective_config,
        report_fingerprint,
        config_fingerprint,
        source_fingerprint,
        created_at_epoch_ms: now,
        updated_at_epoch_ms: now,
    };
    atomic_write_json(&run_dir.join("scan.json"), &report, false)?;
    atomic_write_json(&run_dir.join("run.json"), &run, false)?;
    println!("{}", run_dir.display());
    Ok(())
}

fn select(args: WorkflowSelectArgs) -> Result<()> {
    let mut context = load_context(&args.run)?;
    require_phase(&context.run, &[WorkflowPhase::Scanned])?;
    let report: ScanReport = read_json(&context.dir.join("scan.json"))?;
    validate_report_fingerprint(&context.run, &report)?;

    let mut issue_ids = args
        .issues
        .into_iter()
        .map(IssueKey::from)
        .collect::<Vec<_>>();
    ensure!(
        !args.goal.trim().is_empty(),
        "workflow goal must not be empty"
    );
    ensure_unique(&issue_ids, "selected issue ID")?;
    let report_ids = report
        .issues
        .iter()
        .map(|issue| issue.id.clone())
        .collect::<BTreeSet<_>>();
    for issue_id in &issue_ids {
        ensure!(
            report_ids.contains(issue_id),
            "issue ID {} is not present in scan.json",
            issue_id
        );
    }
    issue_ids.sort();
    let selection = SelectionArtifact {
        artifact_schema_version: ARTIFACT_SCHEMA_VERSION,
        report_fingerprint: context.run.report_fingerprint.clone(),
        issue_ids,
        goal: args.goal,
        selected_at_epoch_ms: epoch_ms(),
    };
    atomic_write_json(&context.dir.join("selection.json"), &selection, false)?;
    update_phase(&mut context, WorkflowPhase::Selected)
}

fn status(args: WorkflowRunArgs) -> Result<()> {
    let context = load_context(&args.run)?;
    let artifacts = [
        "scan.json",
        "selection.json",
        "plan.json",
        "approval.json",
        "application.json",
        "rescan.json",
        "verification.json",
    ]
    .into_iter()
    .map(|name| (name, context.dir.join(name).is_file()))
    .collect::<BTreeMap<_, _>>();
    let investigations = context
        .dir
        .join("investigations")
        .read_dir()
        .map(|entries| {
            entries
                .filter_map(std::result::Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
                .count()
        })
        .unwrap_or(0);
    let value = serde_json::json!({
        "phase": context.run.phase,
        "target_root": context.run.target_root,
        "report_fingerprint": context.run.report_fingerprint,
        "artifacts": artifacts,
        "investigation_count": investigations,
    });
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn validate_command(args: WorkflowRunArgs) -> Result<()> {
    let context = load_context(&args.run)?;
    validate_run(&context)?;
    println!("valid: {:?}", context.run.phase);
    Ok(())
}

fn advance(args: WorkflowRunArgs) -> Result<()> {
    let mut context = load_context(&args.run)?;
    match context.run.phase {
        WorkflowPhase::Selected => {
            let investigations = validate_investigations(&context)?;
            if investigations
                .iter()
                .any(|item| item.status == InvestigationStatus::Failed)
            {
                update_phase(&mut context, WorkflowPhase::Failed)
            } else if investigations
                .iter()
                .any(|item| item.status == InvestigationStatus::NeedsInput)
            {
                update_phase(&mut context, WorkflowPhase::NeedsInput)
            } else {
                update_phase(&mut context, WorkflowPhase::Investigated)
            }
        }
        WorkflowPhase::Investigated => {
            validate_plan(&context)?;
            update_phase(&mut context, WorkflowPhase::Planned)
        }
        phase => bail!("cannot advance workflow from {phase:?}"),
    }
}

fn approve(args: WorkflowRunArgs) -> Result<()> {
    let mut context = load_context(&args.run)?;
    require_phase(&context.run, &[WorkflowPhase::Planned])?;
    let plan = validate_plan(&context)?;
    let plan_value = serde_json::to_value(&plan)?;
    let plan_fingerprint = fingerprint_json(&plan_value);
    let snapshot = workspace_snapshot(&context.root, Some(&context.dir))?;
    let approval = ApprovalArtifact {
        artifact_schema_version: ARTIFACT_SCHEMA_VERSION,
        report_fingerprint: context.run.report_fingerprint.clone(),
        plan_fingerprint,
        write_set: plan.write_set,
        workspace_snapshot_fingerprint: snapshot_fingerprint(&snapshot)?,
        workspace_snapshot: snapshot,
        approved_at_epoch_ms: epoch_ms(),
    };
    atomic_write_json(&context.dir.join("approval.json"), &approval, false)?;
    update_phase(&mut context, WorkflowPhase::Approved)
}

fn mark_applied(args: WorkflowRunArgs) -> Result<()> {
    let mut context = load_context(&args.run)?;
    require_phase(&context.run, &[WorkflowPhase::Approved])?;
    let approval: ApprovalArtifact = read_json(&context.dir.join("approval.json"))?;
    validate_approval(&context, &approval)?;
    let current = workspace_snapshot(&context.root, Some(&context.dir))?;
    let changes = snapshot_changes(&approval.workspace_snapshot, &current);
    let approved = approval.write_set.iter().cloned().collect::<BTreeSet<_>>();
    let outside = changes
        .iter()
        .filter(|change| !approved.contains(&change.path))
        .map(|change| change.path.clone())
        .collect::<Vec<_>>();
    ensure!(
        outside.is_empty(),
        "workspace changed outside the approved write set: {}",
        outside.join(", ")
    );
    let application = ApplicationArtifact {
        artifact_schema_version: ARTIFACT_SCHEMA_VERSION,
        plan_fingerprint: approval.plan_fingerprint,
        changed_files: changes,
        workspace_snapshot_fingerprint: snapshot_fingerprint(&current)?,
        applied_at_epoch_ms: epoch_ms(),
    };
    atomic_write_json(&context.dir.join("application.json"), &application, false)?;
    update_phase(&mut context, WorkflowPhase::Applied)
}

fn check(args: WorkflowCheckArgs) -> Result<()> {
    let mut context = load_context(&args.run)?;
    require_phase(
        &context.run,
        &[
            WorkflowPhase::Applied,
            WorkflowPhase::Failed,
            WorkflowPhase::NeedsInput,
        ],
    )?;
    ensure!(!args.command.is_empty(), "check requires a program");
    let plan: PlanArtifact = read_json(&context.dir.join("plan.json"))?;
    let program = args.command[0].clone();
    let command_args = args.command[1..].to_vec();
    let declared = plan.checks.iter().any(|check| {
        check.kind == args.kind && check.program == program && check.args == command_args
    });
    let record = run_check(CheckExecution {
        kind: args.kind,
        program: &program,
        args: &command_args,
        declared,
        root: &context.root,
        timeout: Duration::from_secs(args.timeout_seconds),
    });
    let mut verification = load_verification(&context.dir)?;
    verification.checks.push(record.clone());
    verification.result = None;
    verification.reasons.clear();
    verification.finished_at_epoch_ms = None;
    atomic_write_json(&context.dir.join("verification.json"), &verification, true)?;
    if !record.success {
        update_phase(&mut context, WorkflowPhase::Failed)?;
        bail!("check failed: {}", record.output_summary);
    }
    println!("check passed in {} ms", record.duration_ms);
    Ok(())
}

fn rescan(args: WorkflowRunArgs) -> Result<()> {
    let context = load_context(&args.run)?;
    require_phase(
        &context.run,
        &[
            WorkflowPhase::Applied,
            WorkflowPhase::Failed,
            WorkflowPhase::NeedsInput,
        ],
    )?;
    ensure!(
        !context.dir.join("rescan.json").exists(),
        "rescan.json already exists; workflow artifacts are immutable"
    );
    let scan_args = parse_stored_scan_command(&context.run.scan_command)?;
    validate_config_fingerprint(&context, &scan_args)?;
    let mut progress = NoopProgress;
    let current = scan::scan_report(&scan_args, &mut progress)?;
    let original: ScanReport = read_json(&context.dir.join("scan.json"))?;
    let selection: SelectionArtifact = read_json(&context.dir.join("selection.json"))?;
    let selected_issues = selection.issue_ids.iter().cloned().collect::<BTreeSet<_>>();
    let selected_findings = original
        .issues
        .iter()
        .filter(|issue| selected_issues.contains(&issue.id))
        .flat_map(|issue| issue.finding_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    let current_ids = current
        .findings
        .iter()
        .map(|finding| finding.id.clone())
        .collect::<BTreeSet<_>>();
    let original_ids = original
        .findings
        .iter()
        .map(|finding| finding.id.clone())
        .collect::<BTreeSet<_>>();
    let unobservable_kinds = unobservable_selected_kinds(&original, &current, &selected_findings);
    let mut removed = Vec::new();
    let mut still = Vec::new();
    let mut unobservable = Vec::new();
    for id in &selected_findings {
        if current_ids.contains(id) {
            still.push(id.clone());
        } else if original
            .findings
            .iter()
            .find(|finding| &finding.id == id)
            .is_some_and(|finding| unobservable_kinds.contains(&finding.kind))
        {
            unobservable.push(id.clone());
        } else {
            removed.push(id.clone());
        }
    }
    let mut new_evidence = current_ids
        .difference(&original_ids)
        .cloned()
        .collect::<Vec<_>>();
    removed.sort();
    still.sort();
    unobservable.sort();
    new_evidence.sort();
    let limitations = selected_coverage_limitations(&current, &unobservable_kinds);
    let artifact = RescanArtifact {
        artifact_schema_version: ARTIFACT_SCHEMA_VERSION,
        original_report_fingerprint: context.run.report_fingerprint.clone(),
        rescan_report_fingerprint: fingerprint_json(&serde_json::to_value(&current)?),
        selected_evidence_removed: removed,
        selected_evidence_still_present: still,
        new_evidence,
        unobservable,
        coverage_limitations: limitations,
        rescanned_at_epoch_ms: epoch_ms(),
    };
    atomic_write_json(&context.dir.join("rescan.json"), &artifact, false)?;
    Ok(())
}

fn finish(args: WorkflowRunArgs) -> Result<()> {
    let mut context = load_context(&args.run)?;
    require_phase(
        &context.run,
        &[
            WorkflowPhase::Applied,
            WorkflowPhase::Failed,
            WorkflowPhase::NeedsInput,
        ],
    )?;
    let approval: ApprovalArtifact = read_json(&context.dir.join("approval.json"))?;
    let application: ApplicationArtifact = read_json(&context.dir.join("application.json"))?;
    let plan: PlanArtifact = read_json(&context.dir.join("plan.json"))?;
    let rescan: RescanArtifact = read_json(&context.dir.join("rescan.json"))?;
    validate_approval(&context, &approval)?;
    ensure!(
        application.plan_fingerprint == approval.plan_fingerprint,
        "application plan fingerprint does not match approval"
    );
    validate_final_workspace(&context, &approval)?;

    let mut verification = load_verification(&context.dir)?;
    let mut reasons = required_check_failures(&plan, &verification);
    let result = finish_result(&verification, &rescan, &mut reasons);
    verification.result = Some(result);
    verification.reasons = reasons;
    verification.finished_at_epoch_ms = Some(epoch_ms());
    atomic_write_json(&context.dir.join("verification.json"), &verification, true)?;
    update_phase(&mut context, result)
}

fn validate_final_workspace(context: &RunContext, approval: &ApprovalArtifact) -> Result<()> {
    let current = workspace_snapshot(&context.root, Some(&context.dir))?;
    let changes = snapshot_changes(&approval.workspace_snapshot, &current);
    let approved = approval.write_set.iter().cloned().collect::<BTreeSet<_>>();
    let outside = changes
        .iter()
        .filter(|change| !approved.contains(&change.path))
        .map(|change| change.path.clone())
        .collect::<Vec<_>>();
    ensure!(
        outside.is_empty(),
        "workspace changed outside the approved write set after apply: {}",
        outside.join(", ")
    );
    Ok(())
}

fn required_check_failures(
    plan: &PlanArtifact,
    verification: &VerificationArtifact,
) -> Vec<String> {
    let required = plan.checks.iter().filter(|check| check.required);
    let mut reasons = if plan
        .checks
        .iter()
        .any(|check| check.required && check.kind == WorkflowCheckKind::Test)
    {
        Vec::new()
    } else {
        vec!["plan does not declare a required test check".to_string()]
    };
    for expected in required {
        let record = verification.checks.iter().rev().find(|record| {
            record.kind == expected.kind
                && record.program == expected.program
                && record.args == expected.args
        });
        match record {
            Some(record) if record.success => {}
            Some(_) => reasons.push(format!("required {:?} check failed", expected.kind)),
            None => reasons.push(format!("required {:?} check was not run", expected.kind)),
        }
    }
    reasons
}

fn finish_result(
    verification: &VerificationArtifact,
    rescan: &RescanArtifact,
    reasons: &mut Vec<String>,
) -> WorkflowPhase {
    if verification.checks.iter().any(|check| !check.success) {
        return WorkflowPhase::Failed;
    }
    if !rescan.unobservable.is_empty() {
        reasons.push("selected evidence became unobservable".to_string());
    }
    if !rescan.coverage_limitations.is_empty() {
        reasons.push("rescan reports degraded observation coverage".to_string());
    }
    if reasons.is_empty() {
        WorkflowPhase::Verified
    } else {
        WorkflowPhase::NeedsInput
    }
}
