use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use ignore::{DirEntry, WalkBuilder};

use crate::agent_drift::{AgentDriftOptions, scan_agent_drift};
use crate::cli::ScanArgs;
use crate::detectors::dependency_graph::scan_dependency_graph_report;
use crate::detectors::manifest::{detector_manifest, evidence_role, raw_metric_manifest};
use crate::documentation::scan_documentation;
use crate::model::{
    ChurnFileMetric, CoverageExpectation, CoverageManifestEntry, CoverageStatus, CoverageSummary,
    DependencyGraphSnapshot, DetectorExecutionReceipt, DetectorExecutionStatus, DirectoryRawMetric,
    EvidenceRole, FileRawMetric, Finding, FindingKind, FindingMetric, FunctionRawMetric, MetricId,
    ParseFailure, ParseFailureReason, RawMetricCoverage, RawMetricCoverageStatus, RawMetrics,
    SCAN_REPORT_SCHEMA_VERSION, ScanReport, ScanStats, ScanSummary, SuppressionSummary,
    TypeRawMetric,
};
#[cfg(test)]
use crate::scoring::finalize_scoring;
use crate::scoring::{
    FindingInput, StaticRiskThresholds, cluster_findings, finalize_scoring_with_policy, finding,
    load_scoring_policy, rank_hotspots, summarize_raw_metrics,
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
mod config;
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

#[cfg(test)]
use churn::parse_git_numstat_churn;
#[cfg(test)]
pub(crate) use progress::WriterProgress;

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
    let scoring_policy = load_scoring_policy(effective.scoring_policy_path.as_deref())?;
    let effective_args = effective.args;
    let mut scan = SourceScan::default();
    let source_plan = collect_source_scan_plan(&root, &effective_args)?;

    let total_source_files = progress
        .wants_detailed_progress()
        .then_some(source_plan.source_files.len());

    report_scan_start(progress, &root, total_source_files);
    scan_sources(
        source_plan,
        &effective_args,
        total_source_files,
        progress,
        &mut scan,
    )?;
    run_scan_signals(&root, &effective_args, progress, &mut scan)?;
    scan.findings.extend(scan_documentation(&root)?);
    merge_structure_raw_metrics(&mut scan.raw_metrics, &scan.parsed_sources);
    let churn_summary = collect_churn_metrics(&root, &effective_args, &mut scan.raw_metrics)?;
    let metrics_summary = summarize_raw_metrics(&scan.raw_metrics);
    let hotspots = rank_hotspots(
        &scan.raw_metrics,
        &metrics_summary,
        effective_args
            .hotspot_model
            .expect("effective args should set hotspot model"),
        StaticRiskThresholds::from(&effective_args),
    );
    scan.findings
        .retain(|finding| evidence_role(finding.kind) != EvidenceRole::CompositeSummary);
    finalize_scoring_with_policy(
        &mut scan.findings,
        &scan.raw_metrics,
        &hotspots,
        &scoring_policy,
    );
    let post_score_controls = apply_post_score_finding_controls(
        &mut scan,
        &root,
        &effective_args,
        &effective.suppressions,
    )?;
    let issues = cluster_findings(&mut scan.findings);
    let manifest = detector_manifest();
    let (coverage_manifest, coverage_summary, detector_execution, raw_metric_coverage) =
        coverage(CoverageProjectionInput {
            manifest: &manifest,
            stats: &scan.stats,
            source_files: &scan.structure_sources,
            function_count: scan.raw_metrics.functions.len(),
            type_count: scan.raw_metrics.types.len(),
            findings: &scan.findings,
            parse_failures: &scan.parse_failures,
            unresolved_dependency_edges: scan.unresolved_dependency_edges,
            churn: &churn_summary,
        });

    let summary = ScanSummary {
        scanned_files: scan.stats.source_files_scanned,
        finding_count: scan.findings.len(),
        issue_count: issues.len(),
        hotspot_count: hotspots.len(),
        similar_function_group_count: post_score_controls.similar_function_group_count,
        duration_ms: started_at.elapsed().as_millis(),
        hotspot_model: effective_args
            .hotspot_model
            .expect("effective args should set hotspot model"),
        churn: churn_summary,
    };

    finish_progress(progress, &summary);

    Ok(ScanReport {
        schema_version: SCAN_REPORT_SCHEMA_VERSION,
        summary,
        stats: scan.stats,
        metrics_summary,
        raw_metrics: scan.raw_metrics,
        raw_metric_manifest: raw_metric_manifest(),
        dependency_graph: scan.dependency_graph,
        hotspots,
        suppression_summary: post_score_controls.suppression_summary,
        coverage_manifest,
        coverage_summary,
        detector_execution,
        raw_metric_coverage,
        scoring_policy,
        issues,
        detector_manifest: manifest,
        findings: scan.findings,
    })
}

