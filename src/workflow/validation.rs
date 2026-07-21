fn validate_run(context: &RunContext) -> Result<()> {
    validate_schema_version(context.run.artifact_schema_version, "run.json")?;
    ensure!(
        context.run.report_schema_version == SCAN_REPORT_SCHEMA_VERSION,
        "run uses unsupported report schema {}",
        context.run.report_schema_version
    );
    let report: ScanReport = read_json(&context.dir.join("scan.json"))?;
    validate_report_fingerprint(&context.run, &report)?;
    if matches!(
        context.run.phase,
        WorkflowPhase::Scanned
            | WorkflowPhase::Selected
            | WorkflowPhase::Investigated
            | WorkflowPhase::Planned
    ) {
        let current = workspace_snapshot(&context.root, Some(&context.dir))?;
        ensure!(
            snapshot_fingerprint(&current)? == context.run.source_fingerprint,
            "target source changed since workflow start; start a new run"
        );
    }
    match context.run.phase {
        WorkflowPhase::Scanned => {}
        WorkflowPhase::Selected => {
            validate_selection(context, &report)?;
        }
        WorkflowPhase::Investigated => {
            validate_selection(context, &report)?;
            validate_investigations(context)?;
        }
        WorkflowPhase::Planned => {
            validate_selection(context, &report)?;
            validate_investigations(context)?;
            validate_plan(context)?;
        }
        WorkflowPhase::Approved => {
            validate_plan(context)?;
            let approval: ApprovalArtifact = read_json(&context.dir.join("approval.json"))?;
            validate_approval(context, &approval)?;
        }
        WorkflowPhase::Applied | WorkflowPhase::Verified => {
            validate_plan(context)?;
            let approval: ApprovalArtifact = read_json(&context.dir.join("approval.json"))?;
            validate_approval(context, &approval)?;
            let _: ApplicationArtifact = read_json(&context.dir.join("application.json"))?;
        }
        WorkflowPhase::Failed | WorkflowPhase::NeedsInput => {
            if context.dir.join("application.json").is_file() {
                validate_plan(context)?;
                let approval: ApprovalArtifact = read_json(&context.dir.join("approval.json"))?;
                validate_approval(context, &approval)?;
                let _: ApplicationArtifact = read_json(&context.dir.join("application.json"))?;
            } else {
                validate_selection(context, &report)?;
                validate_investigations(context)?;
            }
        }
    }
    Ok(())
}

fn validate_selection(context: &RunContext, report: &ScanReport) -> Result<SelectionArtifact> {
    let selection: SelectionArtifact = read_json(&context.dir.join("selection.json"))?;
    validate_schema_version(selection.artifact_schema_version, "selection.json")?;
    ensure!(
        selection.report_fingerprint == context.run.report_fingerprint,
        "selection report fingerprint is stale"
    );
    ensure_unique(&selection.issue_ids, "selected issue ID")?;
    let ids = report
        .issues
        .iter()
        .map(|issue| issue.id.clone())
        .collect::<BTreeSet<_>>();
    for issue_id in &selection.issue_ids {
        ensure!(
            ids.contains(issue_id),
            "unknown selected issue ID {issue_id}"
        );
    }
    Ok(selection)
}

fn validate_investigations(context: &RunContext) -> Result<Vec<InvestigationArtifact>> {
    let report: ScanReport = read_json(&context.dir.join("scan.json"))?;
    let selection = validate_selection(context, &report)?;
    let mut artifacts = Vec::new();
    for issue_id in &selection.issue_ids {
        let path = context
            .dir
            .join("investigations")
            .join(format!("{}.json", issue_id.as_str()));
        let artifact: InvestigationArtifact = read_json(&path)
            .with_context(|| format!("missing or invalid investigation for {issue_id}"))?;
        validate_schema_version(artifact.artifact_schema_version, "investigation")?;
        ensure!(
            artifact.issue_id == *issue_id,
            "investigation filename/ID mismatch"
        );
        ensure!(
            artifact.report_fingerprint == context.run.report_fingerprint,
            "investigation for {issue_id} has a stale report fingerprint"
        );
        let issue = report
            .issues
            .iter()
            .find(|issue| issue.id == *issue_id)
            .expect("selection validation checked issue IDs");
        let expected = issue.finding_ids.iter().cloned().collect::<BTreeSet<_>>();
        let actual = artifact
            .finding_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        ensure!(
            actual == expected,
            "investigation finding IDs do not match {issue_id}"
        );
        ensure_unique(&artifact.finding_ids, "investigation finding ID")?;
        validate_paths(
            &context.root,
            artifact
                .inspected_files
                .iter()
                .chain(&artifact.read_set)
                .chain(&artifact.write_set)
                .map(String::as_str),
        )?;
        ensure!(
            !artifact.facts.is_empty() || artifact.status != InvestigationStatus::Complete,
            "complete investigation {issue_id} must contain facts"
        );
        for fact in &artifact.facts {
            validate_relative_path(&context.root, &fact.path)?;
            ensure!(
                !fact.statement.trim().is_empty(),
                "investigation fact is empty"
            );
        }
        artifacts.push(artifact);
    }
    Ok(artifacts)
}

