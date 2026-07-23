use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use ignore::{DirEntry, WalkBuilder};

use crate::detectors::dependency_graph::scan_dependency_graph_report;
use crate::detectors::drift::{ConceptDriftOptions, scan_concept_drift};
use crate::detectors::manifest::rule_registry;
use crate::detectors::similarity::{
    ParsedSourceFile, SimilarFunctionOptions, SimilarFunctionProgress, SimilarityComparisonStats,
    SourceFile, parse_source_file, scan_parsed_similar_functions_report_with_progress,
};
use crate::detectors::structure::{
    StructureOptions, collect_raw_structure_metrics, is_supported_structure_source,
};
use crate::detectors::unused_functions::{UnusedFunctionOptions, scan_parsed_unused_functions};
use crate::evidence_analysis::DetectedEvidenceInput;
use crate::execution::EffectiveConfig;
use crate::model::{
    ChurnFileMetric, DependencyGraphSnapshot, DetectedEvidence, DetectedMeasurement,
    DirectoryRawMetric, FileRawMetric, FlowAnalysisSummary, FunctionRawMetric, MetricId,
    ParseFailure, ParseFailureReason, RawMetrics, Rule, RunResult, RunStats, RunSummary,
    SourceFailure, SuppressionSummary, TypeRawMetric,
};

mod anchors;
mod churn;
pub(crate) mod config;
mod detection_control;
mod paths;
#[allow(dead_code)]
mod progress;
mod signals;
mod source;
mod thresholds;

use churn::collect_churn_metrics;
use config::{ConfigFile, ConfigSuppression, effective_scan_config_with};
use detection_control::apply_detection_controls;
use progress::ProgressEvent;
pub(crate) use progress::{NoopProgress, ProgressSink};
use signals::ScanSignalContext;

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
struct WorkspaceIndex {
    detections: Vec<DetectedEvidence>,
    parsed_sources: Vec<ParsedSourceFile>,
    codebase_sources: Vec<SourceFile>,
    raw_metrics: RawMetrics,
    dependency_graph: DependencyGraphSnapshot,
    stats: RunStats,
    parse_failures: Vec<ParseFailure>,
    source_failures: Vec<SourceFailure>,
    unresolved_dependency_edges: usize,
    unresolved_dependency_edges_by_file: BTreeMap<String, usize>,
    flow_analysis: FlowAnalysisSummary,
    similarity_comparisons: SimilarityComparisonStats,
    emitted_by_kind: BTreeMap<Rule, usize>,
}

#[derive(Debug, Default)]
struct WorkspacePlan {
    source_files: Vec<PathBuf>,
    directory_source_files: BTreeMap<PathBuf, usize>,
    directories_scanned: usize,
}

