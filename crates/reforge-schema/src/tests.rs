use super::*;

fn coverage() -> BTreeMap<String, AnalysisCoverage> {
    BTreeMap::from([(
        "codebase".into(),
        AnalysisCoverage {
            status: CoverageStatus::Observed,
            scanned_files: 1,
            languages: BTreeMap::new(),
            rules: BTreeMap::new(),
            limitations: Vec::new(),
        },
    )])
}

fn report(evidence: Vec<Evidence>) -> Report {
    let issue = Issue::new(
        "codebase",
        "reforge.codebase.large_file",
        Subject::File {
            path: "./src\\lib.rs".into(),
        },
        ("Large file: src/lib.rs", "Split cohesive responsibilities."),
        evidence,
    );
    Report::new(
        Producer {
            name: "reforge.analyze".into(),
            version: "1".into(),
            revision: None,
        },
        Target {
            root: "/tmp/checkout".into(),
            workspace_identity: "rw5-test".into(),
            source_revision: None,
        },
        SuppressionSummary::default(),
        coverage(),
        vec![issue],
    )
}

#[test]
fn compact_round_trip_and_old_field_rejection() {
    let evidence = Evidence::new("reforge.codebase.large_file", "src/lib.rs", "large file");
    let report = report(vec![evidence]);
    let bytes = serde_json::to_vec(&report).unwrap();
    let parsed: Report = serde_json::from_slice(&bytes).unwrap();
    parsed.validate().unwrap();

    for field in ["profile", "extensions", "findings"] {
        let mut value = serde_json::to_value(&report).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert(field.into(), serde_json::json!({}));
        assert!(serde_json::from_value::<Report>(value).is_err());
    }
}

#[test]
fn coverage_round_trip_rejects_the_transitional_rule_shape() {
    let mut value = serde_json::to_value(report(vec![Evidence::new(
        "reforge.codebase.large_file",
        "src/lib.rs",
        "large file",
    )]))
    .unwrap();
    value["coverage"]["codebase"]["languages"] = serde_json::json!({
        "rust": {
            "status": "partial",
            "files": 2,
            "functions": 4,
            "limitations": [{
                "code": "parse_failure",
                "count": 1,
                "message": "one source file could not be parsed"
            }]
        }
    });
    value["coverage"]["codebase"]["rules"] = serde_json::json!({
        "reforge.codebase.large_file": {
            "status": "observed",
            "observations": [{
                "name": "files_scanned",
                "count": 2,
                "unit": "file"
            }]
        }
    });
    let parsed: Report = serde_json::from_value(value.clone()).unwrap();
    parsed.validate().unwrap();

    value["coverage"]["codebase"]["rules"]["reforge.codebase.large_file"] =
        serde_json::json!({"status": "observed", "observed_entities": 2});
    assert!(serde_json::from_value::<Report>(value).is_err());
}

#[test]
fn issue_analysis_must_name_coverage() {
    let mut value = report(vec![Evidence::new(
        "reforge.codebase.large_file",
        "src/lib.rs",
        "large file",
    )]);
    value.issues[0].analysis = "dataflow".into();
    assert!(
        value
            .validate()
            .unwrap_err()
            .to_string()
            .contains("absent from coverage")
    );
}

#[test]
fn dataflow_evidence_identity_includes_policy_source_and_sink() {
    let rule = "reforge.dataflow.adapter_flow_bypass";
    let first = Evidence::new(rule, "flow:http:source-a:sink-a", "bypass");
    let different_source = Evidence::new(rule, "flow:http:source-b:sink-a", "bypass");
    let different_sink = Evidence::new(rule, "flow:http:source-a:sink-b", "bypass");
    assert_ne!(first.id, different_source.id);
    assert_ne!(first.id, different_sink.id);
}

#[test]
fn schema_26_ids_and_derived_summary_are_enforced() {
    let evidence = Evidence::new("reforge.codebase.large_file", "src/lib.rs", "large file");
    let valid = report(vec![evidence]);
    assert!(valid.issues[0].id.starts_with("ri6-"));
    assert!(valid.issues[0].evidence[0].id.starts_with("re6-"));

    let mut old_issue = valid.clone();
    old_issue.issues[0].id = old_issue.issues[0].id.replacen("ri6-", "ri5-", 1);
    assert!(
        old_issue
            .validate()
            .unwrap_err()
            .to_string()
            .contains("stable ID")
    );

    let mut old_evidence = valid.clone();
    old_evidence.issues[0].evidence[0].id = old_evidence.issues[0].evidence[0]
        .id
        .replacen("re6-", "re5-", 1);
    assert!(
        old_evidence
            .validate()
            .unwrap_err()
            .to_string()
            .contains("schema 26 evidence ID")
    );

    let mut inconsistent = valid;
    inconsistent.summary.scanned_files += 1;
    assert!(
        inconsistent
            .validate()
            .unwrap_err()
            .to_string()
            .contains("coverage and issue contents")
    );
}

