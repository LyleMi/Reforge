use std::collections::BTreeMap;
use std::io::{self, Write};

use crate::baseline::{BaselineDiff, BaselineFinding, BaselineFindingStatus};
use crate::cli::BaselineShow;
use crate::model::{Finding, FindingKind, Hotspot, ScanReport, Severity};

const RELATED_LOCATION_LIMIT: usize = 3;
const HOTSPOT_LIMIT: usize = 10;

pub fn print_human_report(report: &ScanReport) -> io::Result<()> {
    write_human_report_colored(std::io::stdout().lock(), report, false)
}

pub fn print_human_report_colored(report: &ScanReport, color: bool) -> io::Result<()> {
    write_human_report_colored(std::io::stdout().lock(), report, color)
}

pub(crate) fn print_human_report_with_baseline(
    report: &ScanReport,
    diff: &BaselineDiff<'_>,
) -> io::Result<()> {
    write_human_report_with_baseline_colored(std::io::stdout().lock(), report, diff, false)
}

pub(crate) fn print_human_report_with_baseline_colored(
    report: &ScanReport,
    diff: &BaselineDiff<'_>,
    color: bool,
) -> io::Result<()> {
    write_human_report_with_baseline_colored(std::io::stdout().lock(), report, diff, color)
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

pub(crate) fn write_human_report_with_baseline(
    writer: impl Write,
    report: &ScanReport,
    diff: &BaselineDiff<'_>,
) -> io::Result<()> {
    write_human_report_with_baseline_colored(writer, report, diff, false)
}

pub(crate) fn write_human_report_with_baseline_colored(
    mut writer: impl Write,
    report: &ScanReport,
    diff: &BaselineDiff<'_>,
    color: bool,
) -> io::Result<()> {
    writer.write_all(render_human_report_with_baseline_colored(report, diff, color).as_bytes())
}

pub fn render_human_report(report: &ScanReport) -> String {
    render_human_report_colored(report, false)
}

pub fn render_human_report_colored(report: &ScanReport, color: bool) -> String {
    render_human_report_view(report, None, color)
}

#[cfg(test)]
pub(crate) fn render_human_report_with_baseline(
    report: &ScanReport,
    diff: &BaselineDiff<'_>,
) -> String {
    render_human_report_with_baseline_colored(report, diff, false)
}

pub(crate) fn render_human_report_with_baseline_colored(
    report: &ScanReport,
    diff: &BaselineDiff<'_>,
    color: bool,
) -> String {
    render_human_report_view(report, Some(diff), color)
}

fn render_human_report_view<'report>(
    report: &'report ScanReport,
    baseline_diff: Option<&BaselineDiff<'report>>,
    color: bool,
) -> String {
    let mut output = String::new();
    let breakdown = FindingBreakdown::from_findings(&report.findings);
    let mut context = ReportRenderContext {
        output: &mut output,
        report,
        baseline_diff,
        color,
    };

    context.render_header();
    context.render_result(&breakdown);
    context.render_baseline_diff();
    context.render_scan_details();

    if report.findings.is_empty() {
        context.output.push('\n');
        context.output.push_str(
            "No threshold signals found. Watchlist entries are review targets, not findings.\n",
        );
        context.render_watchlist();
        return output;
    }

    context.render_signal_mix(&breakdown);
    context.render_findings();
    context.render_watchlist();
    output
}

struct ReportRenderContext<'output, 'report, 'diff> {
    output: &'output mut String,
    report: &'report ScanReport,
    baseline_diff: Option<&'diff BaselineDiff<'report>>,
    color: bool,
}

