#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("reforge-workflow-{name}-{}", epoch_ms()))
    }

    #[test]
    fn canonical_json_fingerprint_ignores_object_key_order() {
        let left = serde_json::json!({"b": 2, "a": {"d": 4, "c": 3}});
        let right = serde_json::json!({"a": {"c": 3, "d": 4}, "b": 2});
        assert_eq!(fingerprint_json(&left), fingerprint_json(&right));
    }

    #[test]
    fn rejects_root_escaping_paths_and_symlinks() -> Result<()> {
        let root = temp_root("paths");
        fs::create_dir_all(root.join("src"))?;
        assert!(validate_relative_path(&root, "src/lib.rs").is_ok());
        assert!(validate_relative_path(&root, "../outside.rs").is_err());
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn atomic_write_refuses_duplicate_artifacts() -> Result<()> {
        let root = temp_root("atomic");
        fs::create_dir_all(&root)?;
        let path = root.join("artifact.json");
        atomic_write_json(&path, &serde_json::json!({"value": 1}), false)?;
        assert!(atomic_write_json(&path, &serde_json::json!({"value": 2}), false).is_err());
        assert_eq!(read_json::<Value>(&path)?["value"], 1);
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn output_summary_redacts_secret_lines_and_truncates() {
        let output = format!("TOKEN=abc\n{}", "x".repeat(OUTPUT_SUMMARY_LIMIT * 2));
        let summary = summarize_output(&output);
        assert!(summary.starts_with("[redacted]"));
        assert!(summary.ends_with("[truncated]"));
        assert!(!summary.contains("abc"));
    }

    #[test]
    fn snapshot_changes_preserve_preexisting_files_and_detect_add_delete() {
        let before = BTreeMap::from([
            ("a.rs".to_string(), "one".to_string()),
            ("b.rs".to_string(), "two".to_string()),
        ]);
        let after = BTreeMap::from([
            ("a.rs".to_string(), "changed".to_string()),
            ("c.rs".to_string(), "three".to_string()),
        ]);
        let changes = snapshot_changes(&before, &after);
        assert_eq!(changes.len(), 3);
        assert!(
            changes
                .iter()
                .any(|change| change.path == "b.rs" && change.after_sha256.is_none())
        );
        assert!(
            changes
                .iter()
                .any(|change| change.path == "c.rs" && change.before_sha256.is_none())
        );
    }

    #[test]
    fn missing_program_is_recorded_without_using_a_shell() {
        let root = std::env::temp_dir();
        let record = run_check(CheckExecution {
            kind: WorkflowCheckKind::Custom,
            program: "reforge-command-that-does-not-exist",
            args: &[],
            declared: false,
            root: &root,
            timeout: Duration::from_secs(1),
        });
        assert!(!record.command_found);
        assert!(!record.success);
        assert_eq!(record.exit_code, None);
    }

    #[test]
    fn times_out_a_direct_program_and_records_the_result() {
        let root = std::env::temp_dir();
        let args = ["2".to_string()];
        let record = run_check(CheckExecution {
            kind: WorkflowCheckKind::Custom,
            program: "sleep",
            args: &args,
            declared: false,
            root: &root,
            timeout: Duration::from_millis(20),
        });
        assert!(record.command_found);
        assert!(record.timed_out);
        assert!(!record.success);
    }

    #[test]
    fn removed_schema_20_cli_options_are_rejected() {
        for (option, value) in [
            ("--min-priority", "35"),
            ("--severity", "warning"),
            ("--hotspot-model", "static"),
            ("--scoring-policy", "policy.json"),
            ("--fail-on", "warning"),
        ] {
            assert!(
                Cli::try_parse_from_with_explicit_overrides([
                    "reforge", "scan", ".", option, value
                ])
                .is_err(),
                "{option} should remain rejected"
            );
        }
    }

    #[test]
    fn schema_21_serialization_has_no_schema_20_ranking_fields() -> Result<()> {
        let root = temp_root("schema");
        fs::create_dir_all(root.join("src"))?;
        fs::write(root.join("src/lib.rs"), "one\ntwo\n")?;
        let mut args = ScanArgs::defaults_for_path(root.clone());
        args.max_file_lines = 1;
        args.threshold_overrides.max_file_lines = true;
        args.churn = Some(crate::cli::ChurnMode::Off);
        let mut progress = NoopProgress;
        let report = scan::scan_report(&args, &mut progress)?;
        let value = serde_json::to_value(&report)?;
        assert!(!report.findings.is_empty());
        for field in ["hotspots", "scoring_policy"] {
            assert!(value.get(field).is_none(), "unexpected top-level {field}");
        }
        for field in ["hotspot_count", "hotspot_model"] {
            assert!(
                value["summary"].get(field).is_none(),
                "unexpected summary {field}"
            );
        }
        let finding = &value["findings"][0];
        for field in [
            "priority",
            "severity",
            "detection_reliability",
            "interpretation_reliability",
            "priority_factors",
            "rank_explanation",
        ] {
            assert!(finding.get(field).is_none(), "unexpected finding {field}");
        }
        let mut zero_report = report.clone();
        zero_report.findings.clear();
        zero_report.issues.clear();
        zero_report.summary.finding_count = 0;
        zero_report.summary.issue_count = 0;
        let zero_value = serde_json::to_value(&zero_report)?;
        assert_eq!(zero_value["findings"], serde_json::json!([]));
        assert_eq!(zero_value["issues"], serde_json::json!([]));
        assert_eq!(zero_value["summary"]["finding_count"], 0);
        assert_eq!(zero_value["summary"]["issue_count"], 0);
        let mut stale = value;
        stale
            .as_object_mut()
            .expect("report is an object")
            .insert("hotspots".to_string(), Value::Array(Vec::new()));
        assert!(serde_json::from_value::<ScanReport>(stale).is_err());
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn conflict_graph_covers_shared_files_dependencies_tests_and_unity() -> Result<()> {
        let root = temp_root("conflicts");
        fs::create_dir_all(root.join("src"))?;
        fs::write(root.join("src/a.rs"), "one\ntwo\n")?;
        fs::write(root.join("src/b.rs"), "one\ntwo\n")?;
        let mut args = ScanArgs::defaults_for_path(root.clone());
        args.max_file_lines = 1;
        args.threshold_overrides.max_file_lines = true;
        args.churn = Some(crate::cli::ChurnMode::Off);
        let mut progress = NoopProgress;
        let mut report = scan::scan_report(&args, &mut progress)?;
        let issues = report
            .issues
            .iter()
            .filter(|issue| issue.kinds.contains(&FindingKind::LargeFile))
            .take(2)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(issues.len(), 2);
        report.dependency_graph.edges.extend([
            crate::model::DependencyGraphEdge {
                from: issues[0].path.clone(),
                to: "src/shared.rs".to_string(),
            },
            crate::model::DependencyGraphEdge {
                from: issues[1].path.clone(),
                to: "src/shared.rs".to_string(),
            },
        ]);
        for evidence in &mut report.agent_evidence.issues {
            if evidence.issue_id == issues[0].id || evidence.issue_id == issues[1].id {
                evidence
                    .test_reachability
                    .reachable_test_files
                    .push("tests/shared.rs".to_string());
            }
        }
        let investigations = issues
            .iter()
            .map(|issue| InvestigationArtifact {
                artifact_schema_version: ARTIFACT_SCHEMA_VERSION,
                issue_id: issue.id.clone(),
                finding_ids: issue.finding_ids.clone(),
                report_fingerprint: "sha256-test".to_string(),
                status: InvestigationStatus::Complete,
                facts: Vec::new(),
                analysis: Vec::new(),
                unknowns: Vec::new(),
                rejected_alternatives: Vec::new(),
                inspected_files: vec!["src/shared.rs".to_string()],
                read_set: Vec::new(),
                write_set: vec![
                    "src/shared.rs".to_string(),
                    format!("Assets/Shared/{}.meta", issue.id),
                ],
                coverage_limitations: Vec::new(),
                checks: Vec::new(),
            })
            .collect::<Vec<_>>();
        let graph = conflict_graph(&report, &investigations);
        assert_eq!(graph.len(), 1);
        assert!(graph[0].reasons.contains(&"shared_write_file".to_string()));
        assert!(
            graph[0]
                .reasons
                .contains(&"shared_evidence_file".to_string())
        );
        assert!(
            graph[0]
                .reasons
                .contains(&"shared_dependency_boundary".to_string())
        );
        assert!(graph[0].reasons.contains(&"shared_test".to_string()));
        assert!(
            graph[0]
                .reasons
                .contains(&"shared_unity_surface".to_string())
        );
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn completes_an_approved_apply_and_verify_workflow() -> Result<()> {
        let root = temp_root("lifecycle");
        let run_dir = root.with_extension("run");
        fs::create_dir_all(root.join("src"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub fn value() -> usize {\n    1\n}\n",
        )?;

        let mut scan_args = ScanArgs::defaults_for_path(root.clone());
        scan_args.max_file_lines = 1;
        scan_args.threshold_overrides.max_file_lines = true;
        scan_args.churn = Some(crate::cli::ChurnMode::Off);
        scan_args.progress = crate::cli::ProgressMode::Never;
        start(WorkflowStartArgs {
            scan: scan_args,
            run_dir: Some(run_dir.clone()),
        })?;
        let report: ScanReport = read_json(&run_dir.join("scan.json"))?;
        let issue = report
            .issues
            .iter()
            .find(|issue| issue.kinds.contains(&FindingKind::LargeFile))
            .context("expected a large-file issue")?;
        select(WorkflowSelectArgs {
            run: run_dir.clone(),
            issues: vec![issue.id.to_string()],
            goal: "make the fixture small".to_string(),
        })?;
        let run: RunArtifact = read_json(&run_dir.join("run.json"))?;
        let investigation = InvestigationArtifact {
            artifact_schema_version: ARTIFACT_SCHEMA_VERSION,
            issue_id: issue.id.clone(),
            finding_ids: issue.finding_ids.clone(),
            report_fingerprint: run.report_fingerprint.clone(),
            status: InvestigationStatus::Complete,
            facts: vec![InvestigationFact {
                path: "src/lib.rs".to_string(),
                line: Some(1),
                statement: "the fixture exceeds one line".to_string(),
            }],
            analysis: vec!["collapse the fixture".to_string()],
            unknowns: Vec::new(),
            rejected_alternatives: Vec::new(),
            inspected_files: vec!["src/lib.rs".to_string()],
            read_set: vec!["src/lib.rs".to_string()],
            write_set: vec!["src/lib.rs".to_string()],
            coverage_limitations: Vec::new(),
            checks: vec![CommandSpec {
                kind: WorkflowCheckKind::Test,
                program: "rustc".to_string(),
                args: vec!["--version".to_string()],
                expected_observation: "rustc is available".to_string(),
            }],
        };
        atomic_write_json(
            &run_dir
                .join("investigations")
                .join(format!("{}.json", issue.id)),
            &investigation,
            false,
        )?;
        advance(WorkflowRunArgs {
            run: run_dir.clone(),
        })?;
        let plan = PlanArtifact {
            artifact_schema_version: ARTIFACT_SCHEMA_VERSION,
            report_fingerprint: run.report_fingerprint,
            goal: "make the fixture small".to_string(),
            outcome: "the file no longer exceeds the configured line threshold".to_string(),
            selected_issue_ids: vec![issue.id.clone()],
            batches: vec![PlanBatch {
                issue_ids: vec![issue.id.clone()],
                write_set: vec!["src/lib.rs".to_string()],
                outcome: "collapse the fixture".to_string(),
            }],
            write_set: vec!["src/lib.rs".to_string()],
            behavior_assumptions: vec!["the return value remains one".to_string()],
            checks: vec![PlannedCheck {
                kind: WorkflowCheckKind::Test,
                program: "rustc".to_string(),
                args: vec!["--version".to_string()],
                required: true,
                expected_observation: "rustc exits successfully".to_string(),
            }],
            unresolved_risks: Vec::new(),
            conflicts: Vec::new(),
        };
        atomic_write_json(&run_dir.join("plan.json"), &plan, false)?;
        advance(WorkflowRunArgs {
            run: run_dir.clone(),
        })?;
        approve(WorkflowRunArgs {
            run: run_dir.clone(),
        })?;
        fs::write(root.join("src/lib.rs"), "pub fn value() -> usize { 1 }\n")?;
        mark_applied(WorkflowRunArgs {
            run: run_dir.clone(),
        })?;
        check(WorkflowCheckArgs {
            run: run_dir.clone(),
            kind: WorkflowCheckKind::Test,
            timeout_seconds: 30,
            command: vec!["rustc".to_string(), "--version".to_string()],
        })?;
        rescan(WorkflowRunArgs {
            run: run_dir.clone(),
        })?;
        finish(WorkflowRunArgs {
            run: run_dir.clone(),
        })?;
        let completed: RunArtifact = read_json(&run_dir.join("run.json"))?;
        assert_eq!(completed.phase, WorkflowPhase::Verified);

        fs::remove_dir_all(root)?;
        fs::remove_dir_all(run_dir)?;
        Ok(())
    }
}