#[test]
fn issue_id_ignores_evidence_order_and_growth() {
    let first = Evidence::new("reforge.codebase.large_file", "a", "one");
    let second = Evidence::new("reforge.codebase.large_file", "b", "two");
    let a = report(vec![first.clone(), second.clone()]);
    let b = report(vec![second, first]);
    let c = report(vec![
        Evidence::new("reforge.codebase.large_file", "a", "changed"),
        Evidence::new("reforge.codebase.large_file", "b", "two"),
        Evidence::new("reforge.codebase.large_file", "c", "three"),
    ]);
    assert_eq!(a.issues[0].id, b.issues[0].id);
    assert_eq!(a.issues[0].id, c.issues[0].id);
}

#[test]
fn group_subject_is_path_and_symbol_stable() {
    let a = Subject::Group {
        members: vec!["b.rs#b".into(), "a.rs#a".into()],
    };
    let b = Subject::Group {
        members: vec!["./a.rs#a".into(), "b.rs#b".into()],
    };
    assert_eq!(
        issue_id("reforge.codebase.similar", &a),
        issue_id("reforge.codebase.similar", &b)
    );
}

#[test]
fn evidence_id_ignores_prose_and_measurements() {
    assert_eq!(
        evidence_id("reforge.codebase.similar", "same-anchor"),
        evidence_id("reforge.codebase.similar", "same-anchor")
    );
}

#[test]
fn measurements_are_json_numbers() {
    let mut evidence = Evidence::new("reforge.codebase.large_file", "a", "large");
    evidence.measurements.push(Measurement {
        name: "file.loc".into(),
        value: 700.0,
        threshold: Some(600.0),
        unit: "lines".into(),
    });
    let value = serde_json::to_value(report(vec![evidence])).unwrap();
    let measurement = &value["issues"][0]["evidence"][0]["measurements"][0];
    assert!(measurement["value"].is_number());
    assert!(measurement["threshold"].is_number());

    let mut invalid = value;
    invalid["issues"][0]["evidence"][0]["measurements"][0]["value"] = "700".into();
    assert!(serde_json::from_value::<Report>(invalid).is_err());
}

#[test]
fn baseline_requires_matching_producer_workspace_and_analysis_set() {
    let current = report(vec![Evidence::new(
        "reforge.codebase.large_file",
        "src/lib.rs",
        "large file",
    )]);

    let mut different = current.clone();
    different.producer.version = "other".into();
    assert_eq!(
        current
            .validate_baseline(&different)
            .unwrap_err()
            .to_string(),
        "baseline producer does not match the current report"
    );

    let mut different = current.clone();
    different.target.workspace_identity = "rw5-other".into();
    assert_eq!(
        current
            .validate_baseline(&different)
            .unwrap_err()
            .to_string(),
        "baseline workspace does not match the current report"
    );

    let mut different = current.clone();
    different.coverage.insert(
        "dataflow".into(),
        AnalysisCoverage {
            status: CoverageStatus::Observed,
            scanned_files: 1,
            languages: BTreeMap::new(),
            rules: BTreeMap::new(),
            limitations: Vec::new(),
        },
    );
    assert_eq!(
        current
            .validate_baseline(&different)
            .unwrap_err()
            .to_string(),
        "baseline analysis set does not match the current report"
    );
}

#[test]
fn coverage_downgrade_does_not_resolve_disappearing_issues() {
    let baseline = report(vec![Evidence::new(
        "reforge.codebase.large_file",
        "src/lib.rs",
        "large file",
    )]);
    let mut current = baseline.clone();
    current.issues.clear();
    current.summary.issue_count = 0;
    current.summary.evidence_count = 0;
    current.coverage.get_mut("codebase").unwrap().status = CoverageStatus::Partial;

    assert_eq!(current.coverage_downgrades(&baseline), ["codebase"]);
    assert!(current.compare_to(&baseline).resolved_issue_ids.is_empty());
}