impl ReportRenderContext<'_, '_, '_> {
    fn render_header(&mut self) {
        self.output
            .push_str(&paint(self.color, "Reforge scan", AnsiStyle::Header));
        self.output.push('\n');
        self.output.push_str(&format!(
            "{} files  {}  model {}  churn {}{}\n",
            self.report.summary.scanned_files,
            format_duration(self.report.summary.duration_ms),
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

    fn render_result(&mut self, breakdown: &FindingBreakdown) {
        self.output.push('\n');
        self.output
            .push_str(&paint(self.color, "Result", AnsiStyle::Section));
        self.output.push('\n');
        self.render_summary_row(
            "Signals",
            format!(
                "{}  critical {} | warning {} | info {}",
                self.report.summary.finding_count,
                breakdown.critical,
                breakdown.warnings,
                breakdown.info
            ),
        );
        self.render_summary_row(
            "Watchlist",
            format!("{} hotspots", self.report.summary.hotspot_count),
        );
        self.render_summary_row(
            "Similar groups",
            self.report.summary.similar_function_group_count,
        );
    }

    fn render_scan_details(&mut self) {
        self.output.push('\n');
        self.output
            .push_str(&paint(self.color, "Scan details", AnsiStyle::Section));
        self.output.push('\n');
        self.render_summary_row("Source files", self.report.stats.source_files_scanned);
        self.render_summary_row("Directories", self.report.stats.directories_scanned);
        self.render_summary_row("Function candidates", self.report.stats.function_candidates);
    }

    fn render_baseline_diff(&mut self) {
        let Some(diff) = self.baseline_diff else {
            return;
        };

        self.output.push('\n');
        self.output
            .push_str(&paint(self.color, "Baseline diff", AnsiStyle::Section));
        self.output.push('\n');
        self.render_summary_row("New", diff.summary.new);
        self.render_summary_row("Worse", diff.summary.worse);
        self.render_summary_row("Same", diff.summary.same);
        self.render_summary_row("Resolved", diff.summary.resolved);
        self.render_summary_row(
            "Showing",
            format!(
                "{} ({} of {} current)",
                baseline_show_label(diff.show),
                diff.findings.len(),
                self.report.findings.len()
            ),
        );
    }

    fn render_findings(&mut self) {
        self.output.push('\n');
        let title = self
            .baseline_diff
            .map(|diff| format!("Findings ({})", baseline_show_label(diff.show)))
            .unwrap_or_else(|| "Findings".to_string());
        self.output
            .push_str(&paint(self.color, &title, AnsiStyle::Section));
        self.output.push('\n');

        if let Some(diff) = self.baseline_diff {
            let findings = sorted_baseline_findings(&diff.findings);
            if findings.is_empty() {
                self.output.push_str(&format!(
                    "  No findings matched --show {}.\n",
                    baseline_show_value(diff.show)
                ));
                return;
            }

            for entry in findings {
                self.output
                    .push_str(&render_diff_finding(entry, self.color));

                if has_related_location_details(entry.finding) {
                    self.output
                        .push_str(&render_related_locations(entry.finding, self.color));
                }
            }
            return;
        }

        for finding in sorted_findings(&self.report.findings) {
            self.output.push_str(&render_finding(finding, self.color));

            if has_related_location_details(finding) {
                self.output
                    .push_str(&render_related_locations(finding, self.color));
            }
        }
    }

    fn render_signal_mix(&mut self, breakdown: &FindingBreakdown) {
        self.output.push('\n');
        self.output
            .push_str(&paint(self.color, "Signal mix", AnsiStyle::Section));
        self.output.push('\n');

        for display in FINDING_KIND_DISPLAYS {
            let count = breakdown.count(display.kind);
            if count == 0 {
                continue;
            }

            self.render_summary_row(display.label, count);
        }
    }

    fn render_watchlist(&mut self) {
        if self.report.hotspots.is_empty() {
            return;
        }

        self.output.push('\n');
        self.output
            .push_str(&paint(self.color, "Watchlist", AnsiStyle::Section));
        self.output.push('\n');
        self.output.push_str(&format!(
            "  {} {}  {}  {}\n",
            paint(self.color, "severity", AnsiStyle::Muted),
            paint(self.color, "pri", AnsiStyle::Muted),
            paint(self.color, "target", AnsiStyle::Muted),
            paint(self.color, "why", AnsiStyle::Muted)
        ));
        for hotspot in self.report.hotspots.iter().take(HOTSPOT_LIMIT) {
            self.output
                .push_str(&render_watchlist_item(hotspot, self.color));
        }
        if self.report.hotspots.len() > HOTSPOT_LIMIT {
            self.output.push_str(&format!(
                "  +{} more hotspots\n",
                self.report.hotspots.len() - HOTSPOT_LIMIT
            ));
        }
    }

    fn render_summary_row(&mut self, label: &str, value: impl std::fmt::Display) {
        self.output.push_str(&format!(
            "  {} {}\n",
            paint(self.color, &format!("{label:<20}"), AnsiStyle::Muted),
            value
        ));
    }
}

fn hotspot_model_label(model: crate::cli::HotspotModel) -> &'static str {
    match model {
        crate::cli::HotspotModel::Static => "static",
        crate::cli::HotspotModel::Churn => "churn",
        crate::cli::HotspotModel::Hybrid => "hybrid",
    }
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

fn sorted_baseline_findings<'a>(
    findings: &'a [BaselineFinding<'a>],
) -> Vec<&'a BaselineFinding<'a>> {
    let mut sorted = findings.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| compare_findings(left.finding, right.finding));
    sorted
}

