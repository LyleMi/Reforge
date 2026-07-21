use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use ignore::{DirEntry, WalkBuilder};

use crate::cli::ScanArgs;
use crate::concept_drift::{ConceptDriftOptions, scan_concept_drift};
use crate::detectors::data_flow::scan_data_flow;
use crate::detectors::dependency_graph::scan_dependency_graph_report;
use crate::detectors::manifest::{detector_manifest, evidence_role, raw_metric_manifest};
use crate::documentation::scan_documentation;
use crate::evidence_analysis::{
    FindingInput, cluster_findings, finalize_metric_context, summarize_raw_metrics,
};
use crate::model::{
    ChurnFileMetric, CoverageExpectation, CoverageManifestEntry, CoverageStatus, CoverageSummary,
    DependencyGraphSnapshot, DetectorExecutionReceipt, DetectorExecutionStatus, DirectoryRawMetric,
    EvidenceRole, FileRawMetric, Finding, FindingKind, FindingMetric, FlowAnalysisSummary,
    FunctionRawMetric, MetricId, ParseFailure, ParseFailureReason, RawMetricCoverage,
    RawMetricCoverageStatus, RawMetrics, SCAN_REPORT_SCHEMA_VERSION, ScanReport, ScanStats,
    ScanSummary, SuppressionSummary, TypeRawMetric, serialized_finding_kind,
};
use crate::similar_functions::{
    ParsedSourceFile, SimilarFunctionOptions, SimilarFunctionProgress, SourceFile,
    parse_source_file, scan_parsed_similar_functions_report_with_progress,
};
use crate::structural::{
    StructureOptions, collect_raw_structure_metrics, is_supported_structure_source,
};
use crate::unused_functions::{UnusedFunctionOptions, scan_parsed_unused_functions};

mod churn;
pub(crate) mod config;
mod finding_control;
mod progress;
mod thresholds;

use churn::collect_churn_metrics;
pub(crate) use config::{
    CONFIG_FILE_NAME, default_config_toml, effective_config_output, validate_config,
};
use config::{ConfigSuppression, effective_scan_config};
use finding_control::apply_finding_controls;
use progress::ProgressEvent;
pub(crate) use progress::{NoopProgress, ProgressSink, StderrProgress};

const DEFAULT_EXCLUDED_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    "out",
    "target",
    "coverage",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".vite",
    "Library",
    "Temp",
    "Logs",
    "UserSettings",
    "obj",
];
#[derive(Debug, Default)]
struct SourceScan {
    findings: Vec<Finding>,
    parsed_sources: Vec<ParsedSourceFile>,
    structure_sources: Vec<SourceFile>,
    raw_metrics: RawMetrics,
    dependency_graph: DependencyGraphSnapshot,
    stats: ScanStats,
    parse_failures: Vec<ParseFailure>,
    unresolved_dependency_edges: usize,
    unresolved_dependency_edges_by_file: BTreeMap<String, usize>,
    flow_analysis: FlowAnalysisSummary,
}

#[derive(Debug, Default)]
struct SourceScanPlan {
    source_files: Vec<PathBuf>,
    directory_source_files: BTreeMap<PathBuf, usize>,
    directories_scanned: usize,
}

#[derive(Debug, Clone, Copy)]
struct FileScanOptions {
    max_file_lines: usize,
}

#[allow(dead_code)]
pub(crate) fn scan_path(args: &ScanArgs) -> Result<Vec<Finding>> {
    let mut progress = NoopProgress;
    Ok(scan_report(args, &mut progress)?.findings)
}

