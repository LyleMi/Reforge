use super::*;
use crate::scanner::{
    ChurnSummary, FindingInput, FindingMetric, MetricsSummary, RawMetrics, RelatedLocation,
    SCAN_REPORT_SCHEMA_VERSION, ScanStats, ScanSummary, finding, scored_finding,
    severity_for_priority,
};

fn report(findings: Vec<Finding>) -> ScanReport {
    ScanReport {
        schema_version: SCAN_REPORT_SCHEMA_VERSION,
        summary: ScanSummary {
            scanned_files: 2,
            finding_count: findings.len(),
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
            files: BTreeMap::new(),
            functions: BTreeMap::new(),
            types: BTreeMap::new(),
            churn: BTreeMap::new(),
        },
        raw_metrics: RawMetrics::default(),
        hotspots: Vec::new(),
        findings,
    }
}

fn make_finding(
    kind: FindingKind,
    path: impl Into<String>,
    line: Option<usize>,
    message: impl Into<String>,
    metrics: Vec<FindingMetric>,
    related_locations: Vec<RelatedLocation>,
) -> Finding {
    finding(
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
        vec![FindingMetric::threshold("file_lines", lines, 800, "lines")],
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
    let metric = FindingMetric::threshold("file_lines", 1_200, 800, "lines");

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
        vec![FindingMetric::threshold("group_size", 3, 3, "functions")],
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
        vec![FindingMetric::threshold("group_size", 3, 3, "functions")],
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
            FindingMetric::threshold("type_lines", 260, 250, "lines"),
            FindingMetric::threshold("type_members", 60, 30, "members"),
        ],
        Vec::new(),
    );
    let severe = make_finding(
        FindingKind::LargeType,
        "src/types.rs",
        Some(1),
        "large type",
        vec![
            FindingMetric::threshold("type_lines", 260, 250, "lines"),
            FindingMetric::threshold("type_members", 120, 30, "members"),
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

    assert!(output.contains("Reforge scan report"));
    assert!(output.contains("Scanned 2 files"));
    assert!(output.contains("Summary"));
    assert!(output.contains("Signals"));
    assert!(output.contains("No refactoring signals found."));
}

#[test]
fn human_report_sorts_by_priority_and_renders_priority_confidence_and_metrics() {
    let critical = make_finding(
        FindingKind::ComplexFunction,
        "src/critical.rs",
        Some(10),
        "complex",
        vec![FindingMetric::threshold(
            "function_complexity",
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
    assert!(output.contains("[warning priority=58 confidence=1.00]"));
    assert!(output.contains("[warning priority=48 confidence=1.00]"));
    assert!(output.contains("file_lines=1200/800 lines"));
    assert!(output.contains("high complexity, high confidence"));
}

#[test]
fn renders_colored_human_report_when_enabled() {
    let output = render_human_report_colored(&report(vec![large_file("src/a.rs", 900)]), true);

    assert!(output.contains("\u{1b}[1;36mReforge scan report\u{1b}[0m"));
    assert!(output.contains("\u{1b}[33m[warning priority=47 confidence=1.00]\u{1b}[0m"));
}

#[test]
fn renders_json_report_schema_v6_with_priority_metadata() {
    let scan_report = report(vec![make_finding(
        FindingKind::SimilarFunctions,
        "src/a.rs",
        Some(1),
        "similar",
        vec![FindingMetric::threshold("group_size", 3, 3, "functions")],
        vec![RelatedLocation {
            path: "src/a.rs".to_string(),
            line: 1,
            name: Some("alpha".to_string()),
        }],
    )]);

    let value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&scan_report).unwrap()).unwrap();

    assert_eq!(value["schema_version"], 6);
    assert_eq!(value["summary"]["scanned_files"], 2);
    assert_eq!(value["summary"]["hotspot_model"], "hybrid");
    assert!(value.get("metrics_summary").is_some());
    assert!(value.get("raw_metrics").is_some());
    assert!(value.get("hotspots").is_some());
    assert_eq!(value["findings"][0]["kind"], "similar_functions");
    assert_eq!(value["findings"][0]["metrics"][0]["name"], "group_size");
    assert_eq!(
        value["findings"][0]["metrics"][0]["dimension"],
        "duplication"
    );
    assert!(value["findings"][0]["metrics"][0]["normalized"].is_number());
    assert_eq!(value["findings"][0]["priority"], 37);
    assert!(value["findings"][0]["priority_factors"]["impact"].is_number());
    assert!(value["findings"][0]["score"].is_null());
    assert!(value["findings"][0]["score_breakdown"].is_null());
    assert!(value["findings"][0]["rank_reason"].is_null());
    assert_eq!(
        value["findings"][0]["rank_explanation"],
        "duplication signal, high confidence"
    );
    assert!(value["findings"][0].get("magnitude").is_none());
}

#[test]
fn caps_serialized_similar_function_locations() {
    let scan_report = report(vec![scored_finding(
        FindingInput::new(
            FindingKind::SimilarFunctions,
            "src/a.rs",
            Some(1),
            "similar",
            vec![FindingMetric::threshold("group_size", 75, 3, "functions")],
        )
        .with_confidence(0.85)
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
        6
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
        6
    );
}
