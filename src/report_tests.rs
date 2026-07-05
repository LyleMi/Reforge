use super::*;
use crate::scanner::{RelatedLocation, ScanStats, ScanSummary};

fn finding(path: &str, magnitude: Option<usize>) -> Finding {
    Finding {
        kind: if magnitude.is_some() {
            FindingKind::LargeFile
        } else {
            FindingKind::DebtMarker
        },
        severity: if magnitude.is_some() {
            Severity::Warning
        } else {
            Severity::Info
        },
        path: path.to_string(),
        line: Some(1),
        magnitude,
        message: String::new(),
        related_locations: Vec::new(),
    }
}

fn report(findings: Vec<Finding>) -> ScanReport {
    ScanReport {
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

#[test]
fn sorts_by_path_then_large_findings_before_line_findings() {
    let findings = vec![
        finding("src/small_todo.rs", None),
        finding("src/large.rs", Some(900)),
        finding("src/largest.rs", Some(1_200)),
        finding("src/medium.rs", Some(1_000)),
        finding("src/another_todo.rs", None),
    ];

    let paths: Vec<&str> = sorted_findings(&findings)
        .iter()
        .map(|finding| finding.path.as_str())
        .collect();

    assert_eq!(
        paths,
        vec![
            "src/another_todo.rs",
            "src/large.rs",
            "src/largest.rs",
            "src/medium.rs",
            "src/small_todo.rs",
        ]
    );
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
fn renders_multiple_findings_grouped_by_path() {
    let output = render_human_report(&report(vec![
        finding("src/a.rs", Some(900)),
        finding("src/a.rs", None),
    ]));

    assert_eq!(output.matches("src/a.rs").count(), 1);
    assert_eq!(output.matches("[warning]").count(), 1);
    assert_eq!(output.matches("[info]").count(), 1);
}

#[test]
fn truncates_similar_function_locations() {
    let mut finding = finding("src/a.rs", Some(7));
    finding.kind = FindingKind::SimilarFunctions;
    finding.message =
        "7 structurally similar functions/methods found at similarity >= 0.80".to_string();
    finding.related_locations = (0..7)
        .map(|index| RelatedLocation {
            path: format!("src/{index}.rs"),
            line: index + 1,
            name: Some(format!("func_{index}")),
        })
        .collect();

    let output = render_human_report(&report(vec![finding]));

    assert!(output.contains("similar functions: 7 functions"));
    assert!(output.contains("+4 more"));
    assert!(!output.contains("func_3"));
}

#[test]
fn groups_debt_markers_by_path_in_human_report() {
    let findings = (1..=8)
        .map(|line| Finding {
            line: Some(line),
            ..finding("src/a.rs", None)
        })
        .collect::<Vec<_>>();

    let output = render_human_report(&report(findings));

    assert!(output.contains("Debt markers: 8"));
    assert!(output.contains("[info] 8 debt markers: lines 1, 2, 3, 4, 5, 6 (+2 more)"));
}

#[test]
fn renders_colored_human_report_when_enabled() {
    let output = render_human_report_colored(&report(vec![finding("src/a.rs", Some(900))]), true);

    assert!(output.contains("\u{1b}[1;36mReforge scan report\u{1b}[0m"));
    assert!(output.contains("\u{1b}[33m[warning]\u{1b}[0m"));
}

#[test]
fn renders_json_report_shape() {
    let report = ScanReport {
        summary: ScanSummary {
            scanned_files: 1,
            finding_count: 1,
            similar_function_group_count: 1,
            duration_ms: 1,
        },
        stats: ScanStats {
            source_files_scanned: 1,
            directories_scanned: 1,
            function_candidates: 3,
        },
        findings: vec![Finding {
            kind: FindingKind::SimilarFunctions,
            severity: Severity::Warning,
            path: "src/a.rs".to_string(),
            line: Some(1),
            magnitude: Some(3),
            message: "similar".to_string(),
            related_locations: vec![RelatedLocation {
                path: "src/a.rs".to_string(),
                line: 1,
                name: Some("alpha".to_string()),
            }],
        }],
    };

    let value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();

    assert_eq!(value["summary"]["scanned_files"], 1);
    assert_eq!(value["stats"]["function_candidates"], 3);
    assert_eq!(value["findings"][0]["kind"], "similar_functions");
    assert_eq!(
        value["findings"][0]["related_locations"][0]["name"],
        "alpha"
    );
}

#[test]
fn renders_agent_drift_signal_counts_and_related_locations() {
    let kinds = [
        FindingKind::ParallelImplementation,
        FindingKind::ShadowedAbstraction,
        FindingKind::DuplicateTypeShape,
        FindingKind::ConfigKeyDrift,
        FindingKind::FixtureFactoryDrift,
        FindingKind::GenericBucketDrift,
        FindingKind::AdapterBoundaryBypass,
    ];
    let findings = kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| Finding {
            kind: *kind,
            severity: Severity::Warning,
            path: "src/agent.rs".to_string(),
            line: Some(index + 1),
            magnitude: Some(index + 2),
            message: String::new(),
            related_locations: vec![RelatedLocation {
                path: format!("src/related_{index}.rs"),
                line: index + 10,
                name: Some(format!("related_{index}")),
            }],
        })
        .collect::<Vec<_>>();

    let output = render_human_report(&report(findings));

    assert!(output.contains("Parallel implementations: 1"));
    assert!(output.contains("Shadowed abstractions: 1"));
    assert!(output.contains("Duplicate type shapes: 1"));
    assert!(output.contains("Config key drift: 1"));
    assert!(output.contains("Fixture factory drift: 1"));
    assert!(output.contains("Generic bucket drift: 1"));
    assert!(output.contains("Adapter boundary bypasses: 1"));
    assert!(output.contains("parallel implementation: 2 implementations"));
    assert!(output.contains("src/related_0.rs:10 related_0"));
}

#[test]
fn serializes_agent_drift_kind_as_snake_case() {
    let report = report(vec![Finding {
        kind: FindingKind::ParallelImplementation,
        severity: Severity::Warning,
        path: "src/agent.rs".to_string(),
        line: Some(1),
        magnitude: Some(2),
        message: String::new(),
        related_locations: Vec::new(),
    }]);

    let value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();

    assert_eq!(value["findings"][0]["kind"], "parallel_implementation");
}

#[test]
fn writes_json_report_to_writer() {
    let mut output = Vec::new();

    write_json_report(&mut output, &report(Vec::new())).unwrap();

    let output = String::from_utf8(output).unwrap();
    assert!(output.ends_with('\n'));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&output).unwrap()["summary"]["scanned_files"],
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
        serde_yaml::from_str::<serde_yaml::Value>(&output).unwrap()["summary"]["scanned_files"],
        2
    );
}

