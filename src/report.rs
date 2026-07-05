use std::collections::BTreeMap;
use std::io::{self, Write};

use anyhow::Result;

use crate::scanner::{Finding, FindingKind, ScanReport, Severity};

const RELATED_LOCATION_LIMIT: usize = 3;
const DEBT_MARKER_LINE_LIMIT: usize = 6;

pub fn print_human_report(report: &ScanReport) -> io::Result<()> {
    write_human_report_colored(std::io::stdout().lock(), report, false)
}

pub fn print_human_report_colored(report: &ScanReport, color: bool) -> io::Result<()> {
    write_human_report_colored(std::io::stdout().lock(), report, color)
}

pub fn print_json_report(report: &ScanReport) -> Result<()> {
    write_json_report(std::io::stdout().lock(), report)
}

pub fn print_yaml_report(report: &ScanReport) -> Result<()> {
    write_yaml_report(std::io::stdout().lock(), report)
}

pub fn write_human_report(mut writer: impl Write, report: &ScanReport) -> io::Result<()> {
    writer.write_all(render_human_report(report).as_bytes())
}

pub fn write_human_report_colored(
    mut writer: impl Write,
    report: &ScanReport,
    color: bool,
) -> io::Result<()> {
    writer.write_all(render_human_report_colored(report, color).as_bytes())
}

pub fn write_json_report(mut writer: impl Write, report: &ScanReport) -> Result<()> {
    writer.write_all(serde_json::to_string_pretty(report)?.as_bytes())?;
    writer.write_all(b"\n")?;
    Ok(())
}

pub fn write_yaml_report(mut writer: impl Write, report: &ScanReport) -> Result<()> {
    let output = serde_yaml::to_string(report)?;
    writer.write_all(output.as_bytes())?;
    if !output.ends_with('\n') {
        writer.write_all(b"\n")?;
    }
    Ok(())
}

pub fn render_human_report(report: &ScanReport) -> String {
    render_human_report_colored(report, false)
}