pub(crate) fn scan_report(args: &ScanArgs, progress: &mut dyn ProgressSink) -> Result<ScanReport> {
    let started_at = Instant::now();
    let root = resolve_scan_root(args)?;
    let effective = effective_scan_config(args, &root)?;
    let effective_args = effective.args;
    let (mut scan, unity_scan, churn_summary) =
        collect_scan_observations(&root, &effective_args, &effective.data_flow, progress)?;
    let metrics_summary = summarize_raw_metrics(&scan.raw_metrics);
    scan.findings
        .retain(|finding| evidence_role(finding.kind) != EvidenceRole::CompositeSummary);
    finalize_metric_context(&mut scan.findings, &scan.raw_metrics);
    let post_score_controls = apply_post_score_finding_controls(
        &mut scan,
        &root,
        &effective_args,
        &effective.suppressions,
    )?;
    let issues = cluster_findings(&mut scan.findings);
    let agent_evidence = build_agent_evidence(&scan, &issues);
    let manifest = detector_manifest();
    let (coverage_manifest, coverage_summary, detector_execution, raw_metric_coverage) =
        project_scan_coverage(&scan, &manifest, &churn_summary, unity_scan.report.status);

    let summary = build_scan_summary(ScanSummaryInput {
        scan: &scan,
        issues: &issues,
        controls: &post_score_controls,
        churn: churn_summary,
        duration_ms: started_at.elapsed().as_millis(),
    });

    finish_progress(progress, &summary);

    Ok(ScanReport {
        schema_version: SCAN_REPORT_SCHEMA_VERSION,
        summary,
        stats: scan.stats,
        metrics_summary,
        raw_metrics: scan.raw_metrics,
        raw_metric_manifest: raw_metric_manifest(),
        dependency_graph: scan.dependency_graph,
        agent_evidence,
        unity_project: unity_scan.report,
        suppression_summary: post_score_controls.suppression_summary,
        flow_analysis: scan.flow_analysis,
        coverage_manifest,
        coverage_summary,
        detector_execution,
        raw_metric_coverage,
        issues,
        detector_manifest: manifest,
        findings: scan.findings,
    })
}

fn project_scan_coverage(
    scan: &SourceScan,
    manifest: &[crate::model::DetectorManifestEntry],
    churn: &crate::model::ChurnSummary,
    unity_status: crate::model::UnityProjectStatus,
) -> (
    Vec<CoverageManifestEntry>,
    CoverageSummary,
    Vec<DetectorExecutionReceipt>,
    Vec<RawMetricCoverage>,
) {
    coverage(CoverageProjectionInput {
        manifest,
        stats: &scan.stats,
        source_files: &scan.structure_sources,
        function_count: scan.raw_metrics.functions.len(),
        type_count: scan.raw_metrics.types.len(),
        findings: &scan.findings,
        parse_failures: &scan.parse_failures,
        unresolved_dependency_edges: scan.unresolved_dependency_edges,
        flow_analysis: &scan.flow_analysis,
        churn,
        unity_observed: matches!(
            unity_status,
            crate::model::UnityProjectStatus::Observed
                | crate::model::UnityProjectStatus::PartiallyObserved
        ),
    })
}

fn collect_scan_observations(
    root: &Path,
    args: &ScanArgs,
    data_flow: &config::DataFlowConfig,
    progress: &mut dyn ProgressSink,
) -> Result<(
    SourceScan,
    crate::unity::UnityScan,
    crate::model::ChurnSummary,
)> {
    let mut scan = SourceScan::default();
    let source_plan = collect_source_scan_plan(root, args)?;
    let total_source_files = progress
        .wants_detailed_progress()
        .then_some(source_plan.source_files.len());
    report_scan_start(progress, root, total_source_files);
    scan_sources(source_plan, args, total_source_files, progress, &mut scan)?;
    run_scan_signals(root, args, data_flow, progress, &mut scan)?;
    let mut unity_scan = crate::unity::scan_unity(root, args)?;
    scan.findings.append(&mut unity_scan.findings);
    scan.findings.extend(scan_documentation(root)?);
    merge_structure_raw_metrics(&mut scan.raw_metrics, &scan.parsed_sources);
    let churn = collect_churn_metrics(root, args, &mut scan.raw_metrics)?;
    Ok((scan, unity_scan, churn))
}

struct ScanSummaryInput<'a> {
    scan: &'a SourceScan,
    issues: &'a [crate::model::Issue],
    controls: &'a PostScoreControls,
    churn: crate::model::ChurnSummary,
    duration_ms: u128,
}

