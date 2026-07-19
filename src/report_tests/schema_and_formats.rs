#[test]
fn renders_json_report_schema_v16_with_measurement_contract_metadata() {
    let scan_report = report(vec![make_finding(
        FindingKind::SimilarFunctions,
        "src/a.rs",
        Some(1),
        "similar",
        vec![FindingMetric::threshold(
            MetricId::GroupSize,
            3,
            3,
            "functions",
        )],
        vec![RelatedLocation {
            path: "src/a.rs".to_string(),
            line: 1,
            name: Some("alpha".to_string()),
        }],
    )]);

    let value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&scan_report).unwrap()).unwrap();

    assert_eq!(value["schema_version"], SCAN_REPORT_SCHEMA_VERSION);
    assert_eq!(value["summary"]["scanned_files"], 2);
    assert_eq!(value["summary"]["hotspot_model"], "hybrid");
    assert!(value.get("metrics_summary").is_some());
    assert!(value.get("raw_metrics").is_some());
    assert!(value.get("raw_metric_manifest").is_some());
    assert!(value.get("dependency_graph").is_some());
    assert!(value.get("hotspots").is_some());
    assert!(value.get("suppression_summary").is_some());
    assert!(value.get("issues").is_some());
    assert!(value.get("coverage_manifest").is_some());
    assert!(value.get("coverage_summary").is_some());
    assert!(value.get("detector_execution").is_some());
    assert!(value.get("raw_metric_coverage").is_some());
    assert_eq!(value["scoring_policy"]["source"], "builtin");
    assert!(value.get("detector_manifest").is_some());
    assert!(
        value["findings"][0]["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("rf3-"))
    );
    assert_eq!(value["findings"][0]["kind"], "similar_functions");
    assert_eq!(
        value["findings"][0]["recommendation"],
        "Extract the shared behavior into a common helper or deliberately separate the variants if they should evolve independently."
    );
    assert_eq!(value["findings"][0]["metrics"][0]["name"], "group.size");
    assert_eq!(value["findings"][0]["construct"], "reusability");
    assert_eq!(value["findings"][0]["mechanism"], "duplication_divergence");
    assert!(
        value["findings"][0]["metrics"][0]
            .get("dimension")
            .is_none()
    );
    assert!(value["findings"][0]["metrics"][0]["normalized"].is_number());
    assert_eq!(value["findings"][0]["priority"], 33);
    assert!(value["findings"][0]["priority_factors"]["impact"].is_number());
    assert!(value["findings"][0]["score"].is_null());
    assert!(value["findings"][0]["score_breakdown"].is_null());
    assert!(value["findings"][0]["rank_reason"].is_null());
    assert_eq!(
        value["findings"][0]["rank_explanation"],
        "duplication-divergence signal, medium action probability"
    );
    assert!(value["findings"][0].get("magnitude").is_none());
}

#[test]
fn caps_serialized_similar_function_locations() {
    let scan_report = report(vec![Finding::from(
        FindingInput::new(
            FindingKind::SimilarFunctions,
            "src/a.rs",
            Some(1),
            "similar",
            vec![FindingMetric::threshold(
                MetricId::GroupSize,
                75,
                3,
                "functions",
            )],
        )
        .with_detection_reliability(0.85)
        .with_related_locations(
            (0..75)
                .map(|index| RelatedLocation {
                    path: format!("src/{index}.rs"),
                    line: index + 1,
                    name: Some(format!("func_{index}")),
                })
                .collect(),
        ),
    )]);

    let value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&scan_report).unwrap()).unwrap();

    assert_eq!(
        value["findings"][0]["related_locations"]
            .as_array()
            .unwrap()
            .len(),
        50
    );
}

#[test]
fn writes_json_report_to_writer() {
    let mut output = Vec::new();

    write_json_report(&mut output, &report(Vec::new())).unwrap();

    let output = String::from_utf8(output).unwrap();
    assert!(output.ends_with('\n'));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&output).unwrap()["schema_version"],
        SCAN_REPORT_SCHEMA_VERSION
    );
}

