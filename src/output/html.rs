use std::collections::BTreeMap;
use std::io::{self, Write};

use crate::model::{
    FileRawMetric, Finding, FindingKind, Hotspot, HotspotLevel, ScanReport, Severity,
};

const FILE_HEATMAP_LIMIT: usize = 24;
const FINDING_LIMIT: usize = 80;
const HOTSPOT_LIMIT: usize = 20;
const SIMILAR_GROUP_LIMIT: usize = 12;
const RELATED_LOCATION_LIMIT: usize = 8;

pub fn print_html_report(report: &ScanReport) -> io::Result<()> {
    write_html_report(std::io::stdout().lock(), report)
}

pub fn write_html_report(mut writer: impl Write, report: &ScanReport) -> io::Result<()> {
    writer.write_all(render_html_report(report).as_bytes())
}

pub fn render_html_report(report: &ScanReport) -> String {
    let mut context = HtmlRenderContext {
        output: String::new(),
        report,
    };

    context.render_document();
    context.output
}

struct HtmlRenderContext<'a> {
    output: String,
    report: &'a ScanReport,
}

impl HtmlRenderContext<'_> {
    fn render_document(&mut self) {
        self.output.push_str("<!doctype html>\n");
        self.output
            .push_str("<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
        self.output
            .push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
        self.output
            .push_str("<title>Reforge scan report</title>\n<style>\n");
        self.output.push_str(STYLES);
        self.output.push_str("</style>\n</head>\n<body>\n");
        self.render_header();
        self.output.push_str("<main>\n");
        self.render_summary();
        self.render_diagnostics();
        self.render_file_heatmap();
        self.render_hotspots();
        self.render_similar_groups();
        self.render_findings();
        self.output.push_str("</main>\n</body>\n</html>\n");
    }

    fn render_header(&mut self) {
        let summary = &self.report.summary;
        self.output.push_str("<header class=\"hero\">\n");
        self.output.push_str("<div>\n");
        self.output
            .push_str("<p class=\"eyebrow\">Reforge visual report</p>\n");
        self.output.push_str("<h1>Codebase refactoring map</h1>\n");
        self.output.push_str("<p class=\"subhead\">");
        self.output.push_str(&escape_html(&format!(
            "{} files scanned in {} with {} hotspot ranking and churn {}{}.",
            summary.scanned_files,
            format_duration(summary.duration_ms),
            hotspot_model_label(summary.hotspot_model),
            summary.churn.status,
            summary
                .churn
                .reason
                .as_ref()
                .map(|reason| format!(" ({reason})"))
                .unwrap_or_default()
        )));
        self.output.push_str("</p>\n</div>\n");
        self.output.push_str("</header>\n");
    }

    fn render_summary(&mut self) {
        let breakdown = SeverityBreakdown::from_findings(&self.report.findings);
        self.output
            .push_str("<section class=\"summary-grid\" aria-label=\"Scan summary\">\n");
        self.render_stat_card(
            "Signals",
            self.report.summary.finding_count,
            "threshold findings",
        );
        self.render_stat_card("Critical", breakdown.critical, "priority 70+");
        self.render_stat_card("Warnings", breakdown.warnings, "priority 35-69");
        self.render_stat_card(
            "Watchlist",
            self.report.summary.hotspot_count,
            "ranked hotspots",
        );
        self.render_stat_card(
            "Similar Groups",
            self.report.summary.similar_function_group_count,
            "duplication clusters",
        );
        self.render_stat_card(
            "Functions",
            self.report.stats.function_candidates,
            "similarity candidates",
        );
        self.output.push_str("</section>\n");
    }

    fn render_stat_card(&mut self, label: &str, value: usize, note: &str) {
        self.output.push_str("<article class=\"stat-card\"><span>");
        self.output.push_str(&escape_html(label));
        self.output.push_str("</span><strong>");
        self.output.push_str(&value.to_string());
        self.output.push_str("</strong><small>");
        self.output.push_str(&escape_html(note));
        self.output.push_str("</small></article>\n");
    }

    fn render_diagnostics(&mut self) {
        let dimensions = dimension_breakdown(&self.report.findings);
        let severities = SeverityBreakdown::from_findings(&self.report.findings);

        self.output.push_str("<section class=\"panel-grid\">\n");
        self.output
            .push_str("<article class=\"panel\"><h2>Risk Distribution</h2>\n");
        self.render_distribution_bar(&[
            ("critical", severities.critical),
            ("warning", severities.warnings),
            ("info", severities.info),
        ]);
        self.output.push_str("</article>\n");

        self.output
            .push_str("<article class=\"panel\"><h2>Problem Dimensions</h2>\n");
        if dimensions.is_empty() {
            self.output
                .push_str("<p class=\"empty\">No finding dimensions recorded.</p>\n");
        } else {
            for (dimension, count) in dimensions {
                self.output.push_str("<div class=\"dimension-row\"><span>");
                self.output.push_str(&escape_html(&title_label(&dimension)));
                self.output.push_str("</span><strong>");
                self.output.push_str(&count.to_string());
                self.output.push_str("</strong></div>\n");
            }
        }
        self.output.push_str("</article>\n");
        self.output.push_str("</section>\n");
    }

    fn render_distribution_bar(&mut self, parts: &[(&str, usize)]) {
        let total = parts.iter().map(|(_, count)| *count).sum::<usize>();
        if total == 0 {
            self.output
                .push_str("<p class=\"empty\">No threshold signals found.</p>\n");
            return;
        }

        self.output
            .push_str("<div class=\"stacked-bar\" aria-hidden=\"true\">");
        for (label, count) in parts {
            if *count == 0 {
                continue;
            }
            let width = (*count as f64 / total as f64 * 100.0).max(3.0);
            self.output.push_str(&format!(
                "<span class=\"segment {label}\" style=\"width:{width:.2}%\"></span>"
            ));
        }
        self.output.push_str("</div>\n<div class=\"legend\">");
        for (label, count) in parts {
            self.output.push_str("<span><i class=\"dot ");
            self.output.push_str(label);
            self.output.push_str("\"></i>");
            self.output
                .push_str(&escape_html(&format!("{} {}", count, label)));
            self.output.push_str("</span>");
        }
        self.output.push_str("</div>\n");
    }

    fn render_file_heatmap(&mut self) {
        self.output
            .push_str("<section class=\"panel\"><div class=\"section-title\"><h2>File Heatmap</h2><span>risk, findings, size</span></div>\n");

        let files = ranked_file_overviews(self.report);
        if files.is_empty() {
            self.output
                .push_str("<p class=\"empty\">No raw file metrics were recorded.</p>\n");
            self.output.push_str("</section>\n");
            return;
        }

        self.output.push_str("<div class=\"file-heatmap\">\n");
        for file in files.iter().take(FILE_HEATMAP_LIMIT) {
            let heat = heat_class(file.risk);
            self.output.push_str("<div class=\"file-row ");
            self.output.push_str(heat);
            self.output
                .push_str("\">\n<div class=\"file-main\"><strong>");
            self.output
                .push_str(&escape_html(&display_path(&file.path)));
            self.output.push_str("</strong><span>");
            self.output.push_str(&escape_html(&format!(
                "{} LOC · {} imports · {} public · {} churn",
                file.loc, file.imports, file.public_items, file.churn
            )));
            self.output
                .push_str("</span></div><div class=\"file-score\"><span>");
            self.output.push_str(&file.risk.to_string());
            self.output.push_str("</span><small>");
            self.output.push_str(&escape_html(&format!(
                "{} findings · hotspot {}",
                file.findings, file.hotspot_priority
            )));
            self.output.push_str("</small></div>\n</div>\n");
        }
        if files.len() > FILE_HEATMAP_LIMIT {
            self.output.push_str("<p class=\"more\">");
            self.output.push_str(&format!(
                "+{} more files omitted from the heatmap",
                files.len() - FILE_HEATMAP_LIMIT
            ));
            self.output.push_str("</p>\n");
        }
        self.output.push_str("</div>\n</section>\n");
    }

    fn render_hotspots(&mut self) {
        self.output
            .push_str("<section class=\"panel\"><div class=\"section-title\"><h2>Watchlist</h2><span>ranked review targets</span></div>\n");

        if self.report.hotspots.is_empty() {
            self.output
                .push_str("<p class=\"empty\">No hotspots met the watchlist threshold.</p>\n");
            self.output.push_str("</section>\n");
            return;
        }

        self.output.push_str("<div class=\"table-like\">\n");
        for hotspot in self.report.hotspots.iter().take(HOTSPOT_LIMIT) {
            self.output
                .push_str("<article class=\"row-card\"><div><span class=\"pill ");
            self.output.push_str(severity_class(hotspot.severity));
            self.output.push_str("\">");
            self.output.push_str(severity_label(hotspot.severity));
            self.output.push_str("</span><strong>");
            self.output
                .push_str(&escape_html(&render_hotspot_target(hotspot)));
            self.output.push_str("</strong><small>");
            self.output.push_str(&escape_html(&format!(
                "{} · static {:.2} · churn {:.2}",
                hotspot_level_label(hotspot.level),
                hotspot.static_risk,
                hotspot.churn_risk
            )));
            self.output
                .push_str("</small></div><div class=\"priority\">");
            self.output.push_str(&hotspot.priority.to_string());
            self.output.push_str("</div><p>");
            self.output
                .push_str(&escape_html(&concise_hotspot_reason(&hotspot.reason)));
            self.output.push_str("</p></article>\n");
        }
        self.output.push_str("</div>\n</section>\n");
    }

    fn render_similar_groups(&mut self) {
        let groups = self
            .report
            .findings
            .iter()
            .filter(|finding| finding.kind == FindingKind::SimilarFunctions)
            .collect::<Vec<_>>();

        self.output
            .push_str("<section class=\"panel\"><div class=\"section-title\"><h2>Similar Function Groups</h2><span>duplication clusters</span></div>\n");

        if groups.is_empty() {
            self.output
                .push_str("<p class=\"empty\">No similar-function groups were reported.</p>\n");
            self.output.push_str("</section>\n");
            return;
        }

        for finding in groups.into_iter().take(SIMILAR_GROUP_LIMIT) {
            self.output
                .push_str("<article class=\"group-card\"><div><strong>");
            self.output
                .push_str(&escape_html(&finding_summary(finding)));
            self.output
                .push_str("</strong><span class=\"priority mini\">");
            self.output.push_str(&finding.priority.to_string());
            self.output.push_str("</span></div><ul>\n");
            for location in finding
                .related_locations
                .iter()
                .take(RELATED_LOCATION_LIMIT)
            {
                self.output.push_str("<li>");
                self.output.push_str(&escape_html(&format!(
                    "{}:{}{}",
                    display_path(&location.path),
                    location.line,
                    location
                        .name
                        .as_ref()
                        .map(|name| format!(" {name}"))
                        .unwrap_or_default()
                )));
                self.output.push_str("</li>\n");
            }
            if finding.related_locations.len() > RELATED_LOCATION_LIMIT {
                self.output.push_str("<li class=\"more\">");
                self.output.push_str(&format!(
                    "+{} more related locations",
                    finding.related_locations.len() - RELATED_LOCATION_LIMIT
                ));
                self.output.push_str("</li>\n");
            }
            self.output.push_str("</ul></article>\n");
        }

        self.output.push_str("</section>\n");
    }

    fn render_findings(&mut self) {
        self.output
            .push_str("<section class=\"panel\"><div class=\"section-title\"><h2>Findings</h2><span>prioritized diagnostics</span></div>\n");

        if self.report.findings.is_empty() {
            self.output
                .push_str("<p class=\"empty\">No threshold signals found.</p>\n");
            self.output.push_str("</section>\n");
            return;
        }

        let mut findings = self.report.findings.iter().collect::<Vec<_>>();
        findings.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| left.line.cmp(&right.line))
        });

        self.output.push_str("<div class=\"finding-list\">\n");
        for finding in findings.into_iter().take(FINDING_LIMIT) {
            self.output.push_str(
                "<article class=\"finding-card\"><div class=\"finding-head\"><span class=\"pill ",
            );
            self.output.push_str(severity_class(finding.severity));
            self.output.push_str("\">");
            self.output.push_str(severity_label(finding.severity));
            self.output.push_str("</span><strong>");
            self.output
                .push_str(&escape_html(&finding_summary(finding)));
            self.output.push_str("</strong><span class=\"priority\">");
            self.output.push_str(&finding.priority.to_string());
            self.output.push_str("</span></div><p class=\"location\">");
            self.output
                .push_str(&escape_html(&finding_location(finding)));
            self.output.push_str("</p>");
            if !finding.metrics.is_empty() {
                self.output.push_str("<div class=\"metric-list\">");
                for metric in &finding.metrics {
                    self.output.push_str("<span>");
                    self.output.push_str(&escape_html(&format_metric(metric)));
                    self.output.push_str("</span>");
                }
                self.output.push_str("</div>");
            }
            if !finding.rank_explanation.is_empty() {
                self.output.push_str("<p class=\"rank\">");
                self.output
                    .push_str(&escape_html(&finding.rank_explanation));
                self.output.push_str("</p>");
            }
            self.output.push_str("</article>\n");
        }
        if self.report.findings.len() > FINDING_LIMIT {
            self.output.push_str("<p class=\"more\">");
            self.output.push_str(&format!(
                "+{} more findings omitted",
                self.report.findings.len() - FINDING_LIMIT
            ));
            self.output.push_str("</p>\n");
        }
        self.output.push_str("</div>\n</section>\n");
    }
}

