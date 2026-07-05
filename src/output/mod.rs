use std::collections::BTreeMap;
use std::io::{self, Write};

use anyhow::Result;

use crate::model::{Finding, FindingKind, ScanReport, Severity};

const RELATED_LOCATION_LIMIT: usize = 3;

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
            "Scanned {} files in {} ms; {} findings; {} hotspots; {} similar function groups.\n",
            self.report.summary.scanned_files,
            self.report.summary.duration_ms,
            self.report.summary.finding_count,
            self.report.summary.hotspot_count,
            self.report.summary.similar_function_group_count
        ));
    }

    fn render_report_summary(&mut self) {
        self.output.push('\n');
        self.output
            .push_str(&paint(self.color, "Summary", AnsiStyle::Section));
        self.output.push('\n');
        self.output.push_str(&format!(
            "  Source files: {}\n  Directories: {}\n  Function candidates: {}\n  Hotspot model: {}\n  Churn: {}{}\n",
            self.report.stats.source_files_scanned,
            self.report.stats.directories_scanned,
            self.report.stats.function_candidates,
            hotspot_model_label(self.report.summary.hotspot_model),
            self.report.summary.churn.status,
            self.report
                .summary
                .churn
                .reason
                .as_ref()
                .map(|reason| format!(" ({reason})"))
                .unwrap_or_default()
        ));
    }

    fn render_findings(&mut self) {
        self.render_hotspots();
        self.output.push('\n');
        self.output
            .push_str(&paint(self.color, "Findings", AnsiStyle::Section));
        self.output.push('\n');

        for finding in sorted_findings(&self.report.findings) {
            self.output.push_str("  ");
            self.output
                .push_str(&render_finding_line(finding, self.color));
            self.output.push('\n');

            if has_related_location_details(finding) {
                self.output
                    .push_str(&render_related_locations(finding, self.color));
            }
        }
    }

    fn render_hotspots(&mut self) {
        if self.report.hotspots.is_empty() {
            return;
        }

        self.output.push('\n');
        self.output
            .push_str(&paint(self.color, "Hotspots", AnsiStyle::Section));
        self.output.push('\n');
        for hotspot in self.report.hotspots.iter().take(10) {
            self.output.push_str(&format!(
                "  [{} priority={}] {}{} - {}\n",
                hotspot.severity,
                hotspot.priority,
                hotspot.path,
                hotspot
                    .line
                    .map(|line| format!(":{line}"))
                    .unwrap_or_default(),
                hotspot.reason
            ));
        }
    }
}

fn hotspot_model_label(model: crate::cli::HotspotModel) -> &'static str {
    match model {
        crate::cli::HotspotModel::Static => "static",
        crate::cli::HotspotModel::Churn => "churn",
        crate::cli::HotspotModel::Hybrid => "hybrid",
    }
}

fn render_signal_breakdown(output: &mut String, breakdown: &FindingBreakdown, color: bool) {
    output.push('\n');
    output.push_str(&paint(color, "Signals", AnsiStyle::Section));
    output.push('\n');
    output.push_str(&format!(
        "  Critical: {}\n  Warnings: {}\n  Info: {}\n  Large files: {}\n  Large directories: {}\n  Debt markers: {}\n  Similar function groups: {}\n  Long functions: {}\n  Complex functions: {}\n  Deep nesting: {}\n  Many parameters: {}\n  Large types: {}\n  Large public surfaces: {}\n  Import-heavy files: {}\n  Repeated literals: {}\n  Repeated error patterns: {}\n  Test duplication: {}\n  Happy-path-only tests: {}\n  File naming drift: {}\n  Directory drift: {}\n  Data clumps: {}\n  Parallel implementations: {}\n  Shadowed abstractions: {}\n  Duplicate type shapes: {}\n  Config key drift: {}\n  Fixture factory drift: {}\n  Generic bucket drift: {}\n  Adapter boundary bypasses: {}\n  Missing documentation sets: {}\n  Missing user guides: {}\n  Missing report schema docs: {}\n  Missing metrics model docs: {}\n  Missing architecture docs: {}\n  Stale CLI docs: {}\n  Stale schema docs: {}\n",
        breakdown.critical,
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
        breakdown.count(FindingKind::HappyPathOnlyTests),
        breakdown.count(FindingKind::FileNamingDrift),
        breakdown.count(FindingKind::DirectoryDrift),
        breakdown.count(FindingKind::DataClump),
        breakdown.count(FindingKind::ParallelImplementation),
        breakdown.count(FindingKind::ShadowedAbstraction),
        breakdown.count(FindingKind::DuplicateTypeShape),
        breakdown.count(FindingKind::ConfigKeyDrift),
        breakdown.count(FindingKind::FixtureFactoryDrift),
        breakdown.count(FindingKind::GenericBucketDrift),
        breakdown.count(FindingKind::AdapterBoundaryBypass),
        breakdown.count(FindingKind::MissingDocumentationSet),
        breakdown.count(FindingKind::MissingUserGuide),
        breakdown.count(FindingKind::MissingReportSchemaDocs),
        breakdown.count(FindingKind::MissingMetricsModelDocs),
        breakdown.count(FindingKind::MissingArchitectureDocs),
        breakdown.count(FindingKind::StaleCliDocumentation),
        breakdown.count(FindingKind::StaleSchemaDocumentation)
    ));
}