#[derive(Debug, Clone, Copy)]
struct FileScanOptions {
    max_file_lines: usize,
    codebase: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExecutionPlan {
    pub codebase: bool,
    pub dataflow: bool,
    pub materialize_flow_ir: bool,
}

impl ExecutionPlan {
    pub(crate) const ALL: Self = Self {
        codebase: true,
        dataflow: true,
        materialize_flow_ir: true,
    };
}

#[allow(dead_code)]
pub(crate) fn detect_path(args: &EffectiveConfig) -> Result<Vec<DetectedEvidence>> {
    let mut progress = NoopProgress;
    Ok(run(args, &mut progress)?.detected_evidence)
}

pub(crate) fn run(args: &EffectiveConfig, progress: &mut dyn ProgressSink) -> Result<RunResult> {
    run_with_plan(args, progress, ExecutionPlan::ALL)
}

pub(crate) fn run_with_plan(
    args: &EffectiveConfig,
    progress: &mut dyn ProgressSink,
    plan: ExecutionPlan,
) -> Result<RunResult> {
    let root = resolve_scan_root(args)?;
    let effective = effective_scan_config_with(args, Some(&ConfigFile::default()))?;
    run_with_effective(args, progress, plan, root, effective)
}

pub(crate) fn run_with_plan_and_config(
    args: &EffectiveConfig,
    progress: &mut dyn ProgressSink,
    plan: ExecutionPlan,
    config: &ConfigFile,
) -> Result<RunResult> {
    let root = resolve_scan_root(args)?;
    let effective = effective_scan_config_with(args, Some(config))?;
    run_with_effective(args, progress, plan, root, effective)
}

fn run_with_effective(
    args: &EffectiveConfig,
    progress: &mut dyn ProgressSink,
    plan: ExecutionPlan,
    root: PathBuf,
    effective: config::ResolvedConfig,
) -> Result<RunResult> {
    let started_at = Instant::now();
    let source_revision = crate::provenance::git_revision(&root);
    let effective_args = effective.args;
    let (mut scan, churn_summary) =
        collect_scan_observations(&root, &effective_args, &effective.data_flow, plan, progress)?;
    anchors::assign_stable_anchors(
        &mut scan.detections,
        &scan.raw_metrics,
        &scan.codebase_sources,
        &scan.parsed_sources,
    );
    scan.emitted_by_kind = counts_by_kind(&scan.detections);
    let post_score_controls =
        apply_post_score_detection_controls(&mut scan, &root, &effective.suppressions)?;
    let manifest = rule_registry();
    let rule_execution = project_scan_coverage(&scan, manifest);

    let summary = build_run_summary(RunSummaryInput {
        scan: &scan,
        controls: &post_score_controls,
        churn: churn_summary,
        duration_ms: if args.reproducible {
            0
        } else {
            started_at.elapsed().as_millis()
        },
    });

    finish_progress(progress, &summary);

    Ok(RunResult {
        source_revision,
        summary,
        stats: scan.stats,
        raw_metrics: scan.raw_metrics,
        suppression_summary: post_score_controls.suppression_summary,
        flow_analysis: scan.flow_analysis,
        parse_failures: scan.parse_failures,
        source_failures: scan.source_failures,
        rule_execution,
        detected_evidence: scan.detections,
    })
}

fn project_scan_coverage(
    scan: &WorkspaceIndex,
    manifest: &[crate::model::RuleSpec],
) -> BTreeMap<Rule, reforge_schema::RuleExecution> {
    coverage(CoverageProjectionInput {
        manifest,
        stats: &scan.stats,
        source_files: &scan.codebase_sources,
        function_count: scan.raw_metrics.functions.len(),
        type_count: scan.raw_metrics.types.len(),
        dependency_nodes: scan.dependency_graph.nodes.len(),
        parse_failures: &scan.parse_failures,
        source_failures: &scan.source_failures,
        unresolved_dependency_edges: scan.unresolved_dependency_edges,
        flow_analysis: &scan.flow_analysis,
        similarity_comparisons: &scan.similarity_comparisons,
    })
}

fn collect_scan_observations(
    root: &Path,
    args: &EffectiveConfig,
    data_flow: &config::DataFlowConfig,
    plan: ExecutionPlan,
    progress: &mut dyn ProgressSink,
) -> Result<(WorkspaceIndex, crate::model::ChurnSummary)> {
    let mut scan = WorkspaceIndex::default();
    let source_plan = collect_source_scan_plan(root, args)?;
    let total_source_files = progress
        .wants_detailed_progress()
        .then_some(source_plan.source_files.len());
    report_scan_start(progress, root, total_source_files);
    scan_sources(
        source_plan,
        args,
        WorkspaceScanOptions {
            execution: plan,
            total_source_files,
        },
        progress,
        &mut scan,
    )?;
    ScanSignalContext {
        root,
        args,
        data_flow,
        plan,
        progress,
        scan: &mut scan,
    }
    .run()?;
    merge_structure_raw_metrics(&mut scan.raw_metrics, &scan.parsed_sources);
    let churn = if plan.codebase {
        collect_churn_metrics(root, args, &mut scan.raw_metrics)?
    } else {
        crate::model::ChurnSummary {
            mode: crate::execution::ChurnMode::Off,
            enabled: false,
            status: "disabled".into(),
            reason: Some("Codebase analysis was not selected".into()),
            window_days: 0,
            max_commit_lines: 0,
        }
    };
    paths::relativize_scan_paths(root, &mut scan);
    Ok((scan, churn))
}

struct RunSummaryInput<'a> {
    scan: &'a WorkspaceIndex,
    controls: &'a PostScoreControls,
    churn: crate::model::ChurnSummary,
    duration_ms: u128,
}

fn build_run_summary(input: RunSummaryInput<'_>) -> RunSummary {
    RunSummary {
        scanned_files: input.scan.stats.source_files_analyzed,
        detected_evidence_count: input.scan.detections.len(),
        similar_function_group_count: input.controls.similar_function_group_count,
        duration_ms: input.duration_ms,
        churn: input.churn,
    }
}

fn finish_progress(progress: &mut dyn ProgressSink, summary: &RunSummary) {
    progress.report(&format!(
        "Finished scan: {} files, {} detections",
        summary.scanned_files, summary.detected_evidence_count
    ));
    progress.finish();
}

include!("coverage.rs");

#[derive(Debug, Clone, PartialEq, Eq)]
struct PostScoreControls {
    similar_function_group_count: usize,
    suppression_summary: SuppressionSummary,
}

fn apply_post_score_detection_controls(
    scan: &mut WorkspaceIndex,
    root: &Path,
    suppressions: &[ConfigSuppression],
) -> Result<PostScoreControls> {
    let telemetry = apply_detection_controls(&mut scan.detections, root, suppressions)?;
    let similar_function_group_count = scan
        .detections
        .iter()
        .filter(|detection| detection.kind == Rule::SimilarFunctions)
        .count();
    Ok(PostScoreControls {
        similar_function_group_count,
        suppression_summary: telemetry.suppression_summary,
    })
}

fn counts_by_kind(detections: &[DetectedEvidence]) -> BTreeMap<Rule, usize> {
    let mut counts = BTreeMap::new();
    for detection in detections {
        *counts.entry(detection.kind).or_insert(0) += 1;
    }
    counts
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

include!("walk.rs");