fn finish_progress(progress: &mut dyn ProgressSink, summary: &ScanSummary) {
    progress.report(&format!(
        "Finished scan: {} files, {} findings",
        summary.scanned_files, summary.finding_count
    ));
    progress.finish();
}

struct CoverageProjectionInput<'a> {
    manifest: &'a [crate::model::DetectorManifestEntry],
    stats: &'a ScanStats,
    source_files: &'a [SourceFile],
    function_count: usize,
    type_count: usize,
    findings: &'a [Finding],
    parse_failures: &'a [ParseFailure],
    unresolved_dependency_edges: usize,
    churn: &'a crate::model::ChurnSummary,
}

fn coverage(
    input: CoverageProjectionInput<'_>,
) -> (
    Vec<CoverageManifestEntry>,
    CoverageSummary,
    Vec<DetectorExecutionReceipt>,
    Vec<RawMetricCoverage>,
) {
    let CoverageProjectionInput {
        manifest,
        stats,
        source_files,
        function_count,
        type_count,
        findings,
        parse_failures,
        unresolved_dependency_edges,
        churn,
    } = input;
    let detected_languages = source_files
        .iter()
        .filter_map(|source| detected_language(&source.path))
        .collect::<BTreeSet<_>>();
    let entity_counts = BTreeMap::from([
        (crate::model::EntityScope::Repository, 1),
        (
            crate::model::EntityScope::Directory,
            stats.directories_scanned,
        ),
        (crate::model::EntityScope::File, stats.source_files_scanned),
        (crate::model::EntityScope::Function, function_count),
        (crate::model::EntityScope::Type, type_count),
    ]);
    let detector_execution = manifest
        .iter()
        .map(|entry| {
            let applicable = detector_is_applicable(entry, &detected_languages);
            let analyzed_entities = if applicable {
                entity_counts
                    .get(&entry.entity_scope)
                    .copied()
                    .unwrap_or_else(|| {
                        findings
                            .iter()
                            .filter(|finding| finding.kind == entry.kind)
                            .count()
                    })
            } else {
                0
            };
            let parse_sensitive = detector_requires_parse(entry);
            let unresolved = if entry.approach == crate::model::DetectionApproach::GraphAnalysis {
                unresolved_dependency_edges
            } else {
                0
            };
            DetectorExecutionReceipt {
                kind: entry.kind,
                status: if applicable {
                    DetectorExecutionStatus::Completed
                } else {
                    DetectorExecutionStatus::NotApplicable
                },
                analyzed_entities,
                candidate_groups: if entry.entity_scope == crate::model::EntityScope::FindingGroup {
                    findings
                        .iter()
                        .filter(|finding| finding.kind == entry.kind)
                        .count()
                } else {
                    0
                },
                unobservable_count: if applicable && parse_sensitive {
                    parse_failures.len() + unresolved
                } else {
                    0
                },
                unobservable_reasons: if applicable {
                    [
                        (!parse_failures.is_empty() && parse_sensitive).then(|| {
                            format!(
                                "{} source files failed syntax parsing",
                                parse_failures.len()
                            )
                        }),
                        (unresolved > 0).then(|| {
                            format!("{unresolved} dependency edges could not be resolved")
                        }),
                    ]
                    .into_iter()
                    .flatten()
                    .collect()
                } else {
                    Vec::new()
                },
            }
        })
        .collect::<Vec<_>>();
    let coverage_manifest = coverage_targets().into_iter().map(|(mechanism, entity_scope, expectation)| {
            let entries = manifest.iter().filter(|entry| entry.mechanism == mechanism && entry.entity_scope == entity_scope).collect::<Vec<_>>();
            let applicable = entries.iter().filter(|entry| detector_is_applicable(entry, &detected_languages)).collect::<Vec<_>>();
            let completed_detectors = applicable.iter().map(|entry| entry.kind).collect::<Vec<_>>();
            let entity_count = entity_counts.get(&entity_scope).copied().unwrap_or_else(|| applicable.iter().map(|entry| findings.iter().filter(|finding| finding.kind == entry.kind).count()).sum());
            let graph_cell = applicable.iter().any(|entry| entry.approach == crate::model::DetectionApproach::GraphAnalysis);
            let partial = (!parse_failures.is_empty() && applicable.iter().any(|entry| detector_requires_parse(entry))) || (graph_cell && unresolved_dependency_edges > 0);
            let status = match expectation {
                CoverageExpectation::Planned => CoverageStatus::Planned,
                CoverageExpectation::IntentionallyOutOfScope => CoverageStatus::IntentionallyOutOfScope,
                CoverageExpectation::Required if applicable.is_empty() => CoverageStatus::Unsupported,
                CoverageExpectation::Required if partial => CoverageStatus::PartiallyObserved,
                CoverageExpectation::Required if entity_count == 0 => CoverageStatus::NoEntities,
                CoverageExpectation::Required => CoverageStatus::Observed,
            };
            CoverageManifestEntry {
                mechanism,
                entity_scope,
                expectation,
                status,
                reason: coverage_reason(status).into(),
                detectors: entries.into_iter().map(|entry| entry.kind).collect(),
                completed_detectors,
                entity_count,
                unobservable_reasons: if partial { [(!parse_failures.is_empty()).then(|| format!("{} source files failed syntax parsing", parse_failures.len())), (graph_cell && unresolved_dependency_edges > 0).then(|| format!("{unresolved_dependency_edges} dependency edges could not be resolved"))].into_iter().flatten().collect() } else { Vec::new() },
            }
        }).collect();
    let coverage_summary = CoverageSummary {
        detected_languages: detected_languages.iter().cloned().collect(),
        applicable_detectors: manifest
            .iter()
            .filter(|entry| detector_is_applicable(entry, &detected_languages))
            .map(|entry| entry.kind)
            .collect(),
        analyzed_entities: entity_counts,
        parse_failures: parse_failures.to_vec(),
        unresolved_dependency_edges,
        unobservable_reasons: if parse_failures.is_empty() {
            Vec::new()
        } else {
            vec![format!(
                "{} source files failed syntax parsing",
                parse_failures.len()
            )]
        },
    };
    let raw_metric_coverage = canonical_raw_metrics()
        .iter()
        .copied()
        .map(|metric| {
            raw_metric_observation(
                metric,
                stats,
                function_count,
                type_count,
                parse_failures,
                churn,
            )
        })
        .collect();
    (
        coverage_manifest,
        coverage_summary,
        detector_execution,
        raw_metric_coverage,
    )
}

