use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use serde::Deserialize;
use walkdir::{DirEntry, WalkDir};

use crate::agent_drift::{AgentDriftOptions, scan_agent_drift};
use crate::cli::{ChurnMode, HotspotModel, ScanArgs};
use crate::documentation::scan_documentation;
use crate::model::{
    ChurnFileMetric, ChurnSummary, FileRawMetric, Finding, FindingKind, FindingMetric,
    FunctionRawMetric, RawMetrics, SCAN_REPORT_SCHEMA_VERSION, ScanReport, ScanStats, ScanSummary,
    TypeRawMetric,
};
use crate::scoring::{
    FindingInput, finalize_scoring, finding, rank_hotspots, summarize_raw_metrics,
};
use crate::similar_functions::{
    ParsedSourceFile, SimilarFunctionOptions, SimilarFunctionProgress, SourceFile,
    parse_source_file, scan_parsed_similar_functions_report_with_progress,
};
use crate::structural::{
    StructureOptions, collect_raw_structure_metrics, is_supported_structure_source,
};

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
const PERCENT_SCALE: usize = 100;
const DEFAULT_MAX_FILE_LINES: usize = 800;
const DEFAULT_MAX_DIR_FILES: usize = 40;
const DEFAULT_MIN_SIMILAR_FUNCTIONS: usize = 3;
const DEFAULT_MIN_FUNCTION_TOKENS: usize = 80;
const DEFAULT_FUNCTION_SIMILARITY: f64 = 0.85;
const DEFAULT_MAX_FUNCTION_LINES: usize = 80;
const DEFAULT_MAX_FUNCTION_COMPLEXITY: usize = 15;
const DEFAULT_MAX_NESTING_DEPTH: usize = 4;
const DEFAULT_MAX_FUNCTION_PARAMETERS: usize = 5;
const DEFAULT_MAX_TYPE_LINES: usize = 250;
const DEFAULT_MAX_TYPE_MEMBERS: usize = 30;
const DEFAULT_MAX_IMPORTS: usize = 35;
const DEFAULT_MAX_PUBLIC_ITEMS: usize = 30;
const DEFAULT_MIN_REPEATED_LITERAL_OCCURRENCES: usize = 4;
const DEFAULT_MIN_DATA_CLUMP_OCCURRENCES: usize = 3;
const DEFAULT_CHURN_WINDOW_DAYS: usize = 180;
const DEFAULT_CHURN_MAX_COMMIT_LINES: usize = 2_000;

#[derive(Debug, Default)]
struct SourceScan {
    findings: Vec<Finding>,
    parsed_sources: Vec<ParsedSourceFile>,
    structure_sources: Vec<SourceFile>,
    raw_metrics: RawMetrics,
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

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProgressEvent<'a> {
    completed: usize,
    total: usize,
    detail: &'a str,
}

impl ProgressEvent<'_> {
    fn percent(self) -> Option<usize> {
        if self.total == 0 {
            return None;
        }

        Some(self.completed.saturating_mul(PERCENT_SCALE) / self.total)
    }

    fn detail_suffix(self) -> String {
        if self.detail.is_empty() {
            String::new()
        } else {
            format!(" {}", self.detail)
        }
    }

    fn is_complete(self) -> bool {
        self.completed == self.total
    }
}

pub(crate) trait ProgressSink {
    fn report(&mut self, message: &str);

    fn report_scan_progress(&mut self, event: ProgressEvent<'_>) {
        let Some(percent) = event.percent() else {
            return;
        };
        self.report(&format!(
            "[{percent:>3}%] Scanning source files ({}/{}) {}",
            event.completed, event.total, event.detail
        ));
    }

    fn report_analysis_progress(&mut self, event: ProgressEvent<'_>, phase: &str) {
        let Some(percent) = event.percent() else {
            return;
        };
        let detail = event.detail_suffix();
        self.report(&format!(
            "[{percent:>3}%] Analyzing similar functions: {phase} ({}/{}){detail}",
            event.completed, event.total
        ));
    }

    fn wants_detailed_progress(&self) -> bool {
        false
    }

    fn finish(&mut self) {}
}

pub struct NoopProgress;

impl ProgressSink for NoopProgress {
    fn report(&mut self, _message: &str) {}
}

pub struct StderrProgress {
    dynamic: bool,
    last_dynamic_len: usize,
    last_bucket: Option<usize>,
}

impl StderrProgress {
    pub fn new(dynamic: bool) -> Self {
        Self {
            dynamic,
            last_dynamic_len: 0,
            last_bucket: None,
        }
    }

