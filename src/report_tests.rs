use super::*;
use crate::model::{
    DependencyGraphEdge, DependencyGraphNode, DependencyGraphSnapshot, Hotspot, HotspotLevel,
    MetricId,
};
use crate::scanner::{
    ChurnFileMetric, ChurnSummary, FileRawMetric, FindingInput, FindingMetric, MetricsSummary,
    RawMetrics, RelatedLocation, SCAN_REPORT_SCHEMA_VERSION, ScanStats, ScanSummary,
    SuppressionSummary, severity_for_priority,
};

fn report(findings: Vec<Finding>) -> ScanReport {
    let mut findings = findings;
    let issues = crate::scoring::cluster_findings(&mut findings);
    ScanReport {
        schema_version: SCAN_REPORT_SCHEMA_VERSION,
        summary: ScanSummary {
            scanned_files: 2,
            finding_count: findings.len(),
            issue_count: findings.len(),
            hotspot_count: 0,
            similar_function_group_count: findings
                .iter()
                .filter(|finding| finding.kind == FindingKind::SimilarFunctions)
                .count(),
            duration_ms: 1,
            hotspot_model: crate::cli::HotspotModel::Hybrid,
            churn: ChurnSummary {
                mode: crate::cli::ChurnMode::Auto,
                enabled: false,
                status: "unavailable".to_string(),
                reason: None,
                window_days: 180,
                max_commit_lines: 2_000,
            },
        },
        stats: ScanStats::default(),
        metrics_summary: MetricsSummary {
            directories: BTreeMap::new(),
            files: BTreeMap::new(),
            functions: BTreeMap::new(),
            types: BTreeMap::new(),
            churn: BTreeMap::new(),
        },
        raw_metrics: RawMetrics::default(),
        raw_metric_manifest: Vec::new(),
        dependency_graph: DependencyGraphSnapshot::default(),
        unity_project: crate::model::UnityProjectReport::default(),
        hotspots: Vec::new(),
        suppression_summary: SuppressionSummary::default(),
        coverage_manifest: Vec::new(),
        coverage_summary: crate::model::CoverageSummary::default(),
        detector_execution: Vec::new(),
        raw_metric_coverage: Vec::new(),
        scoring_policy: crate::model::EffectiveScoringPolicy::builtin(),
        issues,
        detector_manifest: Vec::new(),
        findings,
    }
}

fn report_with_hotspots(findings: Vec<Finding>, hotspots: Vec<Hotspot>) -> ScanReport {
    let mut report = report(findings);
    report.summary.hotspot_count = hotspots.len();
    report.hotspots = hotspots;
    report
}

#[test]
fn human_report_renders_cluster_primary_once_and_keeps_raw_signal_count() {
    let mut findings = vec![
        make_finding(
            FindingKind::ComplexFunction,
            "src/a.rs",
            Some(10),
            "complex",
            vec![FindingMetric::threshold(
                MetricId::FunctionComplexity,
                20,
                15,
                "complexity",
            )],
            Vec::new(),
        ),
        make_finding(
            FindingKind::DeepNesting,
            "src/a.rs",
            Some(10),
            "nested",
            vec![FindingMetric::threshold(
                MetricId::FunctionNestingDepth,
                6,
                4,
                "levels",
            )],
            Vec::new(),
        ),
    ];
    let clusters = crate::scoring::cluster_findings(&mut findings);
    let mut scan_report = report(findings);
    scan_report.summary.issue_count = 1;
    scan_report.issues = clusters;

    let output = render_human_report(&scan_report);

    assert!(output.contains("Issues               1"));
    assert!(output.contains("Raw signals          2"));
    assert!(output.contains("cluster 2 raw signals"));
    assert_eq!(output.matches("src/a.rs:10").count(), 1);
}

fn make_finding(
    kind: FindingKind,
    path: impl Into<String>,
    line: Option<usize>,
    message: impl Into<String>,
    metrics: Vec<FindingMetric>,
    related_locations: Vec<RelatedLocation>,
) -> Finding {
    Finding::from(
        FindingInput::new(kind, path, line, message, metrics)
            .with_related_locations(related_locations),
    )
}