fn compare_findings(left: &Finding, right: &Finding) -> std::cmp::Ordering {
    right
        .priority
        .cmp(&left.priority)
        .then_with(|| left.path.cmp(&right.path))
        .then_with(|| left.line.cmp(&right.line))
}

fn render_finding(finding: &Finding, color: bool) -> String {
    let location = finding
        .line
        .map(|line| format!("{}:{line}", display_path(&finding.path)))
        .unwrap_or_else(|| display_path(&finding.path));
    let metrics = render_metric_summary(finding);

    let mut output = format!(
        "  {} p={:>2} c={:.2}  {}\n            {}\n",
        render_severity_cell(finding.severity, color),
        finding.priority,
        finding.confidence,
        concise_finding_message(finding),
        paint(color, &location, AnsiStyle::Path),
    );

    if let Some(metrics) = metrics {
        output.push_str(&format!("            metrics {metrics}\n"));
    }
    if !finding.rank_explanation.is_empty() {
        output.push_str(&format!("            rank {}\n", finding.rank_explanation));
    }
    if should_render_recommendation(finding) {
        output.push_str(&format!("            hint {}\n", finding.recommendation()));
    }

    output
}

fn should_render_recommendation(finding: &Finding) -> bool {
    finding.priority >= 35
}

fn render_diff_finding(entry: &BaselineFinding<'_>, color: bool) -> String {
    let finding = entry.finding;
    let location = finding
        .line
        .map(|line| format!("{}:{line}", display_path(&finding.path)))
        .unwrap_or_else(|| display_path(&finding.path));
    let metrics = render_metric_summary(finding);

    let mut output = format!(
        "  {} {} p={:>2} c={:.2}  {}\n            {}\n",
        render_status_cell(entry.status, color),
        render_severity_cell(finding.severity, color),
        finding.priority,
        finding.confidence,
        concise_finding_message(finding),
        paint(color, &location, AnsiStyle::Path),
    );

    if let Some(metrics) = metrics {
        output.push_str(&format!("            metrics {metrics}\n"));
    }
    if !finding.rank_explanation.is_empty() {
        output.push_str(&format!("            rank {}\n", finding.rank_explanation));
    }

    output
}

fn render_watchlist_item(hotspot: &Hotspot, color: bool) -> String {
    let target = render_hotspot_target(hotspot);

    format!(
        "  {} {:>3}  {}  {}\n",
        render_severity_cell(hotspot.severity, color),
        hotspot.priority,
        paint(color, &target, AnsiStyle::Path),
        concise_hotspot_reason(&hotspot.reason)
    )
}

fn render_hotspot_target(hotspot: &Hotspot) -> String {
    let mut target = display_path(&hotspot.path);
    if let Some(line) = hotspot.line {
        target.push_str(&format!(":{line}"));
    }
    if let Some(name) = &hotspot.name {
        target.push(' ');
        target.push_str(name);
    }
    target
}

