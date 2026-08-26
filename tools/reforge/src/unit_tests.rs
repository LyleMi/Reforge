use super::*;

fn gate_report(kind: reforge_schema::IssueKind) -> Report {
    let issue = reforge_schema::Issue::new(reforge_schema::IssueInput {
        kind,
        analysis: "codebase".into(),
        family: "reforge.codebase.file_size".into(),
        subject: reforge_schema::Subject::File {
            entity: reforge_schema::EntityRef::new("file:src/lib.rs", "src/lib.rs", None),
        },
        title: "Large file".into(),
        guidance: "Split cohesive responsibilities.".into(),
        evidence: vec![reforge_schema::Evidence::new(
            "reforge.codebase.large_file",
            "file:src/lib.rs",
            "large file",
        )],
    });
    let coverage = BTreeMap::from([(
        "codebase".into(),
        reforge_schema::AnalysisCoverage {
            status: reforge_schema::CoverageStatus::Observed,
            scanned_files: 1,
            languages: BTreeMap::new(),
            rules: BTreeMap::new(),
            limitations: Vec::new(),
        },
    )]);
    let issues = vec![issue];
    Report::new(reforge_schema::ReportInput {
        producer: reforge_schema::Producer {
            name: "reforge.analyze".into(),
            version: "test".into(),
            revision: None,
        },
        target: reforge_schema::Target {
            root: "/work".into(),
            workspace_identity: "rw5-test".into(),
            source_revision: None,
        },
        provenance: reforge_schema::default_provenance(&coverage, &issues),
        suppression: reforge_schema::SuppressionSummary::default(),
        coverage,
        issues,
    })
}

#[test]
fn gates_count_only_policy_and_fail_closed_for_unknown() {
    let advisory = gate_report(reforge_schema::IssueKind::Advisory);
    assert_eq!(gate_failures(&advisory, Some(GateArg::All)).unwrap(), 0);

    let mut policy = gate_report(reforge_schema::IssueKind::Policy);
    assert_eq!(gate_failures(&policy, Some(GateArg::All)).unwrap(), 1);
    let id = policy.issues[0].id.clone();
    for (state, expected) in [
        (reforge_schema::BaselineState::New, 1),
        (reforge_schema::BaselineState::Updated, 1),
        (reforge_schema::BaselineState::Unknown, 1),
        (reforge_schema::BaselineState::Unchanged, 0),
        (reforge_schema::BaselineState::Absent, 0),
    ] {
        policy.baseline_comparison = Some(reforge_schema::BaselineComparison {
            issues: BTreeMap::from([(
                id.clone(),
                reforge_schema::BaselineEntry {
                    state,
                    reason: None,
                },
            )]),
        });
        assert_eq!(
            gate_failures(&policy, Some(GateArg::New)).unwrap(),
            expected,
            "{state:?}"
        );
    }

    policy.issues.clear();
    policy.baseline_comparison = Some(reforge_schema::BaselineComparison {
        issues: BTreeMap::from([(
            id,
            reforge_schema::BaselineEntry {
                state: reforge_schema::BaselineState::Unknown,
                reason: Some("policy_issue:coverage_changed".into()),
            },
        )]),
    });
    assert_eq!(gate_failures(&policy, Some(GateArg::New)).unwrap(), 1);
}

#[test]
fn repository_guide_uses_current_cli_vocabulary() {
    let guide = include_str!("../../../AGENTS.md");
    assert!(guide.contains("--analysis codebase"));
    assert!(guide.contains("`rules`"));
    assert!(!guide.contains("--analysis structure"));
    assert!(!guide.contains("`catalog`"));
}

#[test]
fn distributed_agent_contracts_use_current_schema_versions() {
    let bundle: serde_json::Value =
        serde_json::from_str(include_str!("../../../.codex-plugin/bundle.json")).unwrap();
    assert_eq!(bundle["report_schema"], 27);

    for contract in [
        include_str!("../../../skills/SKILL.template.md"),
        include_str!("../../../skills/reforge-analyze/SKILL.md"),
        include_str!("../../reforge-workflow/skills/reforge-plan/SKILL.md"),
        include_str!("../../reforge-workflow/skills/reforge-apply/SKILL.md"),
        include_str!("../../reforge-workflow/skills/reforge-verify/SKILL.md"),
    ] {
        assert!(contract.contains("report schema `27`"));
        assert!(contract.contains("artifact schema `6`"));
    }
}