fn detector_requires_parse(entry: &crate::model::DetectorManifestEntry) -> bool {
    matches!(
        entry.entity_scope,
        crate::model::EntityScope::Function | crate::model::EntityScope::Type
    ) || matches!(
        entry.approach,
        crate::model::DetectionApproach::ParsedAnalysis
            | crate::model::DetectionApproach::GraphAnalysis
    )
}

fn coverage_targets() -> Vec<(
    crate::model::SignalMechanism,
    crate::model::EntityScope,
    CoverageExpectation,
)> {
    use crate::model::{EntityScope as E, SignalMechanism as M};
    const MECHANISMS: [M; 7] = [
        M::CognitiveLoad,
        M::DependencyPropagation,
        M::ResponsibilityDispersion,
        M::DuplicationDivergence,
        M::ChangePressure,
        M::VerificationDifficulty,
        M::KnowledgeDrift,
    ];
    const SCOPES: [E; 6] = [
        E::Repository,
        E::Directory,
        E::File,
        E::Function,
        E::Type,
        E::FindingGroup,
    ];
    let required = |m, e| {
        matches!(
            (m, e),
            (M::CognitiveLoad, E::Function)
                | (M::DependencyPropagation, E::File | E::FindingGroup)
                | (
                    M::ResponsibilityDispersion,
                    E::Directory | E::File | E::Type
                )
                | (M::DuplicationDivergence, E::FindingGroup)
                | (M::ChangePressure, E::File | E::FindingGroup)
                | (M::VerificationDifficulty, E::FindingGroup)
                | (M::KnowledgeDrift, E::Directory | E::Repository)
        )
    };
    MECHANISMS
        .into_iter()
        .flat_map(|m| {
            SCOPES.into_iter().map(move |e| {
                (
                    m,
                    e,
                    if required(m, e) {
                        CoverageExpectation::Required
                    } else {
                        CoverageExpectation::IntentionallyOutOfScope
                    },
                )
            })
        })
        .collect()
}

