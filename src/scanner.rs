use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use serde::{Serialize, Serializer, ser::SerializeStruct};
use walkdir::{DirEntry, WalkDir};

use crate::agent_drift::{AgentDriftOptions, scan_agent_drift};
use crate::cli::ScanArgs;
use crate::similar_functions::{
    SimilarFunctionOptions, SimilarFunctionProgress, SourceFile, is_supported_similarity_source,
    scan_similar_functions_report_with_progress,
};
use crate::structural::{StructureOptions, is_supported_structure_source, scan_structure};

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
const SERIALIZED_SIMILAR_LOCATION_LIMIT: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    LargeFile,
    LargeDirectory,
    DebtMarker,
    SimilarFunctions,
    LongFunction,
    ComplexFunction,
    DeepNesting,
    ManyParameters,
    LargeType,
    LargePublicSurface,
    ImportHeavyFile,
    RepeatedLiteral,
    RepeatedErrorPattern,
    TestDuplication,
    HappyPathOnlyTests,
    FileNamingDrift,
    DirectoryDrift,
    DataClump,
    ParallelImplementation,
    ShadowedAbstraction,
    DuplicateTypeShape,
    ConfigKeyDrift,
    FixtureFactoryDrift,
    GenericBucketDrift,
    AdapterBoundaryBypass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RelatedLocation {
    pub path: String,
    pub line: usize,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub kind: FindingKind,
    pub severity: Severity,
    pub path: String,
    pub line: Option<usize>,
    pub magnitude: Option<usize>,
    pub message: String,
    pub related_locations: Vec<RelatedLocation>,
}

impl Serialize for Finding {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Finding", 7)?;
        state.serialize_field("kind", &self.kind)?;
        state.serialize_field("severity", &self.severity)?;
        state.serialize_field("path", &self.path)?;
        state.serialize_field("line", &self.line)?;
        state.serialize_field("magnitude", &self.magnitude)?;
        state.serialize_field("message", &self.message)?;
        state.serialize_field("related_locations", serialized_related_locations(self))?;
        state.end()
    }
}

fn serialized_related_locations(finding: &Finding) -> &[RelatedLocation] {
    if finding.kind == FindingKind::SimilarFunctions
        && finding.related_locations.len() > SERIALIZED_SIMILAR_LOCATION_LIMIT
    {
        &finding.related_locations[..SERIALIZED_SIMILAR_LOCATION_LIMIT]
    } else {
        &finding.related_locations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanSummary {
    pub scanned_files: usize,
    pub finding_count: usize,
    pub similar_function_group_count: usize,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ScanStats {
    pub source_files_scanned: usize,
    pub directories_scanned: usize,
    pub function_candidates: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanReport {
    pub summary: ScanSummary,
    pub stats: ScanStats,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Default)]
struct SourceScan {
    findings: Vec<Finding>,
    similarity_sources: Vec<SourceFile>,
    structure_sources: Vec<SourceFile>,
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
    include_test_similarity: bool,
}

pub(crate) fn severity_for_threshold(value: usize, threshold: usize) -> Severity {
    if threshold > 0 && value >= threshold.saturating_mul(2) {
        Severity::Critical
    } else {
        Severity::Warning
    }
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
    let mut scan = SourceScan::default();
    let source_plan = collect_source_scan_plan(&root, args)?;

    let total_source_files = progress
        .wants_detailed_progress()
        .then_some(source_plan.source_files.len());

    report_scan_start(progress, &root, total_source_files);
    scan_sources(source_plan, args, total_source_files, progress, &mut scan)?;
    let similar_function_group_count = {
        let mut signals = ScanSignalContext {
            args,
            progress,
            scan: &mut scan,
        };
        signals.scan_structural_signals()?;
        signals.scan_agent_drift_signals();
        signals.scan_similarity_signals()?
    };

    let summary = ScanSummary {
        scanned_files: scan.stats.source_files_scanned,
        finding_count: scan.findings.len(),
        similar_function_group_count,
        duration_ms: started_at.elapsed().as_millis(),
    };

    progress.report(&format!(
        "Finished scan: {} files, {} findings",
        summary.scanned_files, summary.finding_count
    ));
    progress.finish();

    Ok(ScanReport {
        summary,
        stats: scan.stats,
        findings: scan.findings,
    })
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
            self.scan.structure_sources.len()
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
        self.scan.findings.extend(scan_structure(
            &self.scan.structure_sources,
            &structure_options,
        )?);
        Ok(())
    }

    fn scan_similarity_signals(&mut self) -> Result<usize> {
        self.progress.report(&format!(
            "Analyzing similar functions in {} files",
            self.scan.similarity_sources.len()
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
        let similarity_scan = scan_similar_functions_report_with_progress(
            &self.scan.similarity_sources,
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
    let file_options = FileScanOptions {
        max_file_lines: args.max_file_lines,
        include_test_similarity: args.include_test_similarity,
    };

    scan.stats.directories_scanned = source_plan.directories_scanned;
    for path in &source_plan.source_files {
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

    if line_count > options.max_file_lines {
        scan.findings.push(Finding {
            kind: FindingKind::LargeFile,
            severity: severity_for_threshold(line_count, options.max_file_lines),
            path: display_path(path),
            line: Some(1),
            magnitude: Some(line_count),
            message: format!("file has {line_count} lines; consider splitting responsibilities"),
            related_locations: Vec::new(),
        });
    }

    for (index, line) in source.lines().enumerate() {
        if has_debt_marker(line) {
            scan.findings.push(Finding {
                kind: FindingKind::DebtMarker,
                severity: Severity::Info,
                path: display_path(path),
                line: Some(index + 1),
                magnitude: None,
                message: "technical-debt marker found".to_string(),
                related_locations: Vec::new(),
            });
        }
    }

    let display_path = display_path(path);
    if is_supported_structure_source(path) {
        scan.structure_sources.push(SourceFile {
            path: path.to_path_buf(),
            display_path: display_path.clone(),
            source: Arc::clone(&source),
        });
    }

    if is_supported_similarity_source(path)
        && (options.include_test_similarity || !is_test_source(path))
    {
        scan.similarity_sources.push(SourceFile {
            path: path.to_path_buf(),
            display_path,
            source,
        });
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
            findings.push(Finding {
                kind: FindingKind::LargeDirectory,
                severity: severity_for_threshold(*file_count, max_dir_files),
                path: display_path(directory),
                line: None,
                magnitude: Some(*file_count),
                message: format!(
                    "directory contains {file_count} source files; consider grouping related responsibilities"
                ),
                related_locations: Vec::new(),
            });
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
#[path = "scanner_tests.rs"]
mod tests;
