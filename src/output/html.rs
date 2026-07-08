use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::PI;
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
const DEPENDENCY_MAP_NODE_LIMIT: usize = 24;
const DEPENDENCY_MAP_EDGE_LIMIT: usize = 80;

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
        self.render_dependency_map();
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

    fn render_dependency_map(&mut self) {
        self.output
            .push_str("<section class=\"panel dependency-panel\"><div class=\"section-title\"><h2>Dependency Map</h2><span>");
        self.output.push_str(&escape_html(&format!(
            "{} nodes · {} edges",
            self.report.dependency_graph.nodes.len(),
            self.report.dependency_graph.edges.len()
        )));
        self.output.push_str("</span></div>\n");

        let Some(view) = dependency_map_view(self.report) else {
            self.output.push_str(
                "<p class=\"empty\">No resolved source-file dependencies were recorded.</p>\n",
            );
            self.output.push_str("</section>\n");
            return;
        };

        self.output
            .push_str("<div class=\"dependency-map-layout\">\n");
        self.render_dependency_map_canvas(&view);
        self.render_dependency_map_meta(&view);
        self.output.push_str("</div>\n</section>\n");
    }

    fn render_dependency_map_canvas(&mut self, view: &DependencyMapView) {
        self.output.push_str(
            "<div class=\"dependency-canvas\" role=\"img\" aria-label=\"Dependency graph focus map\"><svg viewBox=\"0 0 840 380\" xmlns=\"http://www.w3.org/2000/svg\">\n<defs><marker id=\"dependency-arrow\" viewBox=\"0 0 10 10\" refX=\"8\" refY=\"5\" markerWidth=\"5\" markerHeight=\"5\" orient=\"auto-start-reverse\"><path d=\"M 0 0 L 10 5 L 0 10 z\"></path></marker></defs>\n",
        );
        for edge in &view.edges {
            self.output.push_str("<line class=\"dependency-edge ");
            self.output.push_str(edge.class_name);
            self.output.push_str("\" x1=\"");
            self.output.push_str(&svg_number(edge.from_x));
            self.output.push_str("\" y1=\"");
            self.output.push_str(&svg_number(edge.from_y));
            self.output.push_str("\" x2=\"");
            self.output.push_str(&svg_number(edge.to_x));
            self.output.push_str("\" y2=\"");
            self.output.push_str(&svg_number(edge.to_y));
            self.output
                .push_str("\" marker-end=\"url(#dependency-arrow)\"></line>\n");
        }
        for node in &view.nodes {
            self.output.push_str("<g class=\"dependency-node ");
            self.output.push_str(node.class_name);
            self.output.push_str("\" transform=\"translate(");
            self.output.push_str(&svg_number(node.x));
            self.output.push(' ');
            self.output.push_str(&svg_number(node.y));
            self.output.push_str(")\"><title>");
            self.output.push_str(&escape_html(&format!(
                "{} · fan-in {} · fan-out {}",
                display_path(&node.path),
                node.fan_in,
                node.fan_out
            )));
            self.output.push_str("</title><circle r=\"");
            self.output.push_str(&svg_number(node.radius));
            self.output.push_str("\"></circle><text>");
            self.output.push_str(&node.index.to_string());
            self.output.push_str("</text></g>\n");
        }
        self.output.push_str("</svg></div>\n");
    }

    fn render_dependency_map_meta(&mut self, view: &DependencyMapView) {
        self.output
            .push_str("<div class=\"dependency-map-meta\">\n");
        self.output
            .push_str("<div class=\"dependency-stats\"><span><strong>");
        self.output.push_str(&view.nodes.len().to_string());
        self.output
            .push_str("</strong> shown nodes</span><span><strong>");
        self.output.push_str(&view.edges.len().to_string());
        self.output
            .push_str("</strong> shown edges</span><span><strong>");
        self.output.push_str(&view.dependency_findings.to_string());
        self.output
            .push_str("</strong> graph findings</span></div>\n");
        self.output
            .push_str("<ol class=\"dependency-node-list\">\n");
        for node in &view.nodes {
            self.output
                .push_str("<li><span class=\"dependency-node-index ");
            self.output.push_str(node.class_name);
            self.output.push_str("\">");
            self.output.push_str(&node.index.to_string());
            self.output.push_str("</span><div><strong>");
            self.output
                .push_str(&escape_html(&display_path(&node.path)));
            self.output.push_str("</strong><small>");
            self.output.push_str(&escape_html(&format!(
                "{} · fan-in {} · fan-out {} · priority {}",
                node.reason, node.fan_in, node.fan_out, node.priority
            )));
            self.output.push_str("</small></div></li>\n");
        }
        self.output.push_str("</ol>\n");
        if view.clipped_nodes || view.clipped_edges {
            self.output.push_str("<p class=\"dependency-note\">");
            self.output.push_str(&escape_html(&format!(
                "Showing {} of {} nodes and {} of {} selected edges.",
                view.nodes.len(),
                view.total_nodes,
                view.edges.len(),
                view.selected_edge_count
            )));
            self.output.push_str("</p>\n");
        }
        self.output.push_str("</div>\n");
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
            self.render_finding_card(finding);
        }
        self.output.push_str(
            "<p class=\"empty\" data-filter-empty=\"findings\" hidden>No matching findings.</p>\n",
        );
        self.render_pagination_controls("findings", finding_count, FINDING_PAGE_SIZE);
        self.output.push_str("</div>\n</section>\n");
    }

    fn render_finding_card(&mut self, finding: &Finding) {
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
        self.render_finding_card_body(finding, &summary, &location);
    }

    fn render_finding_card_body(&mut self, finding: &Finding, summary: &str, location: &str) {
        self.output
            .push_str("\"><div class=\"finding-head\"><span class=\"pill ");
        self.output.push_str(severity_class(finding.severity));
        self.output.push_str("\">");
        self.output.push_str(severity_label(finding.severity));
        self.output.push_str("</span><strong>");
        self.output.push_str(&escape_html(summary));
        self.output.push_str("</strong><span class=\"priority\">");
        self.output.push_str(&finding.priority.to_string());
        self.output.push_str("</span></div><p class=\"location\">");
        self.output.push_str(&escape_html(location));
        self.output.push_str("</p>");
        self.render_finding_metrics(finding);
        if !finding.rank_explanation.is_empty() {
            self.output.push_str("<p class=\"rank\">");
            self.output
                .push_str(&escape_html(&finding.rank_explanation));
            self.output.push_str("</p>");
        }
        self.render_related_locations_detail(finding, "Related locations");
        self.output.push_str("</article>\n");
    }

    fn render_finding_metrics(&mut self, finding: &Finding) {
        if finding.metrics.is_empty() {
            return;
        }

        self.output.push_str("<div class=\"metric-list\">");
        for metric in &finding.metrics {
            self.output.push_str("<span>");
            self.output.push_str(&escape_html(&format_metric(metric)));
            self.output.push_str("</span>");
        }
        self.output.push_str("</div>");
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

#[derive(Debug)]
struct DependencyMapView {
    nodes: Vec<DependencyMapNode>,
    edges: Vec<DependencyMapEdge>,
    total_nodes: usize,
    dependency_findings: usize,
    selected_edge_count: usize,
    clipped_nodes: bool,
    clipped_edges: bool,
}

#[derive(Debug)]
struct DependencyMapNode {
    index: usize,
    path: String,
    fan_in: usize,
    fan_out: usize,
    priority: u8,
    reason: String,
    class_name: &'static str,
    x: f64,
    y: f64,
    radius: f64,
}

#[derive(Debug)]
struct DependencyMapEdge {
    from_x: f64,
    from_y: f64,
    to_x: f64,
    to_y: f64,
    class_name: &'static str,
}

#[derive(Debug)]
struct DependencyNodeCandidate {
    path: String,
    fan_in: usize,
    fan_out: usize,
    priority: u8,
    is_cycle: bool,
    is_hub: bool,
}

#[derive(Debug, Default)]
struct DependencySignalContext {
    priority_by_path: BTreeMap<String, u8>,
    cycle_paths: BTreeSet<String>,
    hub_paths: BTreeSet<String>,
    dependency_findings: usize,
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

fn dependency_map_view(report: &ScanReport) -> Option<DependencyMapView> {
    if report.dependency_graph.nodes.is_empty() {
        return None;
    }

    let context = dependency_signal_context(&report.findings);
    let candidates = dependency_node_candidates(report, &context);
    let selected_paths = selected_dependency_paths(&candidates);
    let selected_candidates = selected_dependency_candidates(candidates, &selected_paths);
    let nodes = layout_dependency_nodes(selected_candidates);
    let node_positions = nodes
        .iter()
        .map(|node| (node.path.clone(), (node.index, node.x, node.y)))
        .collect::<BTreeMap<_, _>>();
    let edges = selected_dependency_edges(report, &selected_paths, &context);
    let selected_edge_count = edges.len();
    let rendered_edges = renderable_dependency_edges(edges, &node_positions, &context);

    Some(DependencyMapView {
        clipped_nodes: report.dependency_graph.nodes.len() > DEPENDENCY_MAP_NODE_LIMIT,
        clipped_edges: selected_edge_count > DEPENDENCY_MAP_EDGE_LIMIT,
        total_nodes: report.dependency_graph.nodes.len(),
        dependency_findings: context.dependency_findings,
        selected_edge_count,
        nodes,
        edges: rendered_edges,
    })
}

fn dependency_signal_context(findings: &[Finding]) -> DependencySignalContext {
    let mut context = DependencySignalContext::default();
    for finding in findings {
        match finding.kind {
            FindingKind::DependencyCycle => record_dependency_cycle(&mut context, finding),
            FindingKind::DependencyHub => record_dependency_hub(&mut context, finding),
            _ => {}
        }
    }
    context
}

fn record_dependency_cycle(context: &mut DependencySignalContext, finding: &Finding) {
    context.dependency_findings += 1;
    context.cycle_paths.insert(finding.path.clone());
    record_dependency_priority(
        &mut context.priority_by_path,
        &finding.path,
        finding.priority,
    );
    for location in &finding.related_locations {
        context.cycle_paths.insert(location.path.clone());
        record_dependency_priority(
            &mut context.priority_by_path,
            &location.path,
            finding.priority,
        );
    }
}

fn record_dependency_hub(context: &mut DependencySignalContext, finding: &Finding) {
    context.dependency_findings += 1;
    context.hub_paths.insert(finding.path.clone());
    record_dependency_priority(
        &mut context.priority_by_path,
        &finding.path,
        finding.priority,
    );
}

fn record_dependency_priority(priorities: &mut BTreeMap<String, u8>, path: &str, priority: u8) {
    priorities
        .entry(path.to_string())
        .and_modify(|current| *current = (*current).max(priority))
        .or_insert(priority);
}

fn dependency_node_candidates(
    report: &ScanReport,
    context: &DependencySignalContext,
) -> Vec<DependencyNodeCandidate> {
    let mut candidates = report
        .dependency_graph
        .nodes
        .iter()
        .map(|node| DependencyNodeCandidate {
            path: node.path.clone(),
            fan_in: node.fan_in,
            fan_out: node.fan_out,
            priority: context
                .priority_by_path
                .get(&node.path)
                .copied()
                .unwrap_or(0),
            is_cycle: context.cycle_paths.contains(&node.path),
            is_hub: context.hub_paths.contains(&node.path),
        })
        .collect::<Vec<_>>();
    candidates.sort_by(compare_dependency_candidates);
    candidates
}

fn selected_dependency_paths(candidates: &[DependencyNodeCandidate]) -> BTreeSet<String> {
    candidates
        .iter()
        .take(DEPENDENCY_MAP_NODE_LIMIT)
        .map(|candidate| candidate.path.clone())
        .collect()
}

fn selected_dependency_candidates(
    candidates: Vec<DependencyNodeCandidate>,
    selected_paths: &BTreeSet<String>,
) -> Vec<DependencyNodeCandidate> {
    candidates
        .into_iter()
        .filter(|candidate| selected_paths.contains(&candidate.path))
        .collect()
}

fn selected_dependency_edges<'a>(
    report: &'a ScanReport,
    selected_paths: &BTreeSet<String>,
    context: &DependencySignalContext,
) -> Vec<&'a crate::model::DependencyGraphEdge> {
    let mut edges = report
        .dependency_graph
        .edges
        .iter()
        .filter(|edge| selected_paths.contains(&edge.from) && selected_paths.contains(&edge.to))
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| {
        dependency_edge_priority(right, context)
            .cmp(&dependency_edge_priority(left, context))
            .then_with(|| left.from.cmp(&right.from))
            .then_with(|| left.to.cmp(&right.to))
    });
    edges
}