fn build_scan_summary(input: ScanSummaryInput<'_>) -> ScanSummary {
    ScanSummary {
        scanned_files: input.scan.stats.source_files_scanned,
        finding_count: input.scan.findings.len(),
        issue_count: input.issues.len(),
        similar_function_group_count: input.controls.similar_function_group_count,
        duration_ms: input.duration_ms,
        churn: input.churn,
    }
}

fn finish_progress(progress: &mut dyn ProgressSink, summary: &ScanSummary) {
    progress.report(&format!(
        "Finished scan: {} files, {} findings",
        summary.scanned_files, summary.finding_count
    ));
    progress.finish();
}

include!("agent_evidence.rs");
include!("coverage.rs");

fn run_scan_signals(
    root: &Path,
    args: &ScanArgs,
    data_flow: &config::DataFlowConfig,
    progress: &mut dyn ProgressSink,
    scan: &mut SourceScan,
) -> Result<()> {
    let mut signals = ScanSignalContext {
        root,
        args,
        progress,
        scan,
    };
    signals.scan_structural_signals()?;
    signals.scan_unused_function_signals();
    signals.scan_dependency_graph_signals();
    signals.scan_data_flow_signals(data_flow)?;
    signals.scan_concept_drift_signals();
    signals.scan_similarity_signals()?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PostScoreControls {
    similar_function_group_count: usize,
    suppression_summary: SuppressionSummary,
}

fn apply_post_score_finding_controls(
    scan: &mut SourceScan,
    root: &Path,
    args: &ScanArgs,
    suppressions: &[ConfigSuppression],
) -> Result<PostScoreControls> {
    let suppression_summary = apply_finding_controls(&mut scan.findings, root, args, suppressions)?;
    let similar_function_group_count = scan
        .findings
        .iter()
        .filter(|finding| finding.kind == FindingKind::SimilarFunctions)
        .count();
    Ok(PostScoreControls {
        similar_function_group_count,
        suppression_summary,
    })
}

fn merge_structure_raw_metrics(raw_metrics: &mut RawMetrics, parsed_sources: &[ParsedSourceFile]) {
    let structure_metrics = collect_raw_structure_metrics(parsed_sources);
    let by_path = structure_metrics
        .into_iter()
        .map(|metric| (metric.path.clone(), metric))
        .collect::<BTreeMap<_, _>>();

    for file_metric in &mut raw_metrics.files {
        if let Some(structure_metric) = by_path.get(&file_metric.path) {
            file_metric.imports = structure_metric.imports;
            file_metric.public_items = structure_metric.public_items;
            file_metric.is_test = structure_metric.is_test;
        }
    }

    raw_metrics.functions = by_path
        .values()
        .flat_map(|metric| metric.functions.clone())
        .map(|function| FunctionRawMetric {
            path: function.path,
            name: function.name,
            line: function.line,
            loc: function.loc,
            complexity: function.complexity,
            nesting_depth: function.nesting_depth,
            parameter_count: function.parameter_count,
            is_test: function.is_test,
        })
        .collect();
    raw_metrics.types = by_path
        .values()
        .flat_map(|metric| metric.types.clone())
        .map(|type_metric| TypeRawMetric {
            path: type_metric.path,
            name: type_metric.name,
            line: type_metric.line,
            loc: type_metric.loc,
            member_count: type_metric.member_count,
            is_test: type_metric.is_test,
        })
        .collect();
}

struct ScanSignalContext<'a> {
    root: &'a Path,
    args: &'a ScanArgs,
    progress: &'a mut dyn ProgressSink,
    scan: &'a mut SourceScan,
}