    fn finish_dynamic_line(&mut self) {
        if self.last_dynamic_len > 0 {
            let _ = writeln!(std::io::stderr());
            self.last_dynamic_len = 0;
        }
    }

    fn report_percent_progress(&mut self, message: &str, event: ProgressEvent<'_>) {
        if self.dynamic {
            let padding = self.last_dynamic_len.saturating_sub(message.len());
            let mut stderr = std::io::stderr().lock();
            let _ = write!(stderr, "\r{message}{}", " ".repeat(padding));
            let _ = stderr.flush();
            self.last_dynamic_len = message.len();
        } else {
            let Some(percent) = event.percent() else {
                return;
            };
            let bucket = percent / 10;
            if self.last_bucket != Some(bucket) || event.is_complete() {
                self.report(message);
                self.last_bucket = Some(bucket);
            }
        }
    }
}

impl ProgressSink for StderrProgress {
    fn report(&mut self, message: &str) {
        self.finish_dynamic_line();
        let _ = writeln!(std::io::stderr(), "{message}");
    }

    fn report_scan_progress(&mut self, event: ProgressEvent<'_>) {
        let Some(percent) = event.percent() else {
            return;
        };
        let message = format!(
            "[{percent:>3}%] Scanning source files ({}/{}) {}",
            event.completed, event.total, event.detail
        );

        self.report_percent_progress(&message, event);
    }

    fn report_analysis_progress(&mut self, event: ProgressEvent<'_>, phase: &str) {
        let Some(percent) = event.percent() else {
            return;
        };
        let detail = event.detail_suffix();
        let message = format!(
            "[{percent:>3}%] Analyzing similar functions: {phase} ({}/{}){detail}",
            event.completed, event.total
        );

        self.report_percent_progress(&message, event);
    }

    fn wants_detailed_progress(&self) -> bool {
        true
    }

    fn finish(&mut self) {
        self.finish_dynamic_line();
    }
}

#[cfg(test)]
pub struct WriterProgress<W: Write> {
    writer: W,
}

#[cfg(test)]
impl<W: Write> WriterProgress<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

#[cfg(test)]
impl<W: Write> ProgressSink for WriterProgress<W> {
    fn report(&mut self, message: &str) {
        let _ = writeln!(self.writer, "{message}");
    }

    fn wants_detailed_progress(&self) -> bool {
        true
    }
}

#[allow(dead_code)]
pub(crate) fn scan_path(args: &ScanArgs) -> Result<Vec<Finding>> {
    let mut progress = NoopProgress;
    Ok(scan_report(args, &mut progress)?.findings)
}