#[test]
fn nested_override_updates_effective_value() {
    let mut value: toml::Value = toml::from_str(default_config()).unwrap();
    apply_override(&mut value, "dataflow.search.max-path-steps=24").unwrap();
    assert_eq!(
        value_at(&value, "dataflow.search.max-path-steps").and_then(toml::Value::as_integer),
        Some(24)
    );
}

#[test]
fn config_show_materializes_low_cohesion_preset_sources() {
    let mut value: toml::Value = toml::from_str(default_config()).unwrap();
    apply_override(&mut value, "codebase.preset='strict'").unwrap();
    let mut sources = effective_sources(&value, None);
    sources.insert("codebase.preset".into(), "cli --set".into());
    materialize_low_module_cohesion_thresholds(&mut value, &mut sources).unwrap();
    assert_eq!(
        value_at(&value, "codebase.min-module-functions").and_then(toml::Value::as_integer),
        Some(16)
    );
    assert_eq!(
        sources
            .get("codebase.min-module-functions")
            .map(String::as_str),
        Some("strict preset (cli --set)")
    );
}

#[test]
fn baseline_identity_includes_selected_analyses() {
    let status = reforge_schema::CoverageStatus::Observed;
    assert!(status.is_observable());
}

#[test]
fn config_uses_analysis_enabled_and_rejects_removed_lenses() {
    let defaults: toml::Value = toml::from_str(default_config()).unwrap();
    assert_eq!(
        value_at(&defaults, "analysis.enabled")
            .and_then(toml::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    let removed: toml::Value =
        toml::from_str("version = 2\n[analysis]\nlenses = ['codebase']\n").unwrap();
    let error = validate_config(&removed).unwrap_err().to_string();
    assert_eq!(
        error,
        "`analysis.lenses` was removed; use `analysis.enabled`"
    );
}

#[test]
fn config_rejects_nested_unknown_keys() {
    let mut value: toml::Value = toml::from_str(default_config()).unwrap();
    apply_override(&mut value, "dataflow.search.max-path-stepz=2").unwrap();
    let error = validate_config(&value).unwrap_err().to_string();
    assert!(error.contains("dataflow.search.max-path-stepz"));
}

#[test]
fn config_rejects_invalid_nested_types_and_ranges() {
    for (override_value, expected) in [
        ("scope.include-hidden='yes'", "scope.include-hidden"),
        ("scope.ignore-paths=[1]", "scope.ignore-paths[0]"),
        ("codebase.preset='fast'", "codebase.preset"),
        (
            "codebase.function-similarity=1.2",
            "codebase.function-similarity",
        ),
        ("dataflow.fan-out.min-sinks=0", "dataflow.fan-out.min-sinks"),
    ] {
        let mut value: toml::Value = toml::from_str(default_config()).unwrap();
        apply_override(&mut value, override_value).unwrap();
        let error = validate_config(&value).unwrap_err().to_string();
        assert!(error.contains(expected), "{override_value}: {error}");
    }
}

#[test]
fn config_rejects_unknown_suppression_rules_with_location() {
    let value: toml::Value = toml::from_str(
        r#"version = 2
[[suppressions]]
rule = "reforge.codebase.not_a_rule"
path = "src/**"
reason = "test"
"#,
    )
    .unwrap();
    let error = validate_config(&value).unwrap_err().to_string();
    assert!(error.contains("suppressions[0].rule"));
    assert!(error.contains("reforge.codebase.not_a_rule"));
}

#[test]
fn discovered_config_overlays_built_in_defaults() {
    let mut defaults: toml::Value = toml::from_str(default_config()).unwrap();
    let configured: toml::Value =
        toml::from_str("version = 2\n[dataflow.search]\nmax-path-steps = 22\n").unwrap();
    merge_config(&mut defaults, configured);
    assert_eq!(
        value_at(&defaults, "dataflow.search.max-path-steps").and_then(toml::Value::as_integer),
        Some(22)
    );
    assert_eq!(
        value_at(&defaults, "codebase.max-file-lines").and_then(toml::Value::as_integer),
        Some(600)
    );
}

#[test]
fn removed_mode_and_packs_are_rejected() {
    for input in [
        "version = 2\n[dataflow]\nmode = 'observe'\n",
        "version = 2\n[packs.unity]\nmode = 'on'\n",
    ] {
        let value: toml::Value = toml::from_str(input).unwrap();
        assert!(validate_config(&value).is_err());
    }
}