fn sorted_findings(findings: &[Finding]) -> Vec<&Finding> {
    let mut sorted = findings.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.line.cmp(&right.line))
    });
    sorted
}

fn render_finding_line(finding: &Finding, color: bool) -> String {
    let location = finding
        .line
        .map(|line| format!("{}:{line}", finding.path))
        .unwrap_or_else(|| finding.path.clone());
    let metrics = render_metric_summary(finding);
    format!(
        "{} {location} {}{}{}",
        render_severity(finding, color),
        concise_finding_message(finding),
        metrics
            .map(|metrics| format!(" ({metrics})"))
            .unwrap_or_default(),
        render_rank_explanation(finding)
    )
}

fn render_rank_explanation(finding: &Finding) -> String {
    if finding.rank_explanation.is_empty() {
        String::new()
    } else {
        format!(" - {}", finding.rank_explanation)
    }
}

fn render_metric_summary(finding: &Finding) -> Option<String> {
    if finding.metrics.is_empty() {
        return None;
    }

    Some(
        finding
            .metrics
            .iter()
            .map(|metric| {
                let value = if let Some(threshold) = metric.threshold {
                    format!("{}={}/{}", metric.name, metric.value, threshold)
                } else {
                    format!("{}={}", metric.name, metric.value)
                };

                if metric.unit.is_empty() {
                    value
                } else {
                    format!("{value} {}", metric.unit)
                }
            })
            .collect::<Vec<_>>()
            .join(", "),
    )
}

fn concise_finding_message(finding: &Finding) -> String {
    if !finding.message.is_empty() && finding.kind != FindingKind::DebtMarker {
        return finding.message.clone();
    }

    let display = display_for_kind(finding.kind);
    render_kind_metric_message(finding, display).unwrap_or_else(|| display.label.to_string())
}

fn render_kind_metric_message(finding: &Finding, display: &FindingKindDisplay) -> Option<String> {
    let value = match display.metric {
        Some(DisplayMetric::Primary) => primary_metric_value(finding),
        Some(DisplayMetric::GroupSize) => group_size(finding),
        Some(DisplayMetric::Named(name)) => metric_value(finding, name),
        None => None,
    }?;
    Some(display.format.render(display.label, value))
}

fn metric_value(finding: &Finding, name: &str) -> Option<usize> {
    finding
        .metrics
        .iter()
        .find(|metric| metric.name == name)
        .map(|metric| metric.value)
}

fn primary_metric_value(finding: &Finding) -> Option<usize> {
    finding.metrics.first().map(|metric| metric.value)
}

fn group_size(finding: &Finding) -> Option<usize> {
    metric_value(finding, "group_size").or_else(|| primary_metric_value(finding))
}