fn coverage_reason(status: CoverageStatus) -> &'static str {
    match status {
        CoverageStatus::Observed => "all applicable detectors completed",
        CoverageStatus::PartiallyObserved => {
            "applicable detectors completed with unobservable entities"
        }
        CoverageStatus::Unsupported => "no detector supports the detected languages",
        CoverageStatus::NoEntities => "no entities were available for analysis",
        CoverageStatus::Planned => "coverage is planned for a future schema",
        CoverageStatus::IntentionallyOutOfScope => {
            "this mechanism and scope are intentionally out of scope"
        }
    }
}

fn canonical_raw_metrics() -> &'static [MetricId] {
    use MetricId::*;
    &[
        FileLoc,
        FileImports,
        FilePublicItems,
        FileIsTest,
        DirectorySourceFiles,
        FunctionLoc,
        FunctionComplexity,
        FunctionNestingDepth,
        FunctionParameterCount,
        FunctionIsTest,
        TypeLoc,
        TypeMemberCount,
        TypeIsTest,
        ChurnCommitsTouched,
        ChurnLinesAdded,
        ChurnLinesDeleted,
        ChurnAuthorsCount,
        ChurnRecentWeighted,
    ]
}

fn raw_metric_observation(
    metric: MetricId,
    stats: &ScanStats,
    function_count: usize,
    type_count: usize,
    failures: &[ParseFailure],
    churn: &crate::model::ChurnSummary,
) -> RawMetricCoverage {
    let is_churn = matches!(
        metric,
        MetricId::ChurnCommitsTouched
            | MetricId::ChurnLinesAdded
            | MetricId::ChurnLinesDeleted
            | MetricId::ChurnAuthorsCount
            | MetricId::ChurnRecentWeighted
    );
    let parse_sensitive = matches!(
        metric,
        MetricId::FunctionLoc
            | MetricId::FunctionComplexity
            | MetricId::FunctionNestingDepth
            | MetricId::FunctionParameterCount
            | MetricId::FunctionIsTest
            | MetricId::TypeLoc
            | MetricId::TypeMemberCount
            | MetricId::TypeIsTest
            | MetricId::FileImports
            | MetricId::FilePublicItems
    );
    let entity_count = match metric {
        MetricId::DirectorySourceFiles => stats.directories_scanned,
        MetricId::FunctionLoc
        | MetricId::FunctionComplexity
        | MetricId::FunctionNestingDepth
        | MetricId::FunctionParameterCount
        | MetricId::FunctionIsTest => function_count,
        MetricId::TypeLoc | MetricId::TypeMemberCount | MetricId::TypeIsTest => type_count,
        _ => stats.source_files_scanned,
    };
    let status = if is_churn && !churn.enabled {
        RawMetricCoverageStatus::Unavailable
    } else if parse_sensitive && !failures.is_empty() {
        RawMetricCoverageStatus::PartiallyObserved
    } else {
        RawMetricCoverageStatus::Observed
    };
    RawMetricCoverage {
        metric,
        status,
        entity_count,
        reason: match status {
            RawMetricCoverageStatus::Observed => "metric observed for available entities",
            RawMetricCoverageStatus::PartiallyObserved => {
                "metric unavailable for files that failed parsing"
            }
            RawMetricCoverageStatus::Unavailable => {
                "Git churn collection was disabled or unavailable"
            }
        }
        .into(),
        unobservable_reasons: if status == RawMetricCoverageStatus::PartiallyObserved {
            vec![format!(
                "{} source files failed syntax parsing",
                failures.len()
            )]
        } else if status == RawMetricCoverageStatus::Unavailable {
            churn.reason.clone().into_iter().collect()
        } else {
            Vec::new()
        },
    }
}