fn renderable_dependency_edges(
    edges: Vec<&crate::model::DependencyGraphEdge>,
    node_positions: &BTreeMap<String, (usize, f64, f64)>,
    context: &DependencySignalContext,
) -> Vec<DependencyMapEdge> {
    edges
        .into_iter()
        .take(DEPENDENCY_MAP_EDGE_LIMIT)
        .filter_map(|edge| {
            let (_, from_x, from_y) = node_positions.get(&edge.from).copied()?;
            let (_, to_x, to_y) = node_positions.get(&edge.to).copied()?;
            Some(DependencyMapEdge {
                from_x,
                from_y,
                to_x,
                to_y,
                class_name: dependency_edge_class(edge, context),
            })
        })
        .collect()
}

fn compare_dependency_candidates(
    left: &DependencyNodeCandidate,
    right: &DependencyNodeCandidate,
) -> std::cmp::Ordering {
    right
        .priority
        .cmp(&left.priority)
        .then_with(|| right.is_cycle.cmp(&left.is_cycle))
        .then_with(|| right.is_hub.cmp(&left.is_hub))
        .then_with(|| right.degree().cmp(&left.degree()))
        .then_with(|| right.fan_in.cmp(&left.fan_in))
        .then_with(|| right.fan_out.cmp(&left.fan_out))
        .then_with(|| left.path.cmp(&right.path))
}