#[test]
fn renders_new_signal_counts_and_snake_case_json_kind() {
    let finding = Finding {
        kind: FindingKind::LongFunction,
        severity: Severity::Warning,
        path: "src/a.rs".to_string(),
        line: Some(10),
        magnitude: Some(90),
        message: "long".to_string(),
        related_locations: Vec::new(),
    };
    let report = report(vec![finding]);

    let human = render_human_report(&report);
    assert!(human.contains("Long functions: 1"));
    assert!(human.contains("long function: 90 lines"));

    let value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
    assert_eq!(value["findings"][0]["kind"], "long_function");
}

#[test]
fn renders_test_and_naming_signal_counts() {
    let report = report(vec![
        Finding {
            kind: FindingKind::HappyPathOnlyTests,
            severity: Severity::Info,
            path: "tests/user.test.js".to_string(),
            line: Some(2),
            magnitude: Some(3),
            message: String::new(),
            related_locations: Vec::new(),
        },
        Finding {
            kind: FindingKind::FileNamingDrift,
            severity: Severity::Warning,
            path: "src/payments".to_string(),
            line: None,
            magnitude: Some(3),
            message: String::new(),
            related_locations: Vec::new(),
        },
    ]);

    let human = render_human_report(&report);
    assert!(human.contains("Happy-path-only tests: 1"));
    assert!(human.contains("File naming drift: 1"));
    assert!(human.contains("happy-path-only tests: 3 test cases"));
    assert!(human.contains("file naming drift: 3 naming styles"));

    let value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
    assert_eq!(value["findings"][0]["kind"], "happy_path_only_tests");
    assert_eq!(value["findings"][1]["kind"], "file_naming_drift");
}