fn large_file(path: &str, lines: usize) -> Finding {
    make_finding(
        FindingKind::LargeFile,
        path,
        Some(1),
        "",
        vec![FindingMetric::threshold(
            MetricId::FileLoc,
            lines,
            800,
            "lines",
        )],
        Vec::new(),
    )
}

#[test]
fn maps_priority_to_severity() {
    assert_eq!(severity_for_priority(34), Severity::Info);
    assert_eq!(severity_for_priority(35), Severity::Warning);
    assert_eq!(severity_for_priority(69), Severity::Warning);
    assert_eq!(severity_for_priority(70), Severity::Critical);
}

#[test]
fn calculates_threshold_excess_ratio() {
    let metric = FindingMetric::threshold(MetricId::FileLoc, 1_200, 800, "lines");

    assert_eq!(metric.threshold, Some(800));
    assert_eq!(metric.excess_ratio, Some(1.5));
}

#[test]
fn spread_factor_increases_score_for_cross_file_groups() {
    let local = make_finding(
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
            name: None,
        }],
    );
    let spread = make_finding(
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
        vec![
            RelatedLocation {
                path: "src/a.rs".to_string(),
                line: 1,
                name: None,
            },
            RelatedLocation {
                path: "src/b.rs".to_string(),
                line: 1,
                name: None,
            },
            RelatedLocation {
                path: "src/c.rs".to_string(),
                line: 1,
                name: None,
            },
        ],
    );

    assert!(spread.priority > local.priority);
}

#[test]
fn large_type_scores_from_strongest_metric() {
    let moderate = make_finding(
        FindingKind::LargeType,
        "src/types.rs",
        Some(1),
        "large type",
        vec![
            FindingMetric::threshold(MetricId::TypeLoc, 260, 250, "lines"),
            FindingMetric::threshold(MetricId::TypeMemberCount, 60, 30, "members"),
        ],
        Vec::new(),
    );
    let severe = make_finding(
        FindingKind::LargeType,
        "src/types.rs",
        Some(1),
        "large type",
        vec![
            FindingMetric::threshold(MetricId::TypeLoc, 260, 250, "lines"),
            FindingMetric::threshold(MetricId::TypeMemberCount, 120, 30, "members"),
        ],
        Vec::new(),
    );

    assert!(severe.priority > moderate.priority);
    assert_eq!(moderate.severity, Severity::Warning);
    assert_eq!(severe.severity, Severity::Warning);
}

#[test]
fn renders_empty_human_report_clearly() {
    let output = render_human_report(&report(Vec::new()));

    assert!(output.contains("Reforge scan"));
    assert!(output.contains("2 files"));
    assert!(output.contains("Result"));
    assert!(output.contains("Issues               0"));
    assert!(output.contains("Raw signals          0  critical 0 | warning 0 | info 0"));
    assert!(output.contains("Scan details"));
    assert!(output.contains("No threshold signals found."));
}

#[test]
fn human_report_sorts_by_priority_and_renders_priority_confidence_and_metrics() {
    let critical = make_finding(
        FindingKind::ComplexFunction,
        "src/critical.rs",
        Some(10),
        "complex",
        vec![FindingMetric::threshold(
            MetricId::FunctionComplexity,
            30,
            15,
            "complexity",
        )],
        Vec::new(),
    );
    let warning = large_file("src/warning.rs", 1_200);
    let info = make_finding(
        FindingKind::DebtMarker,
        "src/info.rs",
        Some(3),
        "technical-debt marker found",
        Vec::new(),
        Vec::new(),
    );

    let output = render_human_report(&report(vec![info, warning, critical]));

    let critical_index = output.find("src/critical.rs:10").unwrap();
    let warning_index = output.find("src/warning.rs:1").unwrap();
    let info_index = output.find("src/info.rs:3").unwrap();
    assert!(critical_index < warning_index);
    assert!(warning_index < info_index);
    assert!(output.contains("warning  p=52 c=0.90"));
    assert!(output.matches("c=0.90").count() >= 2);
    assert!(output.contains("Signal mix"));
    assert!(output.contains("large file"));
    assert!(output.contains("metrics file.loc=1200/800 lines"));
    assert!(output.contains("rank cognitive-load signal, high action probability"));
}