pub fn render_human_report_colored(report: &ScanReport, color: bool) -> String {
    let mut output = String::new();
    let breakdown = FindingBreakdown::from_findings(&report.findings);

    output.push_str(&paint(color, "Reforge scan report", AnsiStyle::Header));
    output.push('\n');
    output.push_str(&format!(
        "Scanned {} files in {} ms; {} findings; {} similar function groups.\n",
        report.summary.scanned_files,
        report.summary.duration_ms,
        report.summary.finding_count,
        report.summary.similar_function_group_count
    ));
    output.push('\n');
    output.push_str(&paint(color, "Summary", AnsiStyle::Section));
    output.push('\n');
    output.push_str(&format!(
        "  Source files: {}\n  Directories: {}\n  Function candidates: {}\n",
        report.stats.source_files_scanned,
        report.stats.directories_scanned,
        report.stats.function_candidates
    ));
    output.push('\n');
    output.push_str(&paint(color, "Signals", AnsiStyle::Section));
    output.push('\n');
    output.push_str(&format!(
        "  Warnings: {}\n  Info: {}\n  Large files: {}\n  Large directories: {}\n  Debt markers: {}\n  Similar function groups: {}\n  Long functions: {}\n  Complex functions: {}\n  Deep nesting: {}\n  Many parameters: {}\n  Large types: {}\n  Large public surfaces: {}\n  Import-heavy files: {}\n  Repeated literals: {}\n  Repeated error patterns: {}\n  Test duplication: {}\n  Directory drift: {}\n  Data clumps: {}\n",
        breakdown.warnings,
        breakdown.info,
        breakdown.large_files,
        breakdown.large_directories,
        breakdown.debt_markers,
        breakdown.similar_functions,
        breakdown.long_functions,
        breakdown.complex_functions,
        breakdown.deep_nesting,
        breakdown.many_parameters,
        breakdown.large_types,
        breakdown.large_public_surfaces,
        breakdown.import_heavy_files,
        breakdown.repeated_literals,
        breakdown.repeated_error_patterns,
        breakdown.test_duplication,
        breakdown.directory_drift,
        breakdown.data_clumps
    ));

    if report.findings.is_empty() {
        output.push('\n');
        output.push_str("No refactoring signals found.\n");
        return output;
    }

    let mut by_path: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
    for finding in sorted_findings(&report.findings) {
        by_path.entry(&finding.path).or_default().push(finding);
    }

    output.push('\n');
    output.push_str(&paint(color, "Findings", AnsiStyle::Section));
    output.push('\n');

    for (path, findings) in by_path {
        output.push('\n');
        output.push_str(&paint(color, path, AnsiStyle::Path));
        output.push('\n');

        for finding in findings
            .iter()
            .filter(|finding| finding.kind != FindingKind::DebtMarker)
        {
            output.push_str("  ");
            output.push_str(&render_finding_line(finding, color));
            output.push('\n');

            if has_related_location_details(finding) {
                output.push_str(&render_related_locations(finding, color));
            }
        }

        let debt_markers = findings
            .iter()
            .copied()
            .filter(|finding| finding.kind == FindingKind::DebtMarker)
            .collect::<Vec<_>>();
        if !debt_markers.is_empty() {
            output.push_str("  ");
            output.push_str(&render_debt_marker_group(&debt_markers, color));
            output.push('\n');
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

fn render_finding_line(finding: &Finding, color: bool) -> String {
    let location = finding
        .line
        .map(|line| format!(":{line}"))
        .unwrap_or_default();
    let severity = render_severity(&finding.severity, color);
    format!(
        "{severity}{} {}",
        location,
        concise_finding_message(finding)
    )
}

fn concise_finding_message(finding: &Finding) -> String {
    match finding.kind {
        FindingKind::LargeFile => match finding.magnitude {
            Some(lines) => format!("large file: {lines} lines"),
            None => finding.message.clone(),
        },
        FindingKind::LargeDirectory => match finding.magnitude {
            Some(files) => format!("large directory: {files} source files"),
            None => finding.message.clone(),
        },
        FindingKind::DebtMarker => "debt marker".to_string(),
        FindingKind::SimilarFunctions => match finding.magnitude {
            Some(count) => format!(
                "similar functions: {count} {}",
                pluralize(count, "function")
            ),
            None => finding.message.clone(),
        },
        FindingKind::LongFunction => match finding.magnitude {
            Some(lines) => format!("long function: {lines} lines"),
            None => finding.message.clone(),
        },
        FindingKind::ComplexFunction => match finding.magnitude {
            Some(complexity) => format!("complex function: complexity {complexity}"),
            None => finding.message.clone(),
        },
        FindingKind::DeepNesting => match finding.magnitude {
            Some(depth) => format!("deep nesting: {depth} levels"),
            None => finding.message.clone(),
        },
        FindingKind::ManyParameters => match finding.magnitude {
            Some(parameters) => format!("many parameters: {parameters} parameters"),
            None => finding.message.clone(),
        },
        FindingKind::LargeType => match finding.magnitude {
            Some(size) => format!("large type: size {size}"),
            None => finding.message.clone(),
        },
        FindingKind::LargePublicSurface => match finding.magnitude {
            Some(items) => format!("large public surface: {items} items"),
            None => finding.message.clone(),
        },
        FindingKind::ImportHeavyFile => match finding.magnitude {
            Some(imports) => format!("import-heavy file: {imports} imports"),
            None => finding.message.clone(),
        },
        FindingKind::RepeatedLiteral => match finding.magnitude {
            Some(count) => format!("repeated literal: {count} occurrences"),
            None => finding.message.clone(),
        },
        FindingKind::RepeatedErrorPattern => match finding.magnitude {
            Some(count) => format!("repeated error pattern: {count} occurrences"),
            None => finding.message.clone(),
        },
        FindingKind::TestDuplication => match finding.magnitude {
            Some(count) => format!("test duplication: {count} occurrences"),
            None => finding.message.clone(),
        },
        FindingKind::DirectoryDrift => match finding.magnitude {
            Some(concepts) => format!("directory drift: {concepts} concepts"),
            None => finding.message.clone(),
        },
        FindingKind::DataClump => match finding.magnitude {
            Some(count) => format!("data clump: {count} occurrences"),
            None => finding.message.clone(),
        },
    }
}

fn has_related_location_details(finding: &Finding) -> bool {
    matches!(
        finding.kind,
        FindingKind::SimilarFunctions
            | FindingKind::RepeatedLiteral
            | FindingKind::RepeatedErrorPattern
            | FindingKind::TestDuplication
            | FindingKind::DataClump
    )
}

fn render_debt_marker_group(findings: &[&Finding], color: bool) -> String {
    if findings.len() == 1 {
        let location = findings[0]
            .line
            .map(|line| format!(":{line}"))
            .unwrap_or_default();
        return format!(
            "{}{} debt marker",
            render_severity(&findings[0].severity, color),
            location
        );
    }

    let lines = findings
        .iter()
        .filter_map(|finding| finding.line)
        .collect::<Vec<_>>();
    let mut message = format!(
        "{} {} debt markers",
        render_severity(&findings[0].severity, color),
        findings.len()
    );

    if !lines.is_empty() {
        let shown = lines
            .iter()
            .take(DEBT_MARKER_LINE_LIMIT)
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        message.push_str(": lines ");
        message.push_str(&shown);

        if lines.len() > DEBT_MARKER_LINE_LIMIT {
            message.push_str(&format!(
                " (+{} more)",
                lines.len() - DEBT_MARKER_LINE_LIMIT
            ));
        }
    }

    message
}

fn pluralize(count: usize, noun: &str) -> String {
    if count == 1 {
        noun.to_string()
    } else {
        format!("{noun}s")
    }
}

fn render_related_locations(finding: &Finding, color: bool) -> String {
    let mut output = String::new();

    for location in finding
        .related_locations
        .iter()
        .take(RELATED_LOCATION_LIMIT)
    {
        output.push_str("    - ");
        output.push_str(&paint(color, &location.path, AnsiStyle::Path));
        output.push_str(&paint(
            color,
            &format!(":{}", location.line),
            AnsiStyle::Location,
        ));
        if let Some(name) = &location.name {
            output.push(' ');
            output.push_str(name);
        }
        output.push('\n');
    }

    if finding.related_locations.len() > RELATED_LOCATION_LIMIT {
        output.push_str(&format!(
            "    +{} more\n",
            finding.related_locations.len() - RELATED_LOCATION_LIMIT
        ));
    }

    output
}

#[derive(Debug, Default)]
struct FindingBreakdown {
    warnings: usize,
    info: usize,
    large_files: usize,
    large_directories: usize,
    debt_markers: usize,
    similar_functions: usize,
    long_functions: usize,
    complex_functions: usize,
    deep_nesting: usize,
    many_parameters: usize,
    large_types: usize,
    large_public_surfaces: usize,
    import_heavy_files: usize,
    repeated_literals: usize,
    repeated_error_patterns: usize,
    test_duplication: usize,
    directory_drift: usize,
    data_clumps: usize,
}

impl FindingBreakdown {
    fn from_findings(findings: &[Finding]) -> Self {
        let mut breakdown = Self::default();

        for finding in findings {
            match finding.severity {
                Severity::Warning => breakdown.warnings += 1,
                Severity::Info => breakdown.info += 1,
            }

            match finding.kind {
                FindingKind::LargeFile => breakdown.large_files += 1,
                FindingKind::LargeDirectory => breakdown.large_directories += 1,
                FindingKind::DebtMarker => breakdown.debt_markers += 1,
                FindingKind::SimilarFunctions => breakdown.similar_functions += 1,
                FindingKind::LongFunction => breakdown.long_functions += 1,
                FindingKind::ComplexFunction => breakdown.complex_functions += 1,
                FindingKind::DeepNesting => breakdown.deep_nesting += 1,
                FindingKind::ManyParameters => breakdown.many_parameters += 1,
                FindingKind::LargeType => breakdown.large_types += 1,
                FindingKind::LargePublicSurface => breakdown.large_public_surfaces += 1,
                FindingKind::ImportHeavyFile => breakdown.import_heavy_files += 1,
                FindingKind::RepeatedLiteral => breakdown.repeated_literals += 1,
                FindingKind::RepeatedErrorPattern => breakdown.repeated_error_patterns += 1,
                FindingKind::TestDuplication => breakdown.test_duplication += 1,
                FindingKind::DirectoryDrift => breakdown.directory_drift += 1,
                FindingKind::DataClump => breakdown.data_clumps += 1,
            }
        }

        breakdown
    }
}

enum AnsiStyle {
    Header,
    Section,
    Path,
    Location,
    Warning,
    Info,
}

fn render_severity(severity: &Severity, color: bool) -> String {
    let label = format!("[{severity}]");
    let style = match severity {
        Severity::Warning => AnsiStyle::Warning,
        Severity::Info => AnsiStyle::Info,
    };
    paint(color, &label, style)
}

fn paint(color: bool, text: &str, style: AnsiStyle) -> String {
    if !color {
        return text.to_string();
    }

    let code = match style {
        AnsiStyle::Header => "1;36",
        AnsiStyle::Section => "1",
        AnsiStyle::Path => "36",
        AnsiStyle::Location => "2",
        AnsiStyle::Warning => "33",
        AnsiStyle::Info => "34",
    };

    format!("\x1b[{code}m{text}\x1b[0m")
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
        let output =
            render_human_report_colored(&report(vec![finding("src/a.rs", Some(900))]), true);

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
}