fn has_related_location_details(finding: &Finding) -> bool {
    matches!(
        finding.kind,
        FindingKind::SimilarFunctions
            | FindingKind::RepeatedLiteral
            | FindingKind::RepeatedErrorPattern
            | FindingKind::TestDuplication
            | FindingKind::HappyPathOnlyTests
            | FindingKind::FileNamingDrift
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
    critical: usize,
    warnings: usize,
    info: usize,
    by_kind: BTreeMap<FindingKind, usize>,
}

impl FindingBreakdown {
    fn from_findings(findings: &[Finding]) -> Self {
        let mut breakdown = Self::default();

        for finding in findings {
            match finding.severity {
                Severity::Critical => breakdown.critical += 1,
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
enum DisplayMetric {
    Primary,
    GroupSize,
    Named(&'static str),
}

#[derive(Debug, Clone, Copy)]
enum MetricFormat {
    Count(&'static str),
    PluralCount(&'static str),
    PrefixedPluralCount {
        prefix: &'static str,
        noun: &'static str,
    },
    NamedValue(&'static str),
}

impl MetricFormat {
    fn render(self, label: &str, value: usize) -> String {
        match self {
            Self::Count(unit) => format!("{label}: {value} {unit}"),
            Self::PluralCount(noun) => {
                format!("{label}: {value} {}", pluralize(value, noun))
            }
            Self::PrefixedPluralCount { prefix, noun } => {
                format!("{label}: {value} {prefix}{}", pluralize(value, noun))
            }
            Self::NamedValue(name) => format!("{label}: {name} {value}"),
        }
    }
}

fn pluralize(count: usize, noun: &str) -> String {
    if count == 1 {
        noun.to_string()
    } else {
        format!("{noun}s")
    }
}

#[derive(Debug, Clone, Copy)]
struct FindingKindDisplay {
    kind: FindingKind,
    label: &'static str,
    metric: Option<DisplayMetric>,
    format: MetricFormat,
}

const FINDING_KIND_DISPLAYS: &[FindingKindDisplay] = &[
    display(
        FindingKind::LargeFile,
        "large file",
        DisplayMetric::Primary,
        MetricFormat::Count("lines"),
    ),
    display(
        FindingKind::LargeDirectory,
        "large directory",
        DisplayMetric::Primary,
        MetricFormat::Count("source files"),
    ),
    FindingKindDisplay {
        kind: FindingKind::DebtMarker,
        label: "debt marker",
        metric: None,
        format: MetricFormat::Count(""),
    },
    display(
        FindingKind::SimilarFunctions,
        "similar functions",
        DisplayMetric::GroupSize,
        MetricFormat::PluralCount("function"),
    ),
    display(
        FindingKind::LongFunction,
        "long function",
        DisplayMetric::Primary,
        MetricFormat::Count("lines"),
    ),
    display(
        FindingKind::ComplexFunction,
        "complex function",
        DisplayMetric::Primary,
        MetricFormat::NamedValue("complexity"),
    ),
    display(
        FindingKind::DeepNesting,
        "deep nesting",
        DisplayMetric::Primary,
        MetricFormat::Count("levels"),
    ),
    display(
        FindingKind::ManyParameters,
        "many parameters",
        DisplayMetric::Primary,
        MetricFormat::Count("parameters"),
    ),
    display(
        FindingKind::LargeType,
        "large type",
        DisplayMetric::Named("type_lines"),
        MetricFormat::NamedValue("lines"),
    ),
    display(
        FindingKind::LargePublicSurface,
        "large public surface",
        DisplayMetric::Primary,
        MetricFormat::Count("items"),
    ),
    display(
        FindingKind::ImportHeavyFile,
        "import-heavy file",
        DisplayMetric::Primary,
        MetricFormat::Count("imports"),
    ),
    display(
        FindingKind::RepeatedLiteral,
        "repeated literal",
        DisplayMetric::GroupSize,
        MetricFormat::Count("occurrences"),
    ),
    display(
        FindingKind::RepeatedErrorPattern,
        "repeated error pattern",
        DisplayMetric::GroupSize,
        MetricFormat::Count("occurrences"),
    ),
    display(
        FindingKind::TestDuplication,
        "test duplication",
        DisplayMetric::GroupSize,
        MetricFormat::Count("occurrences"),
    ),
    display(
        FindingKind::HappyPathOnlyTests,
        "happy-path-only tests",
        DisplayMetric::GroupSize,
        MetricFormat::PrefixedPluralCount {
            prefix: "test ",
            noun: "case",
        },
    ),
    display(
        FindingKind::FileNamingDrift,
        "file naming drift",
        DisplayMetric::GroupSize,
        MetricFormat::PrefixedPluralCount {
            prefix: "naming ",
            noun: "style",
        },
    ),
    display(
        FindingKind::DirectoryDrift,
        "directory drift",
        DisplayMetric::GroupSize,
        MetricFormat::Count("concepts"),
    ),
    display(
        FindingKind::DataClump,
        "data clump",
        DisplayMetric::GroupSize,
        MetricFormat::Count("occurrences"),
    ),
    display(
        FindingKind::ParallelImplementation,
        "parallel implementation",
        DisplayMetric::GroupSize,
        MetricFormat::PluralCount("implementation"),
    ),
    display(
        FindingKind::ShadowedAbstraction,
        "shadowed abstraction",
        DisplayMetric::GroupSize,
        MetricFormat::Count("occurrences"),
    ),
    display(
        FindingKind::DuplicateTypeShape,
        "duplicate type shape",
        DisplayMetric::GroupSize,
        MetricFormat::PluralCount("type shape"),
    ),
    display(
        FindingKind::ConfigKeyDrift,
        "config key drift",
        DisplayMetric::GroupSize,
        MetricFormat::PrefixedPluralCount {
            prefix: "config ",
            noun: "key",
        },
    ),
    display(
        FindingKind::FixtureFactoryDrift,
        "fixture factory drift",
        DisplayMetric::GroupSize,
        MetricFormat::PluralCount("factory"),
    ),
    display(
        FindingKind::GenericBucketDrift,
        "generic bucket drift",
        DisplayMetric::GroupSize,
        MetricFormat::Count("concepts"),
    ),
    display(
        FindingKind::AdapterBoundaryBypass,
        "adapter boundary bypass",
        DisplayMetric::GroupSize,
        MetricFormat::PluralCount("bypass"),
    ),
    display(
        FindingKind::MissingDocumentationSet,
        "missing documentation set",
        DisplayMetric::Primary,
        MetricFormat::PluralCount("missing required doc"),
    ),
    display(
        FindingKind::MissingUserGuide,
        "missing user guide",
        DisplayMetric::Primary,
        MetricFormat::PluralCount("missing user topic"),
    ),
    display(
        FindingKind::MissingReportSchemaDocs,
        "missing report schema docs",
        DisplayMetric::Primary,
        MetricFormat::Count("risk"),
    ),
    display(
        FindingKind::MissingMetricsModelDocs,
        "missing metrics model docs",
        DisplayMetric::Primary,
        MetricFormat::Count("risk"),
    ),
    display(
        FindingKind::MissingArchitectureDocs,
        "missing architecture docs",
        DisplayMetric::Primary,
        MetricFormat::Count("risk"),
    ),
    display(
        FindingKind::StaleCliDocumentation,
        "stale CLI documentation",
        DisplayMetric::Primary,
        MetricFormat::PluralCount("missing flag"),
    ),
    display(
        FindingKind::StaleSchemaDocumentation,
        "stale schema documentation",
        DisplayMetric::Primary,
        MetricFormat::PluralCount("missing field"),
    ),
];

const fn display(
    kind: FindingKind,
    label: &'static str,
    metric: DisplayMetric,
    format: MetricFormat,
) -> FindingKindDisplay {
    FindingKindDisplay {
        kind,
        label,
        metric: Some(metric),
        format,
    }
}

fn display_for_kind(kind: FindingKind) -> &'static FindingKindDisplay {
    FINDING_KIND_DISPLAYS
        .iter()
        .find(|display| display.kind == kind)
        .expect("every finding kind should have display metadata")
}

enum AnsiStyle {
    Header,
    Section,
    Path,
    Location,
    Warning,
    Critical,
    Info,
}

fn render_severity(finding: &Finding, color: bool) -> String {
    let label = format!(
        "[{} priority={} confidence={:.2}]",
        finding.severity, finding.priority, finding.confidence
    );
    let style = match finding.severity {
        Severity::Critical => AnsiStyle::Critical,
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
        AnsiStyle::Critical => "31",
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
            Severity::Critical => write!(f, "critical"),
        }
    }
}

#[cfg(test)]
#[path = "../report_tests.rs"]
mod tests;