#[test]
fn writes_yaml_report_to_writer() {
    let mut output = Vec::new();

    write_yaml_report(&mut output, &report(Vec::new())).unwrap();

    let output = String::from_utf8(output).unwrap();
    assert!(output.ends_with('\n'));
    assert_eq!(
        serde_yaml::from_str::<serde_yaml::Value>(&output).unwrap()["schema_version"],
        SCAN_REPORT_SCHEMA_VERSION
    );
}

#[test]
fn finding_ids_are_stable_for_equivalent_identity_inputs() {
    let left = make_finding(
        FindingKind::RepeatedLiteral,
        "src/a.rs",
        Some(10),
        "literal",
        vec![FindingMetric::threshold(
            MetricId::GroupSize,
            4,
            3,
            "occurrences",
        )],
        vec![RelatedLocation {
            path: "src/b.rs".to_string(),
            line: 20,
            name: Some("beta".to_string()),
        }],
    );
    let right = make_finding(
        FindingKind::RepeatedLiteral,
        "src/a.rs",
        Some(10),
        "changed wording",
        vec![FindingMetric::threshold(
            MetricId::GroupSize,
            10,
            3,
            "occurrences",
        )],
        vec![RelatedLocation {
            path: "src/b.rs".to_string(),
            line: 20,
            name: Some("beta".to_string()),
        }],
    );

    assert_eq!(left.id, right.id);
    assert!(left.id.starts_with("rf3-"));
}

#[test]
fn finding_ids_are_stable_when_group_representative_rotates() {
    let locations = [
        RelatedLocation {
            path: "src/a.rs".to_string(),
            line: 10,
            name: Some("alpha".to_string()),
        },
        RelatedLocation {
            path: "src/b.rs".to_string(),
            line: 20,
            name: Some("beta".to_string()),
        },
        RelatedLocation {
            path: "src/c.rs".to_string(),
            line: 30,
            name: Some("gamma".to_string()),
        },
    ];
    let metric = || FindingMetric::threshold(MetricId::GroupSize, 3, 3, "functions");
    let left = make_finding(
        FindingKind::SimilarFunctions,
        "src/a.rs",
        Some(10),
        "group",
        vec![metric()],
        locations.to_vec(),
    );
    let right = make_finding(
        FindingKind::SimilarFunctions,
        "src/b.rs",
        Some(20),
        "group",
        vec![metric()],
        locations.into_iter().rev().collect(),
    );

    assert_eq!(left.id, right.id);
}

#[test]
fn renders_sarif_report_with_rules_results_and_fingerprints() {
    let mut findings = vec![make_finding(
        FindingKind::LargeFile,
        "src/a.rs",
        Some(1),
        "file is large",
        vec![FindingMetric::threshold(
            MetricId::FileLoc,
            900,
            800,
            "lines",
        )],
        Vec::new(),
    )];
    let issues = crate::scoring::cluster_findings(&mut findings);
    let mut scan_report = report(findings);
    scan_report.issues = issues;

    let value: serde_json::Value =
        serde_json::from_str(&render_sarif_report(&scan_report)).unwrap();

    assert_eq!(value["version"], "2.1.0");
    assert_eq!(value["runs"][0]["tool"]["driver"]["name"], "Reforge");
    assert_eq!(
        value["runs"][0]["tool"]["driver"]["rules"][0]["id"],
        "responsibility_decomposition"
    );
    assert_eq!(
        value["runs"][0]["results"][0]["ruleId"],
        "responsibility_decomposition"
    );
    assert_eq!(value["runs"][0]["results"][0]["level"], "warning");
    assert_eq!(
        value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "src/a.rs"
    );
    assert_eq!(
        value["runs"][0]["results"][0]["partialFingerprints"]["reforgeIssueId"],
        scan_report.issues[0].id.to_string()
    );
    assert_eq!(
        value["runs"][0]["results"][0]["properties"]["evidence_ids"][0],
        scan_report.findings[0].id.as_str()
    );
}

