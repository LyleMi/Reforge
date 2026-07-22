use std::collections::BTreeMap;
use std::io::{self, Write};

use crate::baseline::{BaselineDiff, BaselineIssue, BaselineIssueStatus};
use crate::cli::BaselineShow;
use crate::model::{Finding, FindingKind, Issue, MetricId, ScanReport};

const RELATED_LOCATION_LIMIT: usize = 3;

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
    context.render_unity();
    context.render_flow_analysis();
    context.render_coverage();

    if report.findings.is_empty() {
        context.output.push('\n');
        context
            .output
            .push_str("No unsuppressed threshold signals found.\n");
        return output;
    }

    context.render_signal_mix(&breakdown);
    context.render_findings();
    output
}

struct ReportRenderContext<'output, 'report, 'diff> {
    output: &'output mut String,
    report: &'report ScanReport,
    baseline_diff: Option<&'diff BaselineDiff<'report>>,
    color: bool,
}

impl ReportRenderContext<'_, '_, '_> {
    fn render_flow_analysis(&mut self) {
        if self.report.flow_analysis.status == crate::model::FlowAnalysisStatus::Disabled {
            return;
        }
        let flow = &self.report.flow_analysis;
        self.output.push_str(&format!(
            "\nData flow  {:?}  {} functions, {} exact edges\n",
            flow.status, flow.functions_analyzed, flow.exact_edges
        ));
        if flow.unresolved_edges > 0 || flow.truncated_paths > 0 {
            self.output.push_str(&format!(
                "  coverage: {} unresolved edges, {} truncated paths\n",
                flow.unresolved_edges, flow.truncated_paths
            ));
        }
    }

    fn render_unity(&mut self) {
        use crate::model::UnityProjectStatus;
        let unity = &self.report.unity_project;
        if matches!(
            unity.status,
            UnityProjectStatus::NotDetected | UnityProjectStatus::Disabled
        ) {
            return;
        }
        self.output.push_str(&format!(
            "\nUnity  {:?}  Editor {}  serialization {}\n",
            unity.status,
            unity.editor_version.as_deref().unwrap_or("unknown"),
            unity.serialization_mode.as_deref().unwrap_or("unknown")
        ));
        self.output.push_str(&format!(
            "  {} assemblies, {} scenes, {} prefabs, {} assets, {} GUIDs, {} tests\n",
            unity.stats.assemblies,
            unity.stats.scenes,
            unity.stats.prefabs,
            unity.stats.assets,
            unity.stats.guids,
            unity.stats.tests
        ));
        for reason in &unity.degraded_reasons {
            self.output.push_str(&format!("  coverage: {reason}\n"));
        }
    }

    fn render_coverage(&mut self) {
        use crate::model::CoverageStatus;
        let required = self
            .report
            .coverage_manifest
            .iter()
            .filter(|cell| cell.expectation == crate::model::CoverageExpectation::Required)
            .collect::<Vec<_>>();
        let observed = required
            .iter()
            .filter(|cell| cell.status == CoverageStatus::Observed)
            .count();
        self.output.push_str(&format!(
            "\nCoverage  {observed}/{} required cells observed\n",
            required.len()
        ));
        for cell in required.into_iter().filter(|cell| {
            matches!(
                cell.status,
                CoverageStatus::PartiallyObserved | CoverageStatus::Unsupported
            )
        }) {
            self.output.push_str(&format!(
                "  {:?}/{:?}: {:?} - {}\n",
                cell.mechanism, cell.entity_scope, cell.status, cell.reason
            ));
        }
    }

    fn render_header(&mut self) {
        self.output
            .push_str(&paint(self.color, "Reforge scan", AnsiStyle::Header));
        self.output.push('\n');
        self.output.push_str(&format!(
            "{} files  {}  churn {}{}\n",
            self.report.summary.scanned_files,
            format_duration(self.report.summary.duration_ms),
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

    fn render_result(&mut self, _breakdown: &FindingBreakdown) {
        self.output.push('\n');
        self.output
            .push_str(&paint(self.color, "Result", AnsiStyle::Section));
        self.output.push('\n');
        self.render_summary_row("Issues", self.report.summary.issue_count);
        self.render_summary_row("Raw signals", self.report.summary.finding_count);
        if self.report.suppression_summary.suppressed_count > 0 {
            self.render_summary_row("Suppressed", render_suppression_summary(self.report));
        }
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
        self.render_summary_row(
            "Source files",
            format!(
                "{} analyzed / {} discovered",
                self.report.stats.source_files_analyzed, self.report.stats.source_files_discovered
            ),
        );
        self.render_summary_row("Directories", self.report.stats.directories_scanned);
        self.render_summary_row("Function candidates", self.report.stats.function_candidates);
        self.render_summary_row("Engine", &self.report.provenance.engine.version);
        self.render_summary_row(
            "Build revision",
            self.report
                .provenance
                .engine
                .build_revision
                .as_deref()
                .unwrap_or("unavailable"),
        );
        self.render_summary_row(
            "Source revision",
            self.report
                .provenance
                .source
                .git_revision
                .as_deref()
                .unwrap_or("non-Git"),
        );
        self.render_summary_row("Config hash", &self.report.provenance.configuration.hash);
        self.render_summary_row("Policy hash", &self.report.provenance.detector_policy_hash);
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
        self.render_summary_row("Same", diff.summary.same);
        self.render_summary_row("Resolved", diff.summary.resolved);
        if let Some(comparison) = &self.report.baseline_comparison {
            self.render_summary_row("Changed findings", comparison.findings.changed.len());
            self.render_summary_row("Changed issues", comparison.issues.changed.len());
            self.render_summary_row("Lineage candidates", comparison.lineage_candidates.len());
            self.render_summary_row(
                "Change origin",
                if comparison.provenance_change_dimensions.is_empty() {
                    "unknown".to_string()
                } else {
                    comparison.provenance_change_dimensions.join(", ")
                },
            );
        }
        self.render_summary_row(
            "Showing",
            format!(
                "{} ({} of {} current)",
                baseline_show_label(diff.show),
                diff.issues.len(),
                self.report.issues.len()
            ),
        );
    }

    fn render_findings(&mut self) {
        self.output.push('\n');
        let title = self
            .baseline_diff
            .map(|diff| format!("Issues ({})", baseline_show_label(diff.show)))
            .unwrap_or_else(|| "Issues".to_string());
        self.output
            .push_str(&paint(self.color, &title, AnsiStyle::Section));
        self.output.push('\n');

        if let Some(diff) = self.baseline_diff {
            self.render_baseline_issues(diff);
            return;
        }

        self.render_current_issues();
    }

    fn render_baseline_issues(&mut self, diff: &BaselineDiff<'_>) {
        let issues = sorted_baseline_issues(&diff.issues);
        if issues.is_empty() {
            self.output.push_str(&format!(
                "  No issues matched --show {}.\n",
                baseline_show_value(diff.show)
            ));
            return;
        }

        for entry in issues {
            self.output.push_str(&render_diff_issue(entry, self.color));
        }
    }

    fn render_current_issues(&mut self) {
        let primary_ids = self
            .report
            .issues
            .iter()
            .map(|cluster| cluster.primary_finding_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for finding in sorted_findings(&self.report.findings)
            .into_iter()
            .filter(|finding| {
                finding.issue_id.is_none() || primary_ids.contains(finding.id.as_str())
            })
        {
            let cluster_context = self
                .report
                .issues
                .iter()
                .find(|cluster| cluster.primary_finding_id == finding.id)
                .map(|cluster| render_cluster_context(cluster, self.color));
            self.output.push_str(&render_finding(finding, self.color));

            if let Some(cluster_context) = cluster_context {
                self.output.push_str(&cluster_context);
            }

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

    fn render_summary_row(&mut self, label: &str, value: impl std::fmt::Display) {
        self.output.push_str(&format!(
            "  {} {}\n",
            paint(self.color, &format!("{label:<20}"), AnsiStyle::Muted),
            value
        ));
    }
}

include!("human/render.rs");

include!("human/display.rs");
