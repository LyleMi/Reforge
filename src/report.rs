use std::collections::BTreeMap;
use std::io::{self, Write};

use anyhow::Result;

use crate::scanner::{Finding, FindingKind, ScanReport, Severity};

pub fn print_human_report(report: &ScanReport) -> io::Result<()> {
    write_human_report(std::io::stdout().lock(), report)
}

pub fn print_json_report(report: &ScanReport) -> Result<()> {
    write_json_report(std::io::stdout().lock(), report)
}

pub fn write_human_report(mut writer: impl Write, report: &ScanReport) -> io::Result<()> {
    writer.write_all(render_human_report(report).as_bytes())
}

pub fn write_json_report(mut writer: impl Write, report: &ScanReport) -> Result<()> {
    writer.write_all(serde_json::to_string_pretty(report)?.as_bytes())?;
    writer.write_all(b"\n")?;
    Ok(())
}

pub fn render_human_report(report: &ScanReport) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "Scanned {} files in {} ms; {} findings; {} similar function groups.\n",
        report.summary.scanned_files,
        report.summary.duration_ms,
        report.summary.finding_count,
        report.summary.similar_function_group_count
    ));

    if report.findings.is_empty() {
        output.push_str("No refactoring signals found.\n");
        return output;
    }

    let mut by_path: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
    for finding in sorted_findings(&report.findings) {
        by_path.entry(&finding.path).or_default().push(finding);
    }

    for (path, findings) in by_path {
        output.push('\n');
        output.push_str(path);
        output.push('\n');

        for finding in findings {
            output.push_str("  ");
            output.push_str(&render_finding_line(finding));
            output.push('\n');

            if finding.kind == FindingKind::SimilarFunctions {
                output.push_str(&render_related_locations(finding));
            }
        }
    }

    output
}

fn sorted_findings(findings: &[Finding]) -> Vec<&Finding> {
    let mut sorted: Vec<&Finding> = findings.iter().collect();

    sorted.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| match (left.magnitude, right.magnitude) {
                (Some(left_magnitude), Some(right_magnitude)) => right_magnitude
                    .cmp(&left_magnitude)
                    .then_with(|| left.line.cmp(&right.line)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => left.line.cmp(&right.line),
            })
    });

    sorted
}

fn render_finding_line(finding: &Finding) -> String {
    let location = finding
        .line
        .map(|line| format!(":{line}"))
        .unwrap_or_default();
    format!("[{}]{} {}", finding.severity, location, finding.message)
}

fn render_related_locations(finding: &Finding) -> String {
    let mut output = String::new();

    for location in finding.related_locations.iter().take(6) {
        output.push_str("    - ");
        output.push_str(&location.path);
        output.push(':');
        output.push_str(&location.line.to_string());
        if let Some(name) = &location.name {
            output.push(' ');
            output.push_str(name);
        }
        output.push('\n');
    }

    if finding.related_locations.len() > 6 {
        output.push_str(&format!(
            "    +{} more\n",
            finding.related_locations.len() - 6
        ));
    }

    output
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}

#[cfg(test)]
mod tests {
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

        assert!(output.contains("Scanned 2 files"));
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

        assert!(output.contains("+1 more"));
        assert!(!output.contains("func_6"));
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
}
