use super::*;
use crate::scanner::{
    FindingMetric, RelatedLocation, SCAN_REPORT_SCHEMA_VERSION, ScanStats, ScanSummary,
    finding as make_finding, scored_finding, severity_for_score,
};

fn report(findings: Vec<Finding>) -> ScanReport {
    ScanReport {
        schema_version: SCAN_REPORT_SCHEMA_VERSION,
        summary: ScanSummary {
            scanned_files: 2,
            finding_count: findings.len(),
            similar_function_group_count: findings
                .iter()
                .filter(|finding| finding.kind == FindingKind::SimilarFunctions)
                .count(),
            duration_ms: 1,
        },
        stats: ScanStats::default(),
        findings,
    }
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
fn maps_score_to_severity() {
    assert_eq!(severity_for_score(39), Severity::Info);
    assert_eq!(severity_for_score(40), Severity::Warning);
    assert_eq!(severity_for_score(74), Severity::Warning);
    assert_eq!(severity_for_score(75), Severity::Critical);
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

    assert!(spread.score > local.score);
}

#[test]
fn large_type_scores_from_strongest_metric() {
    let finding = make_finding(
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

    assert_eq!(finding.score, 90);
    assert_eq!(finding.severity, Severity::Critical);
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
fn human_report_sorts_by_score_and_renders_score_confidence_and_metrics() {
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
    assert!(output.contains("[critical score=100 confidence=1.00]"));
    assert!(output.contains("[warning score=68 confidence=1.00]"));
    assert!(output.contains("file_lines=1200/800 lines"));
}

#[test]
fn renders_colored_human_report_when_enabled() {
    let output = render_human_report_colored(&report(vec![large_file("src/a.rs", 900)]), true);

    assert!(output.contains("\u{1b}[1;36mReforge scan report\u{1b}[0m"));
    assert!(output.contains("\u{1b}[33m[warning score=51 confidence=1.00]\u{1b}[0m"));
}

#[test]
fn renders_json_report_schema_v2_without_magnitude() {
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

    assert_eq!(value["schema_version"], 2);
    assert_eq!(value["summary"]["scanned_files"], 2);
    assert_eq!(value["findings"][0]["kind"], "similar_functions");
    assert_eq!(value["findings"][0]["metrics"][0]["name"], "group_size");
    assert_eq!(value["findings"][0]["score"], 51);
    assert!(value["findings"][0].get("magnitude").is_none());
}

#[test]
fn caps_serialized_similar_function_locations() {
    let scan_report = report(vec![scored_finding(
        FindingKind::SimilarFunctions,
        "src/a.rs",
        Some(1),
        "similar",
        vec![FindingMetric::threshold("group_size", 75, 3, "functions")],
        0.85,
        (0..75)
            .map(|index| RelatedLocation {
                path: format!("src/{index}.rs"),
                line: index + 1,
                name: Some(format!("func_{index}")),
            })
            .collect(),
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
        2
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
        2
    );
}