impl DependencyNodeCandidate {
    fn degree(&self) -> usize {
        self.fan_in + self.fan_out
    }
}

fn layout_dependency_nodes(candidates: Vec<DependencyNodeCandidate>) -> Vec<DependencyMapNode> {
    let total = candidates.len().max(1);
    candidates
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| {
            let angle = if total == 1 {
                0.0
            } else {
                -PI / 2.0 + (2.0 * PI * index as f64 / total as f64)
            };
            let x = if total == 1 {
                420.0
            } else {
                420.0 + 310.0 * angle.cos()
            };
            let y = if total == 1 {
                190.0
            } else {
                190.0 + 132.0 * angle.sin()
            };
            let degree = candidate.degree();
            let class_name = dependency_node_class(&candidate);
            let reason = dependency_node_reason(&candidate);
            let emphasis = if candidate.is_cycle {
                2.4
            } else if candidate.is_hub {
                1.8
            } else {
                0.0
            };

            DependencyMapNode {
                index: index + 1,
                path: candidate.path,
                fan_in: candidate.fan_in,
                fan_out: candidate.fan_out,
                priority: candidate.priority,
                reason,
                class_name,
                x,
                y,
                radius: 8.5 + degree.min(12) as f64 * 0.7 + emphasis,
            }
        })
        .collect()
}