fn validate_plan(context: &RunContext) -> Result<PlanArtifact> {
    let report: ScanReport = read_json(&context.dir.join("scan.json"))?;
    let selection = validate_selection(context, &report)?;
    let investigations = validate_investigations(context)?;
    ensure!(
        investigations
            .iter()
            .all(|artifact| artifact.status == InvestigationStatus::Complete),
        "plan requires complete investigations"
    );
    let plan: PlanArtifact = read_json(&context.dir.join("plan.json"))?;
    validate_schema_version(plan.artifact_schema_version, "plan.json")?;
    ensure!(
        plan.report_fingerprint == context.run.report_fingerprint,
        "plan report fingerprint is stale"
    );
    ensure!(
        plan.goal == selection.goal,
        "plan goal does not match selection"
    );
    ensure!(
        plan.selected_issue_ids == selection.issue_ids,
        "plan issue IDs do not exactly match selection"
    );
    ensure_unique(&plan.write_set, "plan write path")?;
    validate_paths(&context.root, plan.write_set.iter().map(String::as_str))?;
    let investigation_writes = investigations
        .iter()
        .flat_map(|artifact| artifact.write_set.iter().cloned())
        .collect::<BTreeSet<_>>();
    for path in &plan.write_set {
        ensure!(
            investigation_writes.contains(path),
            "plan write path {path} was not proposed by an investigation"
        );
    }
    let conflicts = conflict_graph(&report, &investigations);
    let declared = plan
        .conflicts
        .iter()
        .map(|edge| (edge.left_issue_id.clone(), edge.right_issue_id.clone()))
        .collect::<BTreeSet<_>>();
    let actual = conflicts
        .iter()
        .map(|edge| (edge.left_issue_id.clone(), edge.right_issue_id.clone()))
        .collect::<BTreeSet<_>>();
    ensure!(
        declared == actual,
        "plan conflict graph does not match investigation evidence"
    );
    for batch in &plan.batches {
        ensure_unique(&batch.issue_ids, "batch issue ID")?;
        let batch_ids = batch.issue_ids.iter().cloned().collect::<BTreeSet<_>>();
        ensure!(
            !conflicts.iter().any(|edge| {
                batch_ids.contains(&edge.left_issue_id) && batch_ids.contains(&edge.right_issue_id)
            }),
            "plan batch contains conflicting issues"
        );
        validate_paths(&context.root, batch.write_set.iter().map(String::as_str))?;
    }
    Ok(plan)
}

fn validate_approval(context: &RunContext, approval: &ApprovalArtifact) -> Result<()> {
    validate_schema_version(approval.artifact_schema_version, "approval.json")?;
    ensure!(
        approval.report_fingerprint == context.run.report_fingerprint,
        "approval report fingerprint is stale"
    );
    let plan: PlanArtifact = read_json(&context.dir.join("plan.json"))?;
    ensure!(
        fingerprint_json(&serde_json::to_value(&plan)?) == approval.plan_fingerprint,
        "approved plan has changed"
    );
    ensure!(
        approval.write_set == plan.write_set,
        "approved write set differs from plan"
    );
    ensure!(
        snapshot_fingerprint(&approval.workspace_snapshot)?
            == approval.workspace_snapshot_fingerprint,
        "approval workspace snapshot fingerprint is corrupt"
    );
    Ok(())
}

