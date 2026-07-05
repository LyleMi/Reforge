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
    let mut context = ReportRenderContext {
        output: &mut output,
        report,
        color,
    };

    context.render_report_header();
    context.render_report_summary();
    render_signal_breakdown(&mut *context.output, &breakdown, color);

    if report.findings.is_empty() {
        context.output.push('\n');
        context.output.push_str("No refactoring signals found.\n");
        return output;
    }

    context.render_findings();
    output
}

struct ReportRenderContext<'a> {
    output: &'a mut String,
    report: &'a ScanReport,
    color: bool,
}

impl ReportRenderContext<'_> {
    fn render_report_header(&mut self) {
        self.output
            .push_str(&paint(self.color, "Reforge scan report", AnsiStyle::Header));
        self.output.push('\n');
        self.output.push_str(&format!(
            "Scanned {} files in {} ms; {} findings; {} similar function groups.\n",
            self.report.summary.scanned_files,
            self.report.summary.duration_ms,
            self.report.summary.finding_count,
            self.report.summary.similar_function_group_count
        ));
    }

    fn render_report_summary(&mut self) {
        self.output.push('\n');
        self.output
            .push_str(&paint(self.color, "Summary", AnsiStyle::Section));
        self.output.push('\n');
        self.output.push_str(&format!(
            "  Source files: {}\n  Directories: {}\n  Function candidates: {}\n",
            self.report.stats.source_files_scanned,
            self.report.stats.directories_scanned,
            self.report.stats.function_candidates
        ));
    }

    fn render_findings(&mut self) {
        let mut by_path: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
        for finding in sorted_findings(&self.report.findings) {
            by_path.entry(&finding.path).or_default().push(finding);
        }

        self.output.push('\n');
        self.output
            .push_str(&paint(self.color, "Findings", AnsiStyle::Section));
        self.output.push('\n');

        for (path, findings) in by_path {
            self.output.push('\n');
            self.output
                .push_str(&paint(self.color, path, AnsiStyle::Path));
            self.output.push('\n');

            for finding in findings
                .iter()
                .filter(|finding| finding.kind != FindingKind::DebtMarker)
            {
                self.output.push_str("  ");
                self.output
                    .push_str(&render_finding_line(finding, self.color));
                self.output.push('\n');

                if has_related_location_details(finding) {
                    self.output
                        .push_str(&render_related_locations(finding, self.color));
                }
            }

            let debt_markers = findings
                .iter()
                .copied()
                .filter(|finding| finding.kind == FindingKind::DebtMarker)
                .collect::<Vec<_>>();
            if !debt_markers.is_empty() {
                self.output.push_str("  ");
                self.output
                    .push_str(&render_debt_marker_group(&debt_markers, self.color));
                self.output.push('\n');
            }
        }
    }
}

