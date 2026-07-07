use std::collections::BTreeMap;
use std::io::{self, Write};

use crate::model::{
    FileRawMetric, Finding, FindingKind, Hotspot, HotspotLevel, ScanReport, Severity,
    serialized_finding_kind,
};

const FILE_HEATMAP_PAGE_SIZE: usize = 8;
const FINDING_PAGE_SIZE: usize = 6;
const HOTSPOT_PAGE_SIZE: usize = 5;
const SIMILAR_GROUP_PAGE_SIZE: usize = 4;
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
        self.output.push_str("</main>\n<script>\n");
        self.output.push_str(PAGINATION_SCRIPT);
        self.output.push_str("</script>\n</body>\n</html>\n");
    }

    fn render_header(&mut self) {
        let summary = &self.report.summary;
        let breakdown = SeverityBreakdown::from_findings(&self.report.findings);
        self.output.push_str("<header class=\"hero\">\n");
        self.output.push_str("<div class=\"hero-inner\">\n");
        self.output.push_str("<div class=\"hero-copy\">\n");
        self.output
            .push_str("<p class=\"eyebrow\">Reforge static report</p>\n");
        self.output
            .push_str("<h1>Refactoring signal console</h1>\n");
        self.output.push_str("<p class=\"subhead\">");
        self.output.push_str(&escape_html(&format!(
            "{} files scanned in {} with {} hotspot ranking. Churn is {}{}.",
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
        self.output
            .push_str("<aside class=\"signal-board\" aria-label=\"Scan signal overview\">\n");
        self.output
            .push_str("<div class=\"board-top\"><span>Signal plane</span><strong>");
        self.output
            .push_str(&self.report.summary.finding_count.to_string());
        self.output.push_str("</strong></div>\n");
        self.output
            .push_str("<div class=\"trace-grid\" aria-hidden=\"true\">");
        self.render_trace_cell("critical", breakdown.critical);
        self.render_trace_cell("warning", breakdown.warnings);
        self.render_trace_cell("info", breakdown.info);
        self.render_trace_cell("watch", self.report.summary.hotspot_count);
        self.output.push_str("</div>\n");
        self.output
            .push_str("<div class=\"board-meta\"><span>critical ");
        self.output.push_str(&breakdown.critical.to_string());
        self.output.push_str("</span><span>warning ");
        self.output.push_str(&breakdown.warnings.to_string());
        self.output.push_str("</span><span>info ");
        self.output.push_str(&breakdown.info.to_string());
        self.output.push_str("</span></div>\n");
        self.output.push_str("</aside>\n");
        self.output.push_str("</header>\n");
    }

    fn render_trace_cell(&mut self, class_name: &str, count: usize) {
        self.output.push_str("<span class=\"trace-cell ");
        self.output.push_str(class_name);
        self.output.push_str("\" style=\"--level:");
        self.output.push_str(&count.min(20).to_string());
        self.output.push_str("\"></span>");
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

        self.output
            .push_str("<section class=\"panel-grid diagnostics-grid\">\n");
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
            .push_str("<section class=\"panel file-panel\"><div class=\"section-title\"><h2>File Heatmap</h2><span>");
        self.output.push_str(&escape_html(&format!(
            "{} files ranked",
            self.report.raw_metrics.files.len()
        )));
        self.output.push_str("</span></div>\n");

        let files = ranked_file_overviews(self.report);
        if files.is_empty() {
            self.output
                .push_str("<p class=\"empty\">No raw file metrics were recorded.</p>\n");
            self.output.push_str("</section>\n");
            return;
        }

        self.output.push_str("<div class=\"file-heatmap\">\n");
        for file in &files {
            let heat = heat_class(file.risk);
            self.output.push_str("<div class=\"file-row ");
            self.output.push_str(heat);
            self.output
                .push_str("\" data-page-item=\"files\">\n<div class=\"file-main\"><strong>");
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
        self.render_pagination_controls("files", files.len(), FILE_HEATMAP_PAGE_SIZE);
        self.output.push_str("</div>\n</section>\n");
    }

    fn render_hotspots(&mut self) {
        self.output
            .push_str("<section class=\"panel hotspots-panel\"><div class=\"section-title\"><h2>Watchlist</h2><span>");
        self.output.push_str(&escape_html(&format!(
            "{} ranked targets",
            self.report.hotspots.len()
        )));
        self.output.push_str("</span></div>\n");

        if self.report.hotspots.is_empty() {
            self.output
                .push_str("<p class=\"empty\">No hotspots met the watchlist threshold.</p>\n");
            self.output.push_str("</section>\n");
            return;
        }

        self.output.push_str(
            "<div class=\"report-controls\" data-controls-for=\"hotspots\"><label>Search hotspots <input type=\"search\" data-search-group=\"hotspots\" placeholder=\"Path, symbol, reason\"></label><label>Level <select data-filter-group=\"hotspots\" data-filter-field=\"level\"><option value=\"\">All levels</option><option value=\"file\">File</option><option value=\"function\">Function</option><option value=\"type\">Type</option></select></label></div>\n",
        );
        self.output.push_str("<div class=\"table-like\">\n");
        for hotspot in &self.report.hotspots {
            let target = render_hotspot_target(hotspot);
            let reason = concise_hotspot_reason(&hotspot.reason);
            let search_text = format!("{target} {reason} {}", hotspot_level_label(hotspot.level));
            self.output.push_str(
                "<article class=\"row-card\" data-page-item=\"hotspots\" data-search-text=\"",
            );
            self.output.push_str(&escape_html(&search_text));
            self.output.push_str("\" data-filter-level=\"");
            self.output.push_str(hotspot_level_label(hotspot.level));
            self.output.push_str("\"><div><span class=\"pill ");
            self.output.push_str(severity_class(hotspot.severity));
            self.output.push_str("\">");
            self.output.push_str(severity_label(hotspot.severity));
            self.output.push_str("</span><strong>");
            self.output.push_str(&escape_html(&target));
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
            self.output.push_str(&escape_html(&reason));
            self.output.push_str("</p></article>\n");
        }
        self.output.push_str(
            "<p class=\"empty\" data-filter-empty=\"hotspots\" hidden>No matching hotspots.</p>\n",
        );
        self.render_pagination_controls("hotspots", self.report.hotspots.len(), HOTSPOT_PAGE_SIZE);
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
            .push_str("<section class=\"panel similar-panel\"><div class=\"section-title\"><h2>Similar Function Groups</h2><span>");
        self.output
            .push_str(&escape_html(&format!("{} clusters", groups.len())));
        self.output.push_str("</span></div>\n");

        if groups.is_empty() {
            self.output
                .push_str("<p class=\"empty\">No similar-function groups were reported.</p>\n");
            self.output.push_str("</section>\n");
            return;
        }

        let group_count = groups.len();
        for finding in groups {
            self.output.push_str(
                "<article class=\"group-card\" data-page-item=\"similar-groups\"><div><strong>",
            );
            self.output
                .push_str(&escape_html(&finding_summary(finding)));
            self.output
                .push_str("</strong><span class=\"priority mini\">");
            self.output.push_str(&finding.priority.to_string());
            self.output.push_str("</span></div>");
            if finding.related_locations.len() > RELATED_LOCATION_LIMIT {
                self.render_related_locations_detail(finding, "Group locations");
            }
            self.output.push_str("<ul class=\"related-preview\">\n");
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
                    "+{} more in expandable list",
                    finding.related_locations.len() - RELATED_LOCATION_LIMIT
                ));
                self.output.push_str("</li>\n");
            }
            self.output.push_str("</ul></article>\n");
        }

        self.render_pagination_controls("similar-groups", group_count, SIMILAR_GROUP_PAGE_SIZE);
        self.output.push_str("</section>\n");
    }

    fn render_findings(&mut self) {
        self.output
            .push_str("<section class=\"panel findings-panel\"><div class=\"section-title\"><h2>Findings</h2><span>");
        self.output.push_str(&escape_html(&format!(
            "{} diagnostics",
            self.report.findings.len()
        )));
        self.output.push_str("</span></div>\n");

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

        self.render_finding_controls();
        self.output.push_str("<div class=\"finding-list\">\n");
        let finding_count = findings.len();
        for finding in findings {
            let kind_value = serialized_finding_kind(finding.kind);
            let kind_label = finding_kind_label(finding.kind);
            let location = finding_location(finding);
            let summary = finding_summary(finding);
            let search_text = finding_search_text(finding, &summary, &location, &kind_label);
            self.output.push_str(
                "<article class=\"finding-card\" data-page-item=\"findings\" data-search-text=\"",
            );
            self.output.push_str(&escape_html(&search_text));
            self.output.push_str("\" data-filter-severity=\"");
            self.output.push_str(severity_class(finding.severity));
            self.output.push_str("\" data-filter-kind=\"");
            self.output.push_str(&escape_html(&kind_value));
            self.output.push_str("\" data-sort-priority=\"");
            self.output.push_str(&finding.priority.to_string());
            self.output.push_str("\" data-sort-path=\"");
            self.output.push_str(&escape_html(&location));
            self.output.push_str("\" data-sort-kind=\"");
            self.output.push_str(&escape_html(&kind_label));
            self.output.push_str("\" data-sort-severity=\"");
            self.output
                .push_str(&severity_sort_value(finding.severity).to_string());
            self.output
                .push_str("\"><div class=\"finding-head\"><span class=\"pill ");
            self.output.push_str(severity_class(finding.severity));
            self.output.push_str("\">");
            self.output.push_str(severity_label(finding.severity));
            self.output.push_str("</span><strong>");
            self.output.push_str(&escape_html(&summary));
            self.output.push_str("</strong><span class=\"priority\">");
            self.output.push_str(&finding.priority.to_string());
            self.output.push_str("</span></div><p class=\"location\">");
            self.output.push_str(&escape_html(&location));
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
            self.render_related_locations_detail(finding, "Related locations");
            self.output.push_str("</article>\n");
        }
        self.output.push_str(
            "<p class=\"empty\" data-filter-empty=\"findings\" hidden>No matching findings.</p>\n",
        );
        self.render_pagination_controls("findings", finding_count, FINDING_PAGE_SIZE);
        self.output.push_str("</div>\n</section>\n");
    }

    fn render_finding_controls(&mut self) {
        let mut kinds = BTreeMap::<String, String>::new();
        for finding in &self.report.findings {
            kinds.insert(
                serialized_finding_kind(finding.kind),
                finding_kind_label(finding.kind),
            );
        }

        self.output.push_str("<div class=\"report-controls\" data-controls-for=\"findings\"><label>Search findings <input type=\"search\" data-search-group=\"findings\" placeholder=\"Path, kind, metric, detail\"></label><label>Severity <select data-filter-group=\"findings\" data-filter-field=\"severity\"><option value=\"\">All severities</option><option value=\"critical\">Critical</option><option value=\"warning\">Warning</option><option value=\"info\">Info</option></select></label><label>Kind <select data-filter-group=\"findings\" data-filter-field=\"kind\"><option value=\"\">All kinds</option>");
        for (value, label) in kinds {
            self.output.push_str("<option value=\"");
            self.output.push_str(&escape_html(&value));
            self.output.push_str("\">");
            self.output.push_str(&escape_html(&title_label(&label)));
            self.output.push_str("</option>");
        }
        self.output.push_str("</select></label><label>Sort <select data-sort-group=\"findings\"><option value=\"priority\">Priority</option><option value=\"path\">Path</option><option value=\"kind\">Kind</option><option value=\"severity\">Severity</option></select></label></div>\n");
    }

    fn render_related_locations_detail(&mut self, finding: &Finding, label: &str) {
        if finding.related_locations.is_empty() {
            return;
        }

        self.output
            .push_str("<details class=\"detail-block\"><summary>");
        self.output.push_str(&escape_html(&format!(
            "{label} ({})",
            finding.related_locations.len()
        )));
        self.output.push_str("</summary><ul>\n");
        for location in &finding.related_locations {
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
        self.output.push_str("</ul></details>\n");
    }

    fn render_pagination_controls(&mut self, group: &str, total: usize, page_size: usize) {
        if total <= page_size {
            return;
        }

        self.output
            .push_str("<nav class=\"pager\" data-page-controls=\"");
        self.output.push_str(group);
        self.output.push_str("\" data-page-size=\"");
        self.output.push_str(&page_size.to_string());
        self.output
            .push_str("\" aria-label=\"Section pagination\"><span data-page-range></span><div><button type=\"button\" data-page-action=\"prev\">Prev</button><span data-page-status></span><button type=\"button\" data-page-action=\"next\">Next</button></div></nav>\n");
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

fn finding_search_text(
    finding: &Finding,
    summary: &str,
    location: &str,
    kind_label: &str,
) -> String {
    let mut parts = vec![
        summary.to_string(),
        location.to_string(),
        kind_label.to_string(),
        severity_label(finding.severity).to_string(),
        finding.rank_explanation.clone(),
    ];
    parts.extend(finding.metrics.iter().map(format_metric));
    parts.extend(finding.related_locations.iter().map(|location| {
        format!(
            "{}:{} {}",
            display_path(&location.path),
            location.line,
            location.name.as_deref().unwrap_or("")
        )
    }));
    parts.join(" ")
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

fn severity_sort_value(severity: Severity) -> u8 {
    match severity {
        Severity::Info => 1,
        Severity::Warning => 2,
        Severity::Critical => 3,
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

const PAGINATION_SCRIPT: &str = r#"(() => {
  const groups = new Map();
  document.querySelectorAll("[data-page-item]").forEach((item) => {
    const group = item.dataset.pageItem;
    if (!groups.has(group)) {
      groups.set(group, []);
    }
    groups.get(group).push(item);
  });

  const textValue = (item, key) => (item.dataset[key] || "").toLowerCase();
  const numericValue = (item, key) => Number(item.dataset[key] || 0);

  groups.forEach((items, group) => {
    const controls = document.querySelector(`[data-page-controls="${group}"]`);
    const pageSize = controls ? Math.max(1, Number(controls.dataset.pageSize || 10)) : items.length || 1;
    const status = controls?.querySelector("[data-page-status]");
    const range = controls?.querySelector("[data-page-range]");
    const prev = controls?.querySelector('[data-page-action="prev"]');
    const next = controls?.querySelector('[data-page-action="next"]');
    const search = document.querySelector(`[data-search-group="${group}"]`);
    const filters = Array.from(document.querySelectorAll(`[data-filter-group="${group}"]`));
    const sorter = document.querySelector(`[data-sort-group="${group}"]`);
    const empty = document.querySelector(`[data-filter-empty="${group}"]`);
    const parent = items[0]?.parentElement;
    let page = 0;

    const matchesSearch = (item) => {
      const query = (search?.value || "").trim().toLowerCase();
      if (!query) {
        return true;
      }
      return textValue(item, "searchText").includes(query);
    };

    const matchesFilters = (item) => filters.every((filter) => {
      const value = filter.value;
      if (!value) {
        return true;
      }
      const field = filter.dataset.filterField;
      return field ? item.dataset[`filter${field[0].toUpperCase()}${field.slice(1)}`] === value : true;
    });

    const sortItems = (activeItems) => {
      const sortBy = sorter?.value || "";
      if (!sortBy) {
        return activeItems;
      }
      return [...activeItems].sort((left, right) => {
        if (sortBy === "priority" || sortBy === "severity") {
          return numericValue(right, `sort${sortBy[0].toUpperCase()}${sortBy.slice(1)}`) -
            numericValue(left, `sort${sortBy[0].toUpperCase()}${sortBy.slice(1)}`);
        }
        const leftValue = textValue(left, `sort${sortBy[0].toUpperCase()}${sortBy.slice(1)}`);
        const rightValue = textValue(right, `sort${sortBy[0].toUpperCase()}${sortBy.slice(1)}`);
        return leftValue.localeCompare(rightValue);
      });
    };

    const render = () => {
      const activeItems = sortItems(items.filter((item) => matchesSearch(item) && matchesFilters(item)));
      const pageCount = Math.max(1, Math.ceil(activeItems.length / pageSize));
      page = Math.min(page, pageCount - 1);
      const start = activeItems.length === 0 ? 0 : page * pageSize;
      const end = Math.min(start + pageSize, activeItems.length);
      const visibleItems = new Set(activeItems.slice(start, end));

      if (parent) {
        const marker = controls || empty || null;
        activeItems.forEach((item) => parent.insertBefore(item, marker));
      }

      items.forEach((item) => {
        item.hidden = !visibleItems.has(item);
      });

      if (empty) {
        empty.hidden = activeItems.length !== 0;
      }
      if (controls) {
        controls.hidden = activeItems.length <= pageSize;
      }
      if (status) {
        status.textContent = `${page + 1} / ${pageCount}`;
      }
      if (range) {
        range.textContent = activeItems.length === 0 ? "0 of 0" : `${start + 1}-${end} of ${activeItems.length}`;
      }
      if (prev) {
        prev.disabled = page === 0;
      }
      if (next) {
        next.disabled = page >= pageCount - 1;
      }
    };

    const resetAndRender = () => {
      page = 0;
      render();
    };

    search?.addEventListener("input", resetAndRender);
    filters.forEach((filter) => filter.addEventListener("change", resetAndRender));
    sorter?.addEventListener("change", resetAndRender);
    prev?.addEventListener("click", () => {
      page = Math.max(0, page - 1);
      render();
    });
    next?.addEventListener("click", () => {
      page += 1;
      render();
    });
    render();
  });

  document.querySelectorAll("[data-page-controls]").forEach((controls) => {
    const group = controls.dataset.pageControls;
    if (groups.has(group)) {
      return;
    }
    const items = groups.get(group) || [];
    const pageSize = Math.max(1, Number(controls.dataset.pageSize || 10));
    const pageCount = Math.max(1, Math.ceil(items.length / pageSize));
    const status = controls.querySelector("[data-page-status]");
    const range = controls.querySelector("[data-page-range]");
    const prev = controls.querySelector('[data-page-action="prev"]');
    const next = controls.querySelector('[data-page-action="next"]');
    let page = 0;

    const render = () => {
      const start = page * pageSize;
      const end = Math.min(start + pageSize, items.length);
      items.forEach((item, index) => {
        item.hidden = index < start || index >= end;
      });
      if (status) {
        status.textContent = `${page + 1} / ${pageCount}`;
      }
      if (range) {
        range.textContent = `${start + 1}-${end} of ${items.length}`;
      }
      if (prev) {
        prev.disabled = page === 0;
      }
      if (next) {
        next.disabled = page >= pageCount - 1;
      }
    };

    prev?.addEventListener("click", () => {
      page = Math.max(0, page - 1);
      render();
    });
    next?.addEventListener("click", () => {
      page = Math.min(pageCount - 1, page + 1);
      render();
    });
    render();
  });
})();"#;