fn detector_is_applicable(
    entry: &crate::model::DetectorManifestEntry,
    detected_languages: &BTreeSet<String>,
) -> bool {
    entry.supported_languages.iter().any(|language| {
        matches!(language.as_str(), "repository" | "language_neutral_paths")
            || detected_languages.contains(language)
    })
}

fn detected_language(path: &Path) -> Option<String> {
    const EXTENSION_LANGUAGES: &[(&str, &str)] = &[
        ("rs", "rust"),
        ("js", "javascript"),
        ("jsx", "javascript"),
        ("ts", "typescript"),
        ("tsx", "tsx"),
        ("py", "python"),
        ("go", "go"),
        ("java", "java"),
        ("cs", "csharp"),
        ("kt", "kotlin"),
        ("php", "php"),
        ("rb", "ruby"),
        ("c", "c"),
        ("h", "c"),
        ("cc", "cpp"),
        ("cpp", "cpp"),
        ("cxx", "cpp"),
        ("hh", "cpp"),
        ("hpp", "cpp"),
        ("hxx", "cpp"),
    ];
    let extension = path.extension()?.to_str()?;
    EXTENSION_LANGUAGES
        .iter()
        .find_map(|(candidate, language)| (*candidate == extension).then(|| (*language).into()))
}

fn run_scan_signals(
    root: &Path,
    args: &ScanArgs,
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
    signals.scan_agent_drift_signals();
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
    fn scan_dependency_graph_signals(&mut self) {
        self.progress.report(&format!(
            "Analyzing dependency graph in {} files",
            self.scan.structure_sources.len()
        ));
        let dependency_scan = scan_dependency_graph_report(&self.scan.structure_sources, self.root);
        self.scan.unresolved_dependency_edges = dependency_scan.unresolved_edges;
        self.scan.dependency_graph = dependency_scan.snapshot;
        self.scan.findings.extend(dependency_scan.findings);
    }

    fn scan_agent_drift_signals(&mut self) {
        self.progress.report(&format!(
            "Analyzing agent drift signals in {} files",
            self.scan.structure_sources.len()
        ));
        let options = AgentDriftOptions {
            min_repeated_occurrences: self.args.min_repeated_literal_occurrences,
            min_data_shape_occurrences: self.args.min_data_clump_occurrences,
            max_dir_files: self.args.max_dir_files,
            include_test_structure: self.args.include_test_structure,
        };
        self.scan
            .findings
            .extend(scan_agent_drift(&self.scan.structure_sources, &options));
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

fn resolve_scan_root(args: &ScanArgs) -> Result<PathBuf> {
    args.path
        .canonicalize()
        .with_context(|| format!("failed to resolve path {}", args.path.display()))
}

fn report_scan_start(
    progress: &mut dyn ProgressSink,
    root: &Path,
    total_source_files: Option<usize>,
) {
    match total_source_files {
        Some(total) => progress.report(&format!(
            "Scanning {} ({total} source {})",
            display_path(root),
            pluralize(total, "file")
        )),
        None => progress.report(&format!("Scanning {}", display_path(root))),
    }
}

fn scan_sources(
    source_plan: SourceScanPlan,
    args: &ScanArgs,
    total_source_files: Option<usize>,
    progress: &mut dyn ProgressSink,
    scan: &mut SourceScan,
) -> Result<()> {
    scan.stats.directories_scanned = source_plan.directories_scanned;
    for path in &source_plan.source_files {
        let file_options = FileScanOptions {
            max_file_lines: args.max_file_lines,
        };
        scan_file(path, file_options, scan)?;
        report_file_scan_progress(progress, &scan.stats, total_source_files, path);
    }

    scan_directories(
        &source_plan.directory_source_files,
        args.max_dir_files,
        &mut scan.raw_metrics,
        &mut scan.findings,
    );
    Ok(())
}

fn report_file_scan_progress(
    progress: &mut dyn ProgressSink,
    stats: &ScanStats,
    total_source_files: Option<usize>,
    path: &Path,
) {
    if let Some(total) = total_source_files {
        let detail = display_path(path);
        progress.report_scan_progress(ProgressEvent {
            completed: stats.source_files_scanned,
            total,
            detail: &detail,
        });
    }
}

struct ScanSimilarityProgress<'a> {
    progress: &'a mut dyn ProgressSink,
}

impl SimilarFunctionProgress for ScanSimilarityProgress<'_> {
    fn report_extract_progress(&mut self, completed: usize, total: usize, path: &str) {
        self.progress.report_analysis_progress(
            ProgressEvent {
                completed,
                total,
                detail: path,
            },
            "extracting candidates",
        );
    }

    fn report_compare_progress(&mut self, completed: usize, total: usize) {
        self.progress.report_analysis_progress(
            ProgressEvent {
                completed,
                total,
                detail: "",
            },
            "comparing candidates",
        );
    }
}

