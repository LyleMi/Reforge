use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use ignore::{DirEntry, WalkBuilder};

use crate::agent_drift::{AgentDriftOptions, scan_agent_drift};
use crate::cli::ScanArgs;
use crate::detectors::dependency_graph::scan_dependency_graph_report;
use crate::detectors::manifest::{detector_manifest, raw_metric_manifest};
use crate::documentation::scan_documentation;
use crate::model::{
    ChurnFileMetric, DependencyGraphSnapshot, FileRawMetric, Finding, FindingKind, FindingMetric,
    FunctionRawMetric, RawMetrics, SCAN_REPORT_SCHEMA_VERSION, ScanReport, ScanStats, ScanSummary,
    SuppressionSummary, TypeRawMetric,
};
use crate::scoring::{
    FindingInput, StaticRiskThresholds, cluster_findings, finalize_scoring, finding, rank_hotspots,
    summarize_raw_metrics,
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
    directory_source_files: usize,
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
    finalize_scoring(&mut scan.findings, &scan.raw_metrics, &hotspots);
    let post_score_controls = apply_post_score_finding_controls(
        &mut scan,
        &root,
        &effective_args,
        &effective.suppressions,
    )?;
    let issue_clusters = cluster_findings(&mut scan.findings);
    let clustered_facets = issue_clusters
        .iter()
        .map(|cluster| cluster.finding_ids.len().saturating_sub(1))
        .sum::<usize>();

    let summary = ScanSummary {
        scanned_files: scan.stats.source_files_scanned,
        finding_count: scan.findings.len(),
        issue_count: scan.findings.len().saturating_sub(clustered_facets),
        hotspot_count: hotspots.len(),
        similar_function_group_count: post_score_controls.similar_function_group_count,
        duration_ms: started_at.elapsed().as_millis(),
        hotspot_model: effective_args
            .hotspot_model
            .expect("effective args should set hotspot model"),
        churn: churn_summary,
    };

    progress.report(&format!(
        "Finished scan: {} files, {} findings",
        summary.scanned_files, summary.finding_count
    ));
    progress.finish();

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
        issue_clusters,
        detector_manifest: detector_manifest(),
        findings: scan.findings,
    })
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
            directory_source_files: path
                .parent()
                .and_then(|parent| source_plan.directory_source_files.get(parent))
                .copied()
                .unwrap_or(0),
        };
        scan_file(path, file_options, scan)?;
        report_file_scan_progress(progress, &scan.stats, total_source_files, path);
    }

    scan_directories(
        &source_plan.directory_source_files,
        args.max_dir_files,
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
        directory_source_files: options.directory_source_files,
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
                "file_lines",
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

        if let Some(parsed) = parse_source_file(source_file.clone())? {
            scan.parsed_sources.push(parsed);
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
    findings: &mut Vec<Finding>,
) {
    for (directory, file_count) in directory_source_files {
        if *file_count > max_dir_files {
            findings.push(finding(FindingInput::new(
                FindingKind::LargeDirectory,
                display_path(directory),
                None,
                format!(
                    "directory contains {file_count} source files; consider grouping related responsibilities"
                ),
                vec![FindingMetric::threshold(
                    "directory_files",
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