fn load_context(run_dir: &Path) -> Result<RunContext> {
    let dir = run_dir
        .canonicalize()
        .with_context(|| format!("failed to resolve workflow run {}", run_dir.display()))?;
    ensure!(
        dir.is_dir(),
        "workflow run is not a directory: {}",
        dir.display()
    );
    let run: RunArtifact = read_json(&dir.join("run.json"))?;
    validate_schema_version(run.artifact_schema_version, "run.json")?;
    let root = PathBuf::from(&run.target_root)
        .canonicalize()
        .with_context(|| format!("failed to resolve target root {}", run.target_root))?;
    ensure!(root.is_dir(), "workflow target root is not a directory");
    Ok(RunContext { dir, run, root })
}

fn require_phase(run: &RunArtifact, expected: &[WorkflowPhase]) -> Result<()> {
    ensure!(
        expected.contains(&run.phase),
        "workflow phase {:?} is not valid for this command; expected {}",
        run.phase,
        expected
            .iter()
            .map(|phase| format!("{phase:?}"))
            .collect::<Vec<_>>()
            .join(" or ")
    );
    Ok(())
}

fn update_phase(context: &mut RunContext, phase: WorkflowPhase) -> Result<()> {
    context.run.phase = phase;
    context.run.updated_at_epoch_ms = epoch_ms();
    atomic_write_json(&context.dir.join("run.json"), &context.run, true)?;
    println!("{:?}", phase);
    Ok(())
}

fn validate_report_fingerprint(run: &RunArtifact, report: &ScanReport) -> Result<()> {
    ensure!(
        report.schema_version == run.report_schema_version,
        "scan report schema changed"
    );
    ensure!(
        fingerprint_json(&serde_json::to_value(report)?) == run.report_fingerprint,
        "scan.json fingerprint does not match run.json"
    );
    Ok(())
}

fn validate_optional_lineage(context: &RunContext, rescan: &RescanArtifact) -> Result<()> {
    let path = context.dir.join("lineage.json");
    if !path.exists() {
        return Ok(());
    }
    let lineage: LineageArtifact = read_json(&path)?;
    validate_schema_version(lineage.artifact_schema_version, "lineage.json")?;
    ensure!(lineage.original_report_fingerprint == rescan.original_report_fingerprint, "lineage.json original report fingerprint is stale");
    ensure!(lineage.rescan_report_fingerprint == rescan.rescan_report_fingerprint, "lineage.json rescan report fingerprint is stale");
    let candidates = rescan.lineage_candidates.iter().map(|candidate| (candidate.id.as_str(), candidate)).collect::<BTreeMap<_, _>>();
    let removed = rescan.selected_issues_removed.iter().map(|id| id.as_str()).collect::<BTreeSet<_>>();
    let unobservable = rescan.selected_issues_unobservable.iter().map(|id| id.as_str()).collect::<BTreeSet<_>>();
    let mut previous = BTreeSet::new();
    let mut successors = BTreeSet::new();
    for record in &lineage.records {
        ensure!(previous.insert(record.previous_issue_id.as_str()), "lineage.json contains duplicate issue dispositions");
        match record.kind {
            LineageRecordKind::Supersedes => {
                let id = record.candidate_id.as_deref().context("supersedes lineage record is missing candidate_id")?;
                let candidate = candidates.get(id).context("lineage.json confirms a candidate absent from rescan.json")?;
                ensure!(candidate.previous_id == record.previous_issue_id.as_str() && record.successor_issue_id.as_ref().is_some_and(|successor| successor.as_str() == candidate.current_id), "lineage.json candidate endpoints do not match rescan.json");
                ensure!(successors.insert(candidate.current_id.as_str()), "lineage.json reuses a successor issue");
            }
            LineageRecordKind::Remediated => {
                ensure!(record.successor_issue_id.is_none() && record.candidate_id.is_none(), "remediated lineage records cannot have a successor or candidate");
                ensure!(removed.contains(record.previous_issue_id.as_str()) && !unobservable.contains(record.previous_issue_id.as_str()), "remediated issue was not observably removed");
            }
        }
    }
    Ok(())
}

fn validate_config_fingerprint(context: &RunContext, args: &ScanArgs) -> Result<()> {
    let root = args.path.canonicalize()?;
    let effective = scan::effective_config_output(args, &root)?;
    let effective = serde_json::to_value(effective)?;
    let config_path = scan::validate_config(args.config.as_deref(), &root)?;
    let fingerprint = config_fingerprint(&effective, config_path.as_deref())?;
    ensure!(
        fingerprint == context.run.config_fingerprint,
        "scan configuration changed since workflow start"
    );
    Ok(())
}