#[test]
fn renders_colored_human_report_when_enabled() {
    let output = render_human_report_colored(&report(vec![large_file("src/a.rs", 900)]), true);

    assert!(output.contains("\u{1b}[1;36mReforge scan\u{1b}[0m"));
    assert!(output.contains("\u{1b}[1;33mwarning \u{1b}[0m p=42 c=0.90"));
}

#[test]
fn renders_hotspots_even_when_no_findings() {
    let output = render_human_report(&report_with_hotspots(
        Vec::new(),
        vec![Hotspot {
            level: HotspotLevel::File,
            path: "src/hot.rs".to_string(),
            line: Some(12),
            name: None,
            priority: 61,
            severity: Severity::Warning,
            static_risk: 0.4,
            churn_risk: 0.9,
            reason: "hybrid model: churn dominates".to_string(),
        }],
    ));

    assert!(output.contains("Watchlist            1 hotspots"));
    assert!(output.contains("Watchlist\n"));
    assert!(output.contains("warning   61  src/hot.rs:12"));
    assert!(output.contains("No threshold signals found."));
}

#[test]
fn renders_suppression_summary_when_findings_are_suppressed() {
    let mut scan_report = report(Vec::new());
    scan_report.suppression_summary.suppressed_count = 2;
    scan_report
        .suppression_summary
        .suppressed_by_severity
        .insert(Severity::Warning, 1);
    scan_report
        .suppression_summary
        .suppressed_by_severity
        .insert(Severity::Info, 1);
    scan_report.suppression_summary.highest_suppressed_priority = Some(58);

    let output = render_human_report(&scan_report);

    assert!(output.contains(
        "Suppressed           2 findings (highest p=58); critical 0 | warning 1 | info 1"
    ));
    assert!(output.contains("No threshold signals found."));
}

#[test]
fn renders_human_baseline_diff_summary_and_selected_findings() {
    let same = large_file("src/same.rs", 900);
    let mut old_worse = large_file("src/worse.rs", 900);
    let worse = large_file("src/worse.rs", 1_300);
    let new = large_file("src/new.rs", 900);
    let resolved = large_file("src/resolved.rs", 900);
    old_worse.priority = old_worse.priority.saturating_sub(1);
    let baseline = report(vec![same.clone(), old_worse, resolved]);
    let scan_report = report(vec![same, worse, new]);
    let diff = crate::baseline::diff_issues(
        &scan_report.issues,
        &baseline,
        crate::cli::BaselineShow::NewOrWorse,
    );

    let output = render_human_report_with_baseline(&scan_report, &diff);

    assert!(output.contains("Baseline diff"));
    assert!(
        output
            .lines()
            .any(|line| line.contains("New") && line.ends_with("1"))
    );
    assert!(
        output
            .lines()
            .any(|line| line.contains("Worse") && line.ends_with("1"))
    );
    assert!(
        output
            .lines()
            .any(|line| line.contains("Same") && line.ends_with("1"))
    );
    assert!(
        output
            .lines()
            .any(|line| line.contains("Resolved") && line.ends_with("1"))
    );
    assert!(output.contains("Issues (new or worse)"));
    assert!(output.contains("worse    warning"));
    assert!(output.contains("new      warning"));
    assert!(output.contains("src/worse.rs:1"));
    assert!(output.contains("src/new.rs:1"));
    assert!(!output.contains("src/same.rs:1"));
    assert!(!output.contains("src/resolved.rs:1"));
}

#[test]
fn renders_human_baseline_diff_when_selected_findings_are_empty() {
    let same = large_file("src/same.rs", 900);
    let resolved = large_file("src/resolved.rs", 900);
    let baseline = report(vec![same.clone(), resolved]);
    let scan_report = report(vec![same]);
    let diff = crate::baseline::diff_issues(
        &scan_report.issues,
        &baseline,
        crate::cli::BaselineShow::New,
    );

    let output = render_human_report_with_baseline(&scan_report, &diff);

    assert!(output.contains("Baseline diff"));
    assert!(output.contains("Issues (new)"));
    assert!(output.contains("No issues matched --show new."));
    assert!(!output.contains("src/same.rs:1"));
}

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