#[test]
fn renders_html_report_with_react_shell_and_embedded_report_data() {
    let mut scan_report = report(vec![make_finding(
        FindingKind::SimilarFunctions,
        "src/a.rs",
        Some(10),
        "similar functions",
        vec![FindingMetric::threshold(
            MetricId::GroupSize,
            3,
            3,
            "functions",
        )],
        vec![
            RelatedLocation {
                path: "src/a.rs".to_string(),
                line: 10,
                name: Some("alpha".to_string()),
            },
            RelatedLocation {
                path: "src/b.rs".to_string(),
                line: 20,
                name: Some("beta".to_string()),
            },
        ],
    )]);
    scan_report.raw_metrics.files.push(FileRawMetric {
        path: "src/a.rs".to_string(),
        loc: 120,
        imports: 8,
        public_items: 4,
        is_test: false,
        churn: ChurnFileMetric {
            commits_touched: 2,
            lines_added: 20,
            lines_deleted: 5,
            authors_count: 1,
            recent_weighted_churn: 8,
        },
    });
    scan_report.dependency_graph = DependencyGraphSnapshot {
        nodes: vec![
            DependencyGraphNode {
                path: "src/a.rs".to_string(),
                fan_in: 1,
                fan_out: 2,
            },
            DependencyGraphNode {
                path: "src/b.rs".to_string(),
                fan_in: 1,
                fan_out: 1,
            },
            DependencyGraphNode {
                path: "src/c.rs".to_string(),
                fan_in: 2,
                fan_out: 0,
            },
        ],
        edges: vec![
            DependencyGraphEdge {
                from: "src/a.rs".to_string(),
                to: "src/b.rs".to_string(),
            },
            DependencyGraphEdge {
                from: "src/a.rs".to_string(),
                to: "src/c.rs".to_string(),
            },
            DependencyGraphEdge {
                from: "src/b.rs".to_string(),
                to: "src/c.rs".to_string(),
            },
        ],
    };
    scan_report.summary.hotspot_count = 1;
    scan_report.hotspots.push(Hotspot {
        level: HotspotLevel::Function,
        path: "src/a.rs".to_string(),
        line: Some(10),
        name: Some("alpha".to_string()),
        priority: 62,
        severity: Severity::Warning,
        static_risk: 0.6,
        churn_risk: 0.4,
        reason: "hybrid model: repeated edits and high static risk".to_string(),
    });

    let output = render_html_report(&scan_report);

    assert!(output.starts_with("<!doctype html>"));
    assert!(output.contains("<title>Reforge scan report</title>"));
    assert!(output.contains("id=\"reforge-report-root\""));
    assert!(output.contains("id=\"reforge-report-data\" type=\"application/json\""));
    assert!(output.contains("<script type=\"module\">"));
    assert!(output.contains("createRoot"));
    assert!(output.contains(&format!(
        "\"schema_version\":{}",
        SCAN_REPORT_SCHEMA_VERSION
    )));
    assert!(output.contains("\"dependency_graph\""));
    assert!(output.contains("\"hotspots\""));
    assert!(output.contains("\"suppression_summary\""));
    assert!(output.contains("\"raw_metrics\""));
    assert!(output.contains("src/c.rs"));
    assert!(output.contains("similar_functions"));
    assert!(output.contains("repeated edits and high static risk"));
}

#[test]
fn html_report_embeds_all_findings_for_client_side_rendering() {
    let findings = (0..12)
        .map(|index| large_file(&format!("src/file_{index}.rs"), 1_200 + index))
        .collect::<Vec<_>>();

    let output = render_html_report(&report(findings));

    assert!(output.contains("src/file_0.rs"));
    assert!(output.contains("src/file_11.rs"));
    assert!(!output.contains("more findings omitted"));
}

#[test]
fn html_report_embedded_assets_do_not_close_raw_text_elements() {
    let output = render_html_report(&report(Vec::new()));

    assert_eq!(output.matches("</style>").count(), 1);
    assert_eq!(output.matches("</script>").count(), 2);
}

#[test]
fn html_report_escapes_json_before_embedding_it_in_script_data() {
    let output = render_html_report(&report(vec![make_finding(
        FindingKind::LargeFile,
        "src/</script><div>.rs",
        Some(1),
        "file contains </script> marker",
        vec![FindingMetric::threshold(
            MetricId::FileLoc,
            900,
            800,
            "lines",
        )],
        Vec::new(),
    )]));

    assert!(output.contains("src/\\u003c/script\\u003e\\u003cdiv\\u003e.rs"));
    assert!(!output.contains("src/</script><div>.rs"));
    assert!(!output.contains("file contains </script> marker"));
}