pub(crate) fn scan_report(args: &ScanArgs, progress: &mut dyn ProgressSink) -> Result<ScanReport> {
    let started_at = Instant::now();
    let root = resolve_scan_root(args)?;
    let effective_args = effective_scan_args(args, &root)?;
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
    let similar_function_group_count = {
        let mut signals = ScanSignalContext {
            args: &effective_args,
            progress,
            scan: &mut scan,
        };
        signals.scan_structural_signals()?;
        signals.scan_agent_drift_signals();
        signals.scan_similarity_signals()?
    };
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
    );
    finalize_scoring(&mut scan.findings, &scan.raw_metrics, &hotspots);

    let summary = ScanSummary {
        scanned_files: scan.stats.source_files_scanned,
        finding_count: scan.findings.len(),
        hotspot_count: hotspots.len(),
        similar_function_group_count,
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
        hotspots,
        findings: scan.findings,
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct ReforgeConfig {
    max_file_lines: Option<usize>,
    max_dir_files: Option<usize>,
    min_similar_functions: Option<usize>,
    min_function_tokens: Option<usize>,
    function_similarity: Option<f64>,
    max_function_lines: Option<usize>,
    max_function_complexity: Option<usize>,
    max_nesting_depth: Option<usize>,
    max_function_parameters: Option<usize>,
    max_type_lines: Option<usize>,
    max_type_members: Option<usize>,
    max_imports: Option<usize>,
    max_public_items: Option<usize>,
    min_repeated_literal_occurrences: Option<usize>,
    min_data_clump_occurrences: Option<usize>,
    churn: Option<ChurnMode>,
    hotspot_model: Option<HotspotModel>,
    churn_window_days: Option<usize>,
    churn_max_commit_lines: Option<usize>,
    ignore_paths: Vec<String>,
}

fn effective_scan_args(args: &ScanArgs, root: &Path) -> Result<ScanArgs> {
    let mut effective = args.clone();
    let config = load_config(args, root)?;

    if let Some(config) = config {
        apply_config_defaults(&mut effective, &config);
    }

    effective.churn = Some(
        args.churn
            .unwrap_or(effective.churn.unwrap_or(ChurnMode::Auto)),
    );
    effective.hotspot_model = Some(
        args.hotspot_model
            .unwrap_or(effective.hotspot_model.unwrap_or(HotspotModel::Hybrid)),
    );
    effective.churn_window_days = Some(
        args.churn_window_days.unwrap_or(
            effective
                .churn_window_days
                .unwrap_or(DEFAULT_CHURN_WINDOW_DAYS),
        ),
    );
    effective.churn_max_commit_lines = Some(
        args.churn_max_commit_lines.unwrap_or(
            effective
                .churn_max_commit_lines
                .unwrap_or(DEFAULT_CHURN_MAX_COMMIT_LINES),
        ),
    );

    Ok(effective)
}

fn load_config(args: &ScanArgs, root: &Path) -> Result<Option<ReforgeConfig>> {
    let config_path = if let Some(path) = &args.config {
        Some(path.clone())
    } else {
        discover_config_path(root)
    };

    let Some(path) = config_path else {
        return Ok(None);
    };

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;
    let config = toml::from_str(&contents)
        .with_context(|| format!("failed to parse config file {}", path.display()))?;
    Ok(Some(config))
}

fn discover_config_path(root: &Path) -> Option<PathBuf> {
    let mut current = if root.is_file() {
        root.parent()?.to_path_buf()
    } else {
        root.to_path_buf()
    };

    loop {
        let candidate = current.join("reforge.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn apply_config_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    apply_file_config_defaults(args, config);
    apply_similarity_config_defaults(args, config);
    apply_structure_config_defaults(args, config);
    apply_repetition_config_defaults(args, config);
    apply_churn_config_defaults(args, config);
    apply_ignore_path_defaults(args, config);
}

fn apply_file_config_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    apply_usize_default(
        &mut args.max_file_lines,
        DEFAULT_MAX_FILE_LINES,
        config.max_file_lines,
    );
    apply_usize_default(
        &mut args.max_dir_files,
        DEFAULT_MAX_DIR_FILES,
        config.max_dir_files,
    );
}

fn apply_similarity_config_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    apply_usize_default(
        &mut args.min_similar_functions,
        DEFAULT_MIN_SIMILAR_FUNCTIONS,
        config.min_similar_functions,
    );
    apply_usize_default(
        &mut args.min_function_tokens,
        DEFAULT_MIN_FUNCTION_TOKENS,
        config.min_function_tokens,
    );
    if (args.function_similarity - DEFAULT_FUNCTION_SIMILARITY).abs() < f64::EPSILON
        && let Some(value) = config.function_similarity
    {
        args.function_similarity = value;
    }
}

fn apply_structure_config_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    apply_usize_default(
        &mut args.max_function_lines,
        DEFAULT_MAX_FUNCTION_LINES,
        config.max_function_lines,
    );
    apply_usize_default(
        &mut args.max_function_complexity,
        DEFAULT_MAX_FUNCTION_COMPLEXITY,
        config.max_function_complexity,
    );
    apply_usize_default(
        &mut args.max_nesting_depth,
        DEFAULT_MAX_NESTING_DEPTH,
        config.max_nesting_depth,
    );
    apply_usize_default(
        &mut args.max_function_parameters,
        DEFAULT_MAX_FUNCTION_PARAMETERS,
        config.max_function_parameters,
    );
    apply_usize_default(
        &mut args.max_type_lines,
        DEFAULT_MAX_TYPE_LINES,
        config.max_type_lines,
    );
    apply_usize_default(
        &mut args.max_type_members,
        DEFAULT_MAX_TYPE_MEMBERS,
        config.max_type_members,
    );
    apply_usize_default(
        &mut args.max_imports,
        DEFAULT_MAX_IMPORTS,
        config.max_imports,
    );
    apply_usize_default(
        &mut args.max_public_items,
        DEFAULT_MAX_PUBLIC_ITEMS,
        config.max_public_items,
    );
}

fn apply_repetition_config_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    apply_usize_default(
        &mut args.min_repeated_literal_occurrences,
        DEFAULT_MIN_REPEATED_LITERAL_OCCURRENCES,
        config.min_repeated_literal_occurrences,
    );
    apply_usize_default(
        &mut args.min_data_clump_occurrences,
        DEFAULT_MIN_DATA_CLUMP_OCCURRENCES,
        config.min_data_clump_occurrences,
    );
}