fn concise_hotspot_reason(reason: &str) -> &str {
    reason
        .strip_prefix("hybrid model: ")
        .or_else(|| reason.strip_prefix("static model: "))
        .or_else(|| reason.strip_prefix("churn model: "))
        .unwrap_or(reason)
}

fn display_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let Ok(current_dir) = std::env::current_dir() else {
        return normalized;
    };
    let current_dir = current_dir.to_string_lossy().replace('\\', "/");
    let current_dir = current_dir.trim_end_matches('/');

    if normalized == current_dir {
        return ".".to_string();
    }

    normalized
        .strip_prefix(&format!("{current_dir}/"))
        .unwrap_or(&normalized)
        .to_string()
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
            | FindingKind::StaleCompatibilityPath
            | FindingKind::DependencyCycle
    )
}

fn render_related_locations(finding: &Finding, color: bool) -> String {
    let mut output = String::new();
    output.push_str("            related\n");

    for location in finding
        .related_locations
        .iter()
        .take(RELATED_LOCATION_LIMIT)
    {
        output.push_str("              ");
        output.push_str(&paint(
            color,
            &display_path(&location.path),
            AnsiStyle::Path,
        ));
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
            "              +{} more\n",
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
        FindingKind::FunctionProliferation,
        "function proliferation",
        DisplayMetric::Named("function_count"),
        MetricFormat::PluralCount("function"),
    ),
    display(
        FindingKind::UnusedFunction,
        "unused function",
        DisplayMetric::Named("references"),
        MetricFormat::Count("references"),
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
        FindingKind::StaleCompatibilityPath,
        "stale compatibility path",
        DisplayMetric::GroupSize,
        MetricFormat::PluralCount("marker"),
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
    display(
        FindingKind::DependencyCycle,
        "dependency cycle",
        DisplayMetric::Named("cycle_files"),
        MetricFormat::PluralCount("file"),
    ),
    display(
        FindingKind::DependencyHub,
        "dependency hub",
        DisplayMetric::Named("fan_out"),
        MetricFormat::Count("outgoing dependencies"),
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
    Muted,
    Warning,
    Critical,
    Info,
}

fn render_status_cell(status: BaselineFindingStatus, color: bool) -> String {
    paint(
        color,
        &format!("{:<8}", baseline_status_label(status)),
        match status {
            BaselineFindingStatus::New => AnsiStyle::Info,
            BaselineFindingStatus::Worse => AnsiStyle::Warning,
            BaselineFindingStatus::Same => AnsiStyle::Muted,
        },
    )
}

fn render_severity_cell(severity: Severity, color: bool) -> String {
    paint(
        color,
        &format!("{:<8}", severity.to_string()),
        match severity {
            Severity::Critical => AnsiStyle::Critical,
            Severity::Warning => AnsiStyle::Warning,
            Severity::Info => AnsiStyle::Info,
        },
    )
}

fn baseline_status_label(status: BaselineFindingStatus) -> &'static str {
    match status {
        BaselineFindingStatus::New => "new",
        BaselineFindingStatus::Worse => "worse",
        BaselineFindingStatus::Same => "same",
    }
}

fn baseline_show_label(show: BaselineShow) -> &'static str {
    match show {
        BaselineShow::New => "new",
        BaselineShow::NewOrWorse => "new or worse",
        BaselineShow::All => "all current",
    }
}

fn baseline_show_value(show: BaselineShow) -> &'static str {
    match show {
        BaselineShow::New => "new",
        BaselineShow::NewOrWorse => "new-or-worse",
        BaselineShow::All => "all",
    }
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
        AnsiStyle::Muted => "2",
        AnsiStyle::Critical => "1;31",
        AnsiStyle::Warning => "1;33",
        AnsiStyle::Info => "1;34",
    };

    format!("\x1b[{code}m{text}\x1b[0m")
}

fn format_duration(duration_ms: u128) -> String {
    if duration_ms < 1_000 {
        format!("{duration_ms} ms")
    } else {
        format!("{:.2} s", duration_ms as f64 / 1_000.0)
    }
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