impl ScanSignalContext<'_> {
    fn scan_data_flow_signals(&mut self, config: &config::DataFlowConfig) -> Result<()> {
        if config.mode == config::DataFlowMode::Off {
            return Ok(());
        }
        self.progress.report(&format!(
            "Analyzing exact Rust data flow in {} parsed files",
            self.scan.parsed_sources.len()
        ));
        let mut flow = scan_data_flow(
            self.root,
            &self.scan.parsed_sources,
            &self.scan.parse_failures,
            config,
        )?;
        self.scan.findings.append(&mut flow.findings);
        self.scan.flow_analysis = flow.summary;
        Ok(())
    }

    fn scan_dependency_graph_signals(&mut self) {
        self.progress.report(&format!(
            "Analyzing dependency graph in {} files",
            self.scan.structure_sources.len()
        ));
        let dependency_scan = scan_dependency_graph_report(&self.scan.structure_sources, self.root);
        self.scan.unresolved_dependency_edges = dependency_scan.unresolved_edges;
        self.scan.unresolved_dependency_edges_by_file = dependency_scan.unresolved_by_file;
        self.scan.dependency_graph = dependency_scan.snapshot;
        self.scan.findings.extend(dependency_scan.findings);
    }

    fn scan_concept_drift_signals(&mut self) {
        self.progress.report(&format!(
            "Analyzing concept drift signals in {} files",
            self.scan.structure_sources.len()
        ));
        let options = ConceptDriftOptions {
            min_repeated_occurrences: self.args.min_repeated_literal_occurrences,
            min_data_shape_occurrences: self.args.min_data_clump_occurrences,
            max_dir_files: self.args.max_dir_files,
            include_test_structure: self.args.include_test_structure,
        };
        self.scan
            .findings
            .extend(scan_concept_drift(&self.scan.structure_sources, &options));
    }

    fn scan_structural_signals(&mut self) -> Result<()> {
        self.progress.report(&format!(
            "Analyzing structural signals in {} files",
            self.scan.parsed_sources.len()
        ));
        let structure_options = StructureOptions {
            max_function_lines: self.args.max_function_lines,
            max_function_complexity: self.args.max_function_complexity,
            max_nesting_depth: self.args.max_nesting_depth,
            max_function_parameters: self.args.max_function_parameters,
            max_type_lines: self.args.max_type_lines,
            max_type_members: self.args.max_type_members,
            max_imports: self.args.max_imports,
            max_public_items: self.args.max_public_items,
            max_functions_per_file: self.args.function_proliferation.max_functions_per_file,
            max_functions_per_100_lines: self
                .args
                .function_proliferation
                .max_functions_per_100_lines,
            max_small_function_ratio: self.args.function_proliferation.max_small_function_ratio,
            min_repeated_literal_occurrences: self.args.min_repeated_literal_occurrences,
            min_data_clump_occurrences: self.args.min_data_clump_occurrences,
            max_dir_files: self.args.max_dir_files,
            include_test_structure: self.args.include_test_structure,
        };
        self.scan
            .findings
            .extend(crate::structural::scan_parsed_structure(
                &self.scan.parsed_sources,
                &structure_options,
            )?);
        Ok(())
    }

    fn scan_unused_function_signals(&mut self) {
        self.progress.report(&format!(
            "Analyzing unused functions in {} files",
            self.scan.parsed_sources.len()
        ));
        let options = UnusedFunctionOptions {
            include_tests: self.args.include_test_structure,
        };
        self.scan.findings.extend(scan_parsed_unused_functions(
            &self.scan.parsed_sources,
            &options,
        ));
    }

    fn scan_similarity_signals(&mut self) -> Result<usize> {
        self.progress.report(&format!(
            "Analyzing similar functions in {} files",
            self.scan.parsed_sources.len()
        ));

        let similarity_options = SimilarFunctionOptions {
            min_group_size: self.args.min_similar_functions,
            min_tokens: self.args.min_function_tokens,
            threshold: self.args.function_similarity,
            include_test_similarity: self.args.include_test_similarity,
        };
        let mut similarity_progress = ScanSimilarityProgress {
            progress: self.progress,
        };
        let similarity_scan = scan_parsed_similar_functions_report_with_progress(
            &self.scan.parsed_sources,
            &similarity_options,
            &mut similarity_progress,
        )?;
        self.scan.stats.function_candidates = similarity_scan.candidate_count;
        self.scan.findings.extend(similarity_scan.findings);

        Ok(self
            .scan
            .findings
            .iter()
            .filter(|finding| finding.kind == FindingKind::SimilarFunctions)
            .count())
    }
}

include!("walk.rs");