fn collect_source_scan_plan(root: &Path, args: &ScanArgs) -> Result<SourceScanPlan> {
    let mut plan = SourceScanPlan::default();

    if root.is_file() {
        if is_supported_source(root) && should_scan_source_file(root, args) {
            plan.source_files.push(root.to_path_buf());
        }
        return Ok(plan);
    }

    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(!args.filters.include_hidden)
        .git_ignore(!args.filters.no_gitignore)
        .git_global(!args.filters.no_gitignore)
        .git_exclude(!args.filters.no_gitignore)
        .require_git(false);

    let root_for_filter = root.to_path_buf();
    let args_for_filter = args.clone();
    for entry in builder
        .filter_entry(move |entry| should_visit_entry(entry, &root_for_filter, &args_for_filter))
        .build()
    {
        let entry = entry?;
        let Some(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            plan.directories_scanned += 1;
        } else if file_type.is_file()
            && is_supported_source(entry.path())
            && should_scan_source_file(entry.path(), args)
        {
            let path = entry.path().to_path_buf();
            count_source_file_parent(&path, &mut plan.directory_source_files);
            plan.source_files.push(path);
        }
    }

    Ok(plan)
}

fn scan_file(path: &Path, options: FileScanOptions, scan: &mut SourceScan) -> Result<()> {
    if !is_supported_source(path) {
        return Ok(());
    }

    scan.stats.source_files_scanned += 1;

    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read source file {}", path.display()))?;
    let source: Arc<str> = Arc::from(source);
    let line_count = source.lines().count();
    let display_path = display_path(path);
    let is_test = is_test_source(path);

    scan.raw_metrics.files.push(FileRawMetric {
        path: display_path.clone(),
        loc: line_count,
        imports: 0,
        public_items: 0,
        is_test,
        churn: ChurnFileMetric::default(),
    });

    if line_count > options.max_file_lines {
        scan.findings.push(finding(FindingInput::new(
            FindingKind::LargeFile,
            display_path.clone(),
            Some(1),
            format!("file has {line_count} lines; consider splitting responsibilities"),
            vec![FindingMetric::threshold(
                MetricId::FileLoc,
                line_count,
                options.max_file_lines,
                "lines",
            )],
        )));
    }

    for (index, line) in source.lines().enumerate() {
        if has_debt_marker(line) {
            scan.findings.push(finding(FindingInput::new(
                FindingKind::DebtMarker,
                display_path.clone(),
                Some(index + 1),
                "technical-debt marker found",
                Vec::new(),
            )));
        }
    }

    if is_supported_structure_source(path) {
        let source_file = SourceFile {
            path: path.to_path_buf(),
            display_path: display_path.clone(),
            source: Arc::clone(&source),
        };

        match parse_source_file(source_file.clone())? {
            Some(parsed) => scan.parsed_sources.push(parsed),
            None => scan.parse_failures.push(ParseFailure {
                path: display_path.clone(),
                language: detected_language(path).unwrap_or_else(|| "unknown".into()),
                reason: ParseFailureReason::SyntaxError,
            }),
        }

        scan.structure_sources.push(source_file);
    }

    Ok(())
}

fn count_source_file_parent(path: &Path, directory_source_files: &mut BTreeMap<PathBuf, usize>) {
    if let Some(parent) = path.parent() {
        *directory_source_files
            .entry(parent.to_path_buf())
            .or_insert(0) += 1;
    }
}