fn dependency_node_class(candidate: &DependencyNodeCandidate) -> &'static str {
    if candidate.is_cycle {
        "cycle"
    } else if candidate.is_hub {
        "hub"
    } else {
        "normal"
    }
}

fn dependency_node_reason(candidate: &DependencyNodeCandidate) -> String {
    if candidate.is_cycle {
        "cycle member".to_string()
    } else if candidate.is_hub {
        "hub candidate".to_string()
    } else {
        format!("degree {}", candidate.degree())
    }
}

fn dependency_edge_priority(
    edge: &crate::model::DependencyGraphEdge,
    context: &DependencySignalContext,
) -> u16 {
    let endpoint_priority = context
        .priority_by_path
        .get(&edge.from)
        .copied()
        .unwrap_or(0)
        .max(context.priority_by_path.get(&edge.to).copied().unwrap_or(0))
        as u16;
    let cycle_bonus = (context.cycle_paths.contains(&edge.from)
        && context.cycle_paths.contains(&edge.to)) as u16
        * 200;
    let hub_bonus = (context.hub_paths.contains(&edge.from) || context.hub_paths.contains(&edge.to))
        as u16
        * 80;
    cycle_bonus + hub_bonus + endpoint_priority
}

fn dependency_edge_class(
    edge: &crate::model::DependencyGraphEdge,
    context: &DependencySignalContext,
) -> &'static str {
    if context.cycle_paths.contains(&edge.from) && context.cycle_paths.contains(&edge.to) {
        "cycle-edge"
    } else if context.hub_paths.contains(&edge.from) || context.hub_paths.contains(&edge.to) {
        "hub-edge"
    } else {
        "normal-edge"
    }
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

fn svg_number(value: f64) -> String {
    format!("{value:.1}")
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