fn apply_churn_config_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    args.churn = args.churn.or(config.churn);
    args.hotspot_model = args.hotspot_model.or(config.hotspot_model);
    args.churn_window_days = args.churn_window_days.or(config.churn_window_days);
    args.churn_max_commit_lines = args
        .churn_max_commit_lines
        .or(config.churn_max_commit_lines);
}

fn apply_ignore_path_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    if args.ignore_paths.is_empty() {
        args.ignore_paths = config.ignore_paths.clone();
    }
}

fn apply_usize_default(target: &mut usize, default: usize, configured: Option<usize>) {
    if *target == default
        && let Some(value) = configured
    {
        *target = value;
    }
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

fn collect_churn_metrics(
    root: &Path,
    args: &ScanArgs,
    raw_metrics: &mut RawMetrics,
) -> Result<ChurnSummary> {
    let mode = args.churn.expect("effective args should set churn mode");
    let window_days = args
        .churn_window_days
        .expect("effective args should set churn window");
    let max_commit_lines = args
        .churn_max_commit_lines
        .expect("effective args should set churn max commit lines");

    if mode == ChurnMode::Off {
        return Ok(churn_summary(ChurnSummaryInput {
            mode,
            enabled: false,
            status: "disabled",
            reason: Some("churn collection disabled by configuration".to_string()),
            window_days,
            max_commit_lines,
        }));
    }

    match collect_git_churn(root, window_days, max_commit_lines) {
        Ok(churn_by_path) => {
            for file_metric in &mut raw_metrics.files {
                if let Some(churn) = churn_by_path.get(&file_metric.path) {
                    file_metric.churn = churn.clone();
                }
            }

            Ok(churn_summary(ChurnSummaryInput {
                mode,
                enabled: true,
                status: "enabled",
                reason: None,
                window_days,
                max_commit_lines,
            }))
        }
        Err(error) if mode == ChurnMode::Auto => Ok(churn_summary(ChurnSummaryInput {
            mode,
            enabled: false,
            status: "unavailable",
            reason: Some(error.to_string()),
            window_days,
            max_commit_lines,
        })),
        Err(error) => Err(error),
    }
}

struct ChurnSummaryInput {
    mode: ChurnMode,
    enabled: bool,
    status: &'static str,
    reason: Option<String>,
    window_days: usize,
    max_commit_lines: usize,
}

fn churn_summary(input: ChurnSummaryInput) -> ChurnSummary {
    ChurnSummary {
        mode: input.mode,
        enabled: input.enabled,
        status: input.status.to_string(),
        reason: input.reason,
        window_days: input.window_days,
        max_commit_lines: input.max_commit_lines,
    }
}

fn collect_git_churn(
    root: &Path,
    window_days: usize,
    max_commit_lines: usize,
) -> Result<BTreeMap<String, ChurnFileMetric>> {
    let command_root = if root.is_file() {
        root.parent().unwrap_or(root)
    } else {
        root
    };
    let git_root_output = Command::new("git")
        .arg("-C")
        .arg(command_root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("failed to run git rev-parse")?;
    if !git_root_output.status.success() {
        anyhow::bail!("scan root is not inside a git repository");
    }

    let git_root_text = String::from_utf8_lossy(&git_root_output.stdout);
    let git_root = PathBuf::from(git_root_text.trim());
    let scan_relative = root
        .strip_prefix(&git_root)
        .ok()
        .map(path_to_git_slash)
        .unwrap_or_default();
    let since = format!("{window_days} days ago");
    let log_output = Command::new("git")
        .arg("-C")
        .arg(&git_root)
        .args([
            "log",
            "--no-merges",
            &format!("--since={since}"),
            "--numstat",
            "--format=commit:%H%x09%an",
        ])
        .output()
        .context("failed to run git log")?;
    if !log_output.status.success() {
        anyhow::bail!("failed to collect git churn");
    }

    let churn_by_relative_path = parse_git_numstat_churn(
        &String::from_utf8_lossy(&log_output.stdout),
        &scan_relative,
        max_commit_lines,
    );
    Ok(churn_by_relative_path
        .into_iter()
        .map(|(path, churn)| (display_path(&git_root.join(path)), churn))
        .collect())
}

#[derive(Debug, Clone)]
struct PendingCommitChurn {
    author: String,
    files: Vec<(String, usize, usize)>,
    total_lines: usize,
}

fn parse_git_numstat_churn(
    output: &str,
    scan_relative: &str,
    max_commit_lines: usize,
) -> BTreeMap<String, ChurnFileMetric> {
    let mut churn_by_path = BTreeMap::<String, ChurnFileMetric>::new();
    let mut authors_by_path = BTreeMap::<String, BTreeSet<String>>::new();
    let mut pending: Option<PendingCommitChurn> = None;

    for line in output.lines() {
        if let Some(header) = line.strip_prefix("commit:") {
            flush_pending_commit(
                &mut churn_by_path,
                &mut authors_by_path,
                pending.take(),
                max_commit_lines,
            );
            let author = header
                .split_once('\t')
                .map(|(_, author)| author)
                .unwrap_or_default()
                .to_string();
            pending = Some(PendingCommitChurn {
                author,
                files: Vec::new(),
                total_lines: 0,
            });
            continue;
        }

        let Some(commit) = pending.as_mut() else {
            continue;
        };
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() < 3 || fields[0] == "-" || fields[1] == "-" {
            continue;
        }
        let Ok(added) = fields[0].parse::<usize>() else {
            continue;
        };
        let Ok(deleted) = fields[1].parse::<usize>() else {
            continue;
        };
        let path = normalize_git_numstat_path(fields[2]);
        if !path_in_scan_root(&path, scan_relative) {
            continue;
        }
        commit.total_lines += added + deleted;
        commit.files.push((path, added, deleted));
    }

    flush_pending_commit(
        &mut churn_by_path,
        &mut authors_by_path,
        pending,
        max_commit_lines,
    );
    for (path, authors) in authors_by_path {
        if let Some(metric) = churn_by_path.get_mut(&path) {
            metric.authors_count = authors.len();
        }
    }
    churn_by_path
}

fn flush_pending_commit(
    churn_by_path: &mut BTreeMap<String, ChurnFileMetric>,
    authors_by_path: &mut BTreeMap<String, BTreeSet<String>>,
    pending: Option<PendingCommitChurn>,
    max_commit_lines: usize,
) {
    let Some(pending) = pending else {
        return;
    };
    if pending.total_lines > max_commit_lines {
        return;
    }

    for (path, added, deleted) in pending.files {
        let metric = churn_by_path.entry(path.clone()).or_default();
        metric.commits_touched += 1;
        metric.lines_added += added;
        metric.lines_deleted += deleted;
        metric.recent_weighted_churn += added + deleted;
        if !pending.author.is_empty() {
            authors_by_path
                .entry(path)
                .or_default()
                .insert(pending.author.clone());
        }
    }
}

fn normalize_git_numstat_path(path: &str) -> String {
    let path = path
        .rsplit_once(" => ")
        .map(|(_, new_path)| new_path)
        .unwrap_or(path);
    path.trim_matches(['{', '}']).replace('\\', "/")
}

fn path_in_scan_root(path: &str, scan_relative: &str) -> bool {
    scan_relative.is_empty()
        || path == scan_relative
        || path
            .strip_prefix(scan_relative)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn path_to_git_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

struct ScanSignalContext<'a> {
    args: &'a ScanArgs,
    progress: &'a mut dyn ProgressSink,
    scan: &'a mut SourceScan,
}

impl ScanSignalContext<'_> {
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
        if is_supported_source(root) {
            plan.source_files.push(root.to_path_buf());
        }
        return Ok(plan);
    }

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| should_visit_entry(entry, root, args))
    {
        let entry = entry?;
        if entry.file_type().is_dir() {
            plan.directories_scanned += 1;
        } else if entry.file_type().is_file() && is_supported_source(entry.path()) {
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
    entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .is_some_and(|name| DEFAULT_EXCLUDED_DIRS.contains(&name))
}

fn should_visit_entry(entry: &DirEntry, root: &Path, args: &ScanArgs) -> bool {
    let is_root = entry.path() == root;
    is_root
        || ((args.include_hidden || !is_hidden(entry))
            && (args.include_generated || !is_default_excluded_dir(entry)))
            && !is_ignored_path(entry.path(), root, args)
}

fn is_ignored_path(path: &Path, root: &Path, args: &ScanArgs) -> bool {
    if args.ignore_paths.is_empty() {
        return false;
    }

    let relative = path
        .strip_prefix(root)
        .ok()
        .map(display_path)
        .unwrap_or_else(|| display_path(path));
    args.ignore_paths.iter().any(|ignore| {
        let ignore = ignore.replace('\\', "/").trim_matches('/').to_string();
        !ignore.is_empty()
            && (relative == ignore
                || relative
                    .strip_prefix(&ignore)
                    .is_some_and(|suffix| suffix.starts_with('/')))
    })
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
mod tests;