#[derive(Debug, Default)]
struct SeverityBreakdown {
    critical: usize,
    warnings: usize,
    info: usize,
}

impl SeverityBreakdown {
    fn from_findings(findings: &[Finding]) -> Self {
        let mut breakdown = Self::default();
        for finding in findings {
            match finding.severity {
                Severity::Critical => breakdown.critical += 1,
                Severity::Warning => breakdown.warnings += 1,
                Severity::Info => breakdown.info += 1,
            }
        }
        breakdown
    }
}

#[derive(Debug)]
struct FileOverview {
    path: String,
    loc: usize,
    imports: usize,
    public_items: usize,
    churn: usize,
    findings: usize,
    risk: u8,
    hotspot_priority: u8,
}

fn ranked_file_overviews(report: &ScanReport) -> Vec<FileOverview> {
    let mut finding_counts = BTreeMap::<&str, usize>::new();
    let mut finding_priorities = BTreeMap::<&str, u8>::new();
    for finding in &report.findings {
        *finding_counts.entry(&finding.path).or_default() += 1;
        finding_priorities
            .entry(&finding.path)
            .and_modify(|priority| *priority = (*priority).max(finding.priority))
            .or_insert(finding.priority);
    }

    let mut hotspot_priorities = BTreeMap::<&str, u8>::new();
    for hotspot in &report.hotspots {
        hotspot_priorities
            .entry(&hotspot.path)
            .and_modify(|priority| *priority = (*priority).max(hotspot.priority))
            .or_insert(hotspot.priority);
    }

    let max_loc = report
        .raw_metrics
        .files
        .iter()
        .map(|file| file.loc)
        .max()
        .unwrap_or(1)
        .max(1);

    let mut files = report
        .raw_metrics
        .files
        .iter()
        .map(|file| {
            let finding_priority = finding_priorities
                .get(file.path.as_str())
                .copied()
                .unwrap_or(0);
            let hotspot_priority = hotspot_priorities
                .get(file.path.as_str())
                .copied()
                .unwrap_or(0);
            FileOverview {
                path: file.path.clone(),
                loc: file.loc,
                imports: file.imports,
                public_items: file.public_items,
                churn: churn_total(file),
                findings: finding_counts.get(file.path.as_str()).copied().unwrap_or(0),
                risk: finding_priority
                    .max(hotspot_priority)
                    .max(((file.loc as f64 / max_loc as f64) * 35.0).round() as u8),
                hotspot_priority,
            }
        })
        .collect::<Vec<_>>();

    files.sort_by(|left, right| {
        right
            .risk
            .cmp(&left.risk)
            .then_with(|| right.findings.cmp(&left.findings))
            .then_with(|| right.loc.cmp(&left.loc))
            .then_with(|| left.path.cmp(&right.path))
    });
    files
}