fn render_signal_breakdown(output: &mut String, breakdown: &FindingBreakdown, color: bool) {
    output.push('\n');
    output.push_str(&paint(color, "Signals", AnsiStyle::Section));
    output.push('\n');
    output.push_str(&format!(
        "  Warnings: {}\n  Info: {}\n  Large files: {}\n  Large directories: {}\n  Debt markers: {}\n  Similar function groups: {}\n  Long functions: {}\n  Complex functions: {}\n  Deep nesting: {}\n  Many parameters: {}\n  Large types: {}\n  Large public surfaces: {}\n  Import-heavy files: {}\n  Repeated literals: {}\n  Repeated error patterns: {}\n  Test duplication: {}\n  Directory drift: {}\n  Data clumps: {}\n  Parallel implementations: {}\n  Shadowed abstractions: {}\n  Duplicate type shapes: {}\n  Config key drift: {}\n  Fixture factory drift: {}\n  Generic bucket drift: {}\n  Adapter boundary bypasses: {}\n",
        breakdown.warnings,
        breakdown.info,
        breakdown.count(FindingKind::LargeFile),
        breakdown.count(FindingKind::LargeDirectory),
        breakdown.count(FindingKind::DebtMarker),
        breakdown.count(FindingKind::SimilarFunctions),
        breakdown.count(FindingKind::LongFunction),
        breakdown.count(FindingKind::ComplexFunction),
        breakdown.count(FindingKind::DeepNesting),
        breakdown.count(FindingKind::ManyParameters),
        breakdown.count(FindingKind::LargeType),
        breakdown.count(FindingKind::LargePublicSurface),
        breakdown.count(FindingKind::ImportHeavyFile),
        breakdown.count(FindingKind::RepeatedLiteral),
        breakdown.count(FindingKind::RepeatedErrorPattern),
        breakdown.count(FindingKind::TestDuplication),
        breakdown.count(FindingKind::DirectoryDrift),
        breakdown.count(FindingKind::DataClump),
        breakdown.count(FindingKind::ParallelImplementation),
        breakdown.count(FindingKind::ShadowedAbstraction),
        breakdown.count(FindingKind::DuplicateTypeShape),
        breakdown.count(FindingKind::ConfigKeyDrift),
        breakdown.count(FindingKind::FixtureFactoryDrift),
        breakdown.count(FindingKind::GenericBucketDrift),
        breakdown.count(FindingKind::AdapterBoundaryBypass)
    ));
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
    let display = display_for_kind(finding.kind);
    if let Some(magnitude) = finding.magnitude
        && let Some(phrase) = display.magnitude
    {
        return render_magnitude(display.label, magnitude, phrase);
    }

    if finding.message.is_empty() || finding.kind == FindingKind::DebtMarker {
        display.label.to_string()
    } else {
        finding.message.clone()
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
            | FindingKind::ParallelImplementation
            | FindingKind::ShadowedAbstraction
            | FindingKind::DuplicateTypeShape
            | FindingKind::ConfigKeyDrift
            | FindingKind::FixtureFactoryDrift
            | FindingKind::GenericBucketDrift
            | FindingKind::AdapterBoundaryBypass
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
    by_kind: BTreeMap<FindingKind, usize>,
}

impl FindingBreakdown {
    fn from_findings(findings: &[Finding]) -> Self {
        let mut breakdown = Self::default();

        for finding in findings {
            match finding.severity {
                Severity::Warning => breakdown.warnings += 1,
                Severity::Info => breakdown.info += 1,
            }

            *breakdown.by_kind.entry(finding.kind).or_insert(0) += 1;
        }

        breakdown
    }

    fn count(&self, kind: FindingKind) -> usize {
        self.by_kind.get(&kind).copied().unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
enum MagnitudePhrase {
    Lines,
    SourceFiles,
    Functions,
    Complexity,
    Levels,
    Parameters,
    Size,
    Items,
    Imports,
    Occurrences,
    Concepts,
    Implementations,
    TypeShapes,
    ConfigKeys,
    Factories,
    Bypasses,
}

const MAGNITUDE_PHRASE_COUNT: usize = MagnitudePhrase::Bypasses as usize + 1;

impl MagnitudePhrase {
    fn format(self) -> MagnitudeFormat {
        MAGNITUDE_FORMATS[self as usize]
    }
}

#[derive(Debug, Clone, Copy)]
enum MagnitudeFormat {
    Count(&'static str),
    PluralCount(&'static str),
    PrefixedPluralCount {
        prefix: &'static str,
        noun: &'static str,
    },
    NamedValue(&'static str),
}

impl MagnitudeFormat {
    fn render(self, label: &str, magnitude: usize) -> String {
        match self {
            Self::Count(unit) => render_count_magnitude(label, magnitude, unit),
            Self::PluralCount(noun) => {
                render_count_magnitude(label, magnitude, &pluralize(magnitude, noun))
            }
            Self::PrefixedPluralCount { prefix, noun } => render_count_magnitude(
                label,
                magnitude,
                &format!("{prefix}{}", pluralize(magnitude, noun)),
            ),
            Self::NamedValue(name) => format!("{label}: {name} {magnitude}"),
        }
    }
}

const MAGNITUDE_FORMATS: [MagnitudeFormat; MAGNITUDE_PHRASE_COUNT] = [
    MagnitudeFormat::Count("lines"),
    MagnitudeFormat::Count("source files"),
    MagnitudeFormat::PluralCount("function"),
    MagnitudeFormat::NamedValue("complexity"),
    MagnitudeFormat::Count("levels"),
    MagnitudeFormat::Count("parameters"),
    MagnitudeFormat::NamedValue("size"),
    MagnitudeFormat::Count("items"),
    MagnitudeFormat::Count("imports"),
    MagnitudeFormat::Count("occurrences"),
    MagnitudeFormat::Count("concepts"),
    MagnitudeFormat::PluralCount("implementation"),
    MagnitudeFormat::PluralCount("type shape"),
    MagnitudeFormat::PrefixedPluralCount {
        prefix: "config ",
        noun: "key",
    },
    MagnitudeFormat::PluralCount("factory"),
    MagnitudeFormat::PluralCount("bypass"),
];

#[derive(Debug, Clone, Copy)]
struct FindingKindDisplay {
    kind: FindingKind,
    label: &'static str,
    magnitude: Option<MagnitudePhrase>,
}

const FINDING_KIND_DISPLAYS: &[FindingKindDisplay] = &[
    FindingKindDisplay {
        kind: FindingKind::LargeFile,
        label: "large file",
        magnitude: Some(MagnitudePhrase::Lines),
    },
    FindingKindDisplay {
        kind: FindingKind::LargeDirectory,
        label: "large directory",
        magnitude: Some(MagnitudePhrase::SourceFiles),
    },
    FindingKindDisplay {
        kind: FindingKind::DebtMarker,
        label: "debt marker",
        magnitude: None,
    },
    FindingKindDisplay {
        kind: FindingKind::SimilarFunctions,
        label: "similar functions",
        magnitude: Some(MagnitudePhrase::Functions),
    },
    FindingKindDisplay {
        kind: FindingKind::LongFunction,
        label: "long function",
        magnitude: Some(MagnitudePhrase::Lines),
    },
    FindingKindDisplay {
        kind: FindingKind::ComplexFunction,
        label: "complex function",
        magnitude: Some(MagnitudePhrase::Complexity),
    },
    FindingKindDisplay {
        kind: FindingKind::DeepNesting,
        label: "deep nesting",
        magnitude: Some(MagnitudePhrase::Levels),
    },
    FindingKindDisplay {
        kind: FindingKind::ManyParameters,
        label: "many parameters",
        magnitude: Some(MagnitudePhrase::Parameters),
    },
    FindingKindDisplay {
        kind: FindingKind::LargeType,
        label: "large type",
        magnitude: Some(MagnitudePhrase::Size),
    },
    FindingKindDisplay {
        kind: FindingKind::LargePublicSurface,
        label: "large public surface",
        magnitude: Some(MagnitudePhrase::Items),
    },
    FindingKindDisplay {
        kind: FindingKind::ImportHeavyFile,
        label: "import-heavy file",
        magnitude: Some(MagnitudePhrase::Imports),
    },
    FindingKindDisplay {
        kind: FindingKind::RepeatedLiteral,
        label: "repeated literal",
        magnitude: Some(MagnitudePhrase::Occurrences),
    },
    FindingKindDisplay {
        kind: FindingKind::RepeatedErrorPattern,
        label: "repeated error pattern",
        magnitude: Some(MagnitudePhrase::Occurrences),
    },
    FindingKindDisplay {
        kind: FindingKind::TestDuplication,
        label: "test duplication",
        magnitude: Some(MagnitudePhrase::Occurrences),
    },
    FindingKindDisplay {
        kind: FindingKind::DirectoryDrift,
        label: "directory drift",
        magnitude: Some(MagnitudePhrase::Concepts),
    },
    FindingKindDisplay {
        kind: FindingKind::DataClump,
        label: "data clump",
        magnitude: Some(MagnitudePhrase::Occurrences),
    },
    FindingKindDisplay {
        kind: FindingKind::ParallelImplementation,
        label: "parallel implementation",
        magnitude: Some(MagnitudePhrase::Implementations),
    },
    FindingKindDisplay {
        kind: FindingKind::ShadowedAbstraction,
        label: "shadowed abstraction",
        magnitude: Some(MagnitudePhrase::Occurrences),
    },
    FindingKindDisplay {
        kind: FindingKind::DuplicateTypeShape,
        label: "duplicate type shape",
        magnitude: Some(MagnitudePhrase::TypeShapes),
    },
    FindingKindDisplay {
        kind: FindingKind::ConfigKeyDrift,
        label: "config key drift",
        magnitude: Some(MagnitudePhrase::ConfigKeys),
    },
    FindingKindDisplay {
        kind: FindingKind::FixtureFactoryDrift,
        label: "fixture factory drift",
        magnitude: Some(MagnitudePhrase::Factories),
    },
    FindingKindDisplay {
        kind: FindingKind::GenericBucketDrift,
        label: "generic bucket drift",
        magnitude: Some(MagnitudePhrase::Concepts),
    },
    FindingKindDisplay {
        kind: FindingKind::AdapterBoundaryBypass,
        label: "adapter boundary bypass",
        magnitude: Some(MagnitudePhrase::Bypasses),
    },
];

fn display_for_kind(kind: FindingKind) -> &'static FindingKindDisplay {
    FINDING_KIND_DISPLAYS
        .iter()
        .find(|display| display.kind == kind)
        .expect("every finding kind should have display metadata")
}

fn render_magnitude(label: &str, magnitude: usize, phrase: MagnitudePhrase) -> String {
    phrase.format().render(label, magnitude)
}

fn render_count_magnitude(label: &str, magnitude: usize, unit: &str) -> String {
    format!("{label}: {magnitude} {unit}")
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
#[path = "report_tests.rs"]
mod tests;