fn scan_directories(
    directory_source_files: &BTreeMap<PathBuf, usize>,
    max_dir_files: usize,
    raw_metrics: &mut RawMetrics,
    findings: &mut Vec<Finding>,
) {
    for (directory, file_count) in directory_source_files {
        raw_metrics.directories.push(DirectoryRawMetric {
            path: display_path(directory),
            source_files: *file_count,
        });
        if *file_count > max_dir_files {
            findings.push(finding(FindingInput::new(
                FindingKind::LargeDirectory,
                display_path(directory),
                None,
                format!(
                    "directory contains {file_count} source files; consider grouping related responsibilities"
                ),
                vec![FindingMetric::threshold(
                    MetricId::DirectorySourceFiles,
                    *file_count,
                    max_dir_files,
                    "source files",
                )],
            )));
        }
    }
}

fn has_debt_marker(line: &str) -> bool {
    let trimmed = line.trim_start();
    let is_comment = trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with("<!--");

    if !is_comment {
        return false;
    }

    let normalized = trimmed.to_ascii_lowercase();
    normalized.contains("todo") || normalized.contains("fixme")
}

fn is_supported_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(
            "c" | "cc"
                | "cpp"
                | "cs"
                | "go"
                | "java"
                | "js"
                | "jsx"
                | "kt"
                | "php"
                | "py"
                | "rb"
                | "rs"
                | "ts"
                | "tsx"
        )
    )
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| name.starts_with('.'))
}

fn is_default_excluded_dir(entry: &DirEntry) -> bool {
    entry
        .file_type()
        .is_some_and(|file_type| file_type.is_dir())
        && entry
            .file_name()
            .to_str()
            .is_some_and(|name| DEFAULT_EXCLUDED_DIRS.contains(&name))
}

fn should_visit_entry(entry: &DirEntry, root: &Path, args: &ScanArgs) -> bool {
    let is_root = entry.path() == root;
    is_root
        || ((args.filters.include_hidden || !is_hidden(entry))
            && (args.filters.include_generated || !is_default_excluded_dir(entry)))
            && !is_ignored_path(entry.path(), root, args)
            && !is_excluded_test_path(entry.path(), args)
}

fn is_ignored_path(path: &Path, root: &Path, args: &ScanArgs) -> bool {
    if args.filters.ignore_paths.is_empty() {
        return false;
    }

    let relative = path
        .strip_prefix(root)
        .ok()
        .map(display_path)
        .unwrap_or_else(|| display_path(path));
    args.filters.ignore_paths.iter().any(|ignore| {
        let ignore = ignore.replace('\\', "/").trim_matches('/').to_string();
        !ignore.is_empty()
            && (relative == ignore
                || relative
                    .strip_prefix(&ignore)
                    .is_some_and(|suffix| suffix.starts_with('/')))
    })
}

fn is_excluded_test_path(path: &Path, args: &ScanArgs) -> bool {
    args.filters.exclude_tests && is_test_source(path)
}

fn should_scan_source_file(path: &Path, args: &ScanArgs) -> bool {
    !is_excluded_test_path(path, args)
}

fn pluralize(count: usize, noun: &str) -> String {
    if count == 1 {
        noun.to_string()
    } else {
        format!("{noun}s")
    }
}

pub(crate) fn is_test_source(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    if file_name.starts_with("test_")
        || file_name.contains(".test.")
        || file_name.contains(".spec.")
        || file_name.ends_with("_test.go")
        || file_name.ends_with("_test.py")
        || file_name.ends_with("_test.rs")
        || file_name.ends_with("_tests.rs")
    {
        return true;
    }

    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| matches!(name, "test" | "tests" | "__tests__" | "spec" | "specs"))
    })
}

fn display_path(path: &Path) -> String {
    let display = path.to_string_lossy().replace('\\', "/");
    display
        .strip_prefix("//?/")
        .unwrap_or(display.as_str())
        .to_string()
}

#[cfg(test)]
#[path = "../scanner_tests.rs"]
mod scanner_tests;

#[cfg(test)]
#[path = "../scan_documentation_tests.rs"]
mod scan_documentation_tests;