fn churn_total(file: &FileRawMetric) -> usize {
    file.churn.lines_added + file.churn.lines_deleted + file.churn.recent_weighted_churn
}

fn dimension_breakdown(findings: &[Finding]) -> Vec<(String, usize)> {
    let mut dimensions = BTreeMap::<String, usize>::new();
    for finding in findings {
        if let Some(metric) = finding.metrics.first() {
            *dimensions
                .entry(format!("{:?}", metric.dimension))
                .or_default() += 1;
        }
    }

    dimensions.into_iter().collect()
}

fn finding_summary(finding: &Finding) -> String {
    if !finding.message.is_empty() && finding.kind != FindingKind::DebtMarker {
        return finding.message.clone();
    }

    finding_kind_label(finding.kind)
}

fn finding_kind_label(kind: FindingKind) -> String {
    title_label(&format!("{kind:?}")).to_lowercase()
}

fn finding_location(finding: &Finding) -> String {
    finding
        .line
        .map(|line| format!("{}:{line}", display_path(&finding.path)))
        .unwrap_or_else(|| display_path(&finding.path))
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

fn format_metric(metric: &crate::model::FindingMetric) -> String {
    let mut output = if let Some(threshold) = metric.threshold {
        format!("{} {}/{}", metric.name, metric.value, threshold)
    } else {
        format!("{} {}", metric.name, metric.value)
    };
    if !metric.unit.is_empty() {
        output.push(' ');
        output.push_str(&metric.unit);
    }
    output
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

fn concise_hotspot_reason(reason: &str) -> String {
    reason
        .strip_prefix("hybrid model: ")
        .or_else(|| reason.strip_prefix("static model: "))
        .or_else(|| reason.strip_prefix("churn model: "))
        .unwrap_or(reason)
        .to_string()
}

fn format_duration(duration_ms: u128) -> String {
    if duration_ms < 1_000 {
        format!("{duration_ms} ms")
    } else {
        format!("{:.2} s", duration_ms as f64 / 1_000.0)
    }
}

fn hotspot_model_label(model: crate::cli::HotspotModel) -> &'static str {
    match model {
        crate::cli::HotspotModel::Static => "static",
        crate::cli::HotspotModel::Churn => "churn",
        crate::cli::HotspotModel::Hybrid => "hybrid",
    }
}

fn hotspot_level_label(level: HotspotLevel) -> &'static str {
    match level {
        HotspotLevel::File => "file",
        HotspotLevel::Function => "function",
        HotspotLevel::Type => "type",
    }
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Critical => "critical",
    }
}

fn severity_class(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Critical => "critical",
    }
}

fn heat_class(risk: u8) -> &'static str {
    match risk {
        70..=u8::MAX => "heat-critical",
        45..=69 => "heat-warning",
        20..=44 => "heat-info",
        _ => "heat-calm",
    }
}

fn title_label(value: &str) -> String {
    let mut output = String::new();
    let mut previous_was_lowercase = false;
    for character in value.chars() {
        if character == '_' || character == '-' {
            output.push(' ');
            previous_was_lowercase = false;
            continue;
        }
        if character.is_uppercase() && previous_was_lowercase {
            output.push(' ');
        }
        output.push(character);
        previous_was_lowercase = character.is_lowercase() || character.is_ascii_digit();
    }
    output
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

const STYLES: &str = include_str!("../../assets/report.css");
