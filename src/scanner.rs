use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use serde::Serialize;
use walkdir::{DirEntry, WalkDir};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    DirectoryDrift,
    DataClump,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RelatedLocation {
    pub path: String,
    pub line: usize,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub kind: FindingKind,
    pub severity: Severity,
    pub path: String,
    pub line: Option<usize>,
    pub magnitude: Option<usize>,
    pub message: String,
    pub related_locations: Vec<RelatedLocation>,
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

pub trait ProgressSink {
    fn report(&mut self, message: &str);

    fn report_scan_progress(&mut self, completed: usize, total: usize, path: &str) {
        if total == 0 {
            return;
        }

        let percent = completed.saturating_mul(100) / total;
        self.report(&format!(
            "[{percent:>3}%] Scanning source files ({completed}/{total}) {path}"
        ));
    }

    fn report_analysis_progress(
        &mut self,
        completed: usize,
        total: usize,
        phase: &str,
        detail: &str,
    ) {
        if total == 0 {
            return;
        }

        let percent = completed.saturating_mul(100) / total;
        let detail = if detail.is_empty() {
            String::new()
        } else {
            format!(" {detail}")
        };
        self.report(&format!(
            "[{percent:>3}%] Analyzing similar functions: {phase} ({completed}/{total}){detail}"
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

    fn report_percent_progress(
        &mut self,
        message: &str,
        percent: usize,
        completed: usize,
        total: usize,
    ) {
        if self.dynamic {
            let padding = self.last_dynamic_len.saturating_sub(message.len());
            let mut stderr = std::io::stderr().lock();
            let _ = write!(stderr, "\r{message}{}", " ".repeat(padding));
            let _ = stderr.flush();
            self.last_dynamic_len = message.len();
        } else {
            let bucket = percent / 10;
            if self.last_bucket != Some(bucket) || completed == total {
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

    fn report_scan_progress(&mut self, completed: usize, total: usize, path: &str) {
        if total == 0 {
            return;
        }

        let percent = completed.saturating_mul(100) / total;
        let message = format!("[{percent:>3}%] Scanning source files ({completed}/{total}) {path}");

        self.report_percent_progress(&message, percent, completed, total);
    }

    fn report_analysis_progress(
        &mut self,
        completed: usize,
        total: usize,
        phase: &str,
        detail: &str,
    ) {
        if total == 0 {
            return;
        }

        let percent = completed.saturating_mul(100) / total;
        let detail = if detail.is_empty() {
            String::new()
        } else {
            format!(" {detail}")
        };
        let message = format!(
            "[{percent:>3}%] Analyzing similar functions: {phase} ({completed}/{total}){detail}"
        );

        self.report_percent_progress(&message, percent, completed, total);
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
pub fn scan_path(args: &ScanArgs) -> Result<Vec<Finding>> {
    let mut progress = NoopProgress;
    Ok(scan_report(args, &mut progress)?.findings)
}

pub fn scan_report(args: &ScanArgs, progress: &mut dyn ProgressSink) -> Result<ScanReport> {
    let started_at = Instant::now();
    let root = args
        .path
        .canonicalize()
        .with_context(|| format!("failed to resolve path {}", args.path.display()))?;

    let mut findings = Vec::new();
    let mut similarity_sources = Vec::new();
    let mut structure_sources = Vec::new();
    let mut stats = ScanStats::default();

    let total_source_files = if progress.wants_detailed_progress() {
        Some(count_source_files(&root, args)?)
    } else {
        None
    };

    match total_source_files {
        Some(total) => progress.report(&format!(
            "Scanning {} ({total} source {})",
            display_path(&root),
            pluralize(total, "file")
        )),
        None => progress.report(&format!("Scanning {}", display_path(&root))),
    }

    if root.is_file() {
        scan_file(
            &root,
            args.max_file_lines,
            args.include_test_similarity,
            &mut findings,
            &mut similarity_sources,
            &mut structure_sources,
            &mut stats,
        )?;
        if let Some(total) = total_source_files {
            progress.report_scan_progress(stats.source_files_scanned, total, &display_path(&root));
        }
    } else {
        let mut directory_source_files = BTreeMap::new();

        for entry in WalkDir::new(&root)
            .into_iter()
            .filter_entry(|entry| should_visit_entry(entry, &root, args))
        {
            let entry = entry?;

            if entry.file_type().is_dir() {
                stats.directories_scanned += 1;
            }

            if entry.file_type().is_file() && is_supported_source(entry.path()) {
                scan_file(
                    entry.path(),
                    args.max_file_lines,
                    args.include_test_similarity,
                    &mut findings,
                    &mut similarity_sources,
                    &mut structure_sources,
                    &mut stats,
                )?;
                if let Some(total) = total_source_files {
                    progress.report_scan_progress(
                        stats.source_files_scanned,
                        total,
                        &display_path(entry.path()),
                    );
                }
                count_source_file_parent(entry.path(), &mut directory_source_files);
            }
        }

        scan_directories(&directory_source_files, args.max_dir_files, &mut findings);
    }

    progress.report(&format!(
        "Analyzing structural signals in {} files",
        structure_sources.len()
    ));
    let structure_options = StructureOptions {
        max_function_lines: args.max_function_lines,
        max_function_complexity: args.max_function_complexity,
        max_nesting_depth: args.max_nesting_depth,
        max_function_parameters: args.max_function_parameters,
        max_type_lines: args.max_type_lines,
        max_type_members: args.max_type_members,
        max_imports: args.max_imports,
        max_public_items: args.max_public_items,
        min_repeated_literal_occurrences: args.min_repeated_literal_occurrences,
        min_data_clump_occurrences: args.min_data_clump_occurrences,
        max_dir_files: args.max_dir_files,
        include_test_structure: args.include_test_structure,
    };
    findings.extend(scan_structure(&structure_sources, &structure_options)?);

    progress.report(&format!(
        "Analyzing similar functions in {} files",
        similarity_sources.len()
    ));

    let similarity_options = SimilarFunctionOptions {
        min_group_size: args.min_similar_functions,
        min_tokens: args.min_function_tokens,
        threshold: args.function_similarity,
    };
    let mut similarity_progress = ScanSimilarityProgress { progress };
    let similarity_scan = scan_similar_functions_report_with_progress(
        &similarity_sources,
        &similarity_options,
        &mut similarity_progress,
    )?;
    stats.function_candidates = similarity_scan.candidate_count;
    findings.extend(similarity_scan.findings);

    let similar_function_group_count = findings
        .iter()
        .filter(|finding| finding.kind == FindingKind::SimilarFunctions)
        .count();
    let summary = ScanSummary {
        scanned_files: stats.source_files_scanned,
        finding_count: findings.len(),
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
        stats,
        findings,
    })
}

struct ScanSimilarityProgress<'a> {
    progress: &'a mut dyn ProgressSink,
}

impl SimilarFunctionProgress for ScanSimilarityProgress<'_> {
    fn report_extract_progress(&mut self, completed: usize, total: usize, path: &str) {
        self.progress
            .report_analysis_progress(completed, total, "extracting candidates", path);
    }

    fn report_compare_progress(&mut self, completed: usize, total: usize) {
        self.progress
            .report_analysis_progress(completed, total, "comparing candidates", "");
    }
}

fn count_source_files(root: &Path, args: &ScanArgs) -> Result<usize> {
    if root.is_file() {
        return Ok(usize::from(is_supported_source(root)));
    }

    let mut count = 0;
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| should_visit_entry(entry, root, args))
    {
        let entry = entry?;
        if entry.file_type().is_file() && is_supported_source(entry.path()) {
            count += 1;
        }
    }

    Ok(count)
}

fn scan_file(
    path: &Path,
    max_file_lines: usize,
    include_test_similarity: bool,
    findings: &mut Vec<Finding>,
    similarity_sources: &mut Vec<SourceFile>,
    structure_sources: &mut Vec<SourceFile>,
    stats: &mut ScanStats,
) -> Result<()> {
    if !is_supported_source(path) {
        return Ok(());
    }

    stats.source_files_scanned += 1;

    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read source file {}", path.display()))?;
    let line_count = source.lines().count();

    if line_count > max_file_lines {
        findings.push(Finding {
            kind: FindingKind::LargeFile,
            severity: Severity::Warning,
            path: display_path(path),
            line: Some(1),
            magnitude: Some(line_count),
            message: format!("file has {line_count} lines; consider splitting responsibilities"),
            related_locations: Vec::new(),
        });
    }

    for (index, line) in source.lines().enumerate() {
        if has_debt_marker(line) {
            findings.push(Finding {
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
        structure_sources.push(SourceFile {
            path: path.to_path_buf(),
            display_path: display_path.clone(),
            source: source.clone(),
        });
    }

    if is_supported_similarity_source(path) && (include_test_similarity || !is_test_source(path)) {
        similarity_sources.push(SourceFile {
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
                severity: Severity::Warning,
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
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn test_root(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("reforge-{name}-{suffix}"))
    }

    fn scan_args(path: std::path::PathBuf, include_generated: bool) -> ScanArgs {
        ScanArgs {
            path,
            max_file_lines: 800,
            max_dir_files: 40,
            include_hidden: false,
            include_generated,
            min_similar_functions: 3,
            min_function_tokens: 80,
            function_similarity: 0.85,
            include_test_similarity: false,
            max_function_lines: 80,
            max_function_complexity: 15,
            max_nesting_depth: 4,
            max_function_parameters: 5,
            max_type_lines: 250,
            max_type_members: 30,
            max_imports: 35,
            max_public_items: 30,
            min_repeated_literal_occurrences: 4,
            min_data_clump_occurrences: 3,
            include_test_structure: false,
            output: Some(crate::cli::OutputFormat::Human),
            output_file: None,
            progress: crate::cli::ProgressMode::Auto,
            color: crate::cli::ColorMode::Auto,
        }
    }

    #[test]
    fn skips_generated_directories_by_default() -> Result<()> {
        let root = test_root("skip-generated");
        fs::create_dir_all(root.join("node_modules/pkg"))?;
        fs::create_dir_all(root.join("src"))?;
        fs::write(root.join("node_modules/pkg/index.js"), "// TODO: ignored\n")?;
        fs::write(root.join("src/main.rs"), "// TODO: reported\n")?;

        let findings = scan_path(&scan_args(root.clone(), false))?;

        fs::remove_dir_all(root)?;

        assert_eq!(findings.len(), 1);
        assert!(findings[0].path.ends_with("src/main.rs"));
        Ok(())
    }

    #[test]
    fn can_include_generated_directories() -> Result<()> {
        let root = test_root("include-generated");
        fs::create_dir_all(root.join("dist"))?;
        fs::write(root.join("dist/app.js"), "// TODO: reported\n")?;

        let findings = scan_path(&scan_args(root.clone(), true))?;

        fs::remove_dir_all(root)?;

        assert_eq!(findings.len(), 1);
        assert!(findings[0].path.ends_with("dist/app.js"));
        Ok(())
    }

    #[test]
    fn reports_directories_with_many_source_files() -> Result<()> {
        let root = test_root("large-directory");
        let source_dir = root.join("src");
        fs::create_dir_all(&source_dir)?;
        fs::write(source_dir.join("one.rs"), "fn one() {}\n")?;
        fs::write(source_dir.join("two.rs"), "fn two() {}\n")?;
        fs::write(source_dir.join("three.rs"), "fn three() {}\n")?;
        fs::write(source_dir.join("notes.md"), "not source\n")?;

        let mut args = scan_args(root.clone(), false);
        args.max_dir_files = 2;
        let findings = scan_path(&args)?;

        fs::remove_dir_all(root)?;

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, FindingKind::LargeDirectory);
        assert!(findings[0].path.ends_with("src"));
        assert_eq!(findings[0].line, None);
        assert_eq!(findings[0].magnitude, Some(3));
        assert!(
            findings[0]
                .message
                .contains("directory contains 3 source files")
        );
        Ok(())
    }

    #[test]
    fn excludes_generated_directories_from_source_file_counts_by_default() -> Result<()> {
        let root = test_root("directory-count-generated");
        let dist_dir = root.join("dist");
        fs::create_dir_all(&dist_dir)?;
        fs::write(dist_dir.join("one.js"), "const one = 1;\n")?;
        fs::write(dist_dir.join("two.js"), "const two = 2;\n")?;

        let mut args = scan_args(root.clone(), false);
        args.max_dir_files = 1;
        let findings = scan_path(&args)?;

        fs::remove_dir_all(root)?;

        assert!(findings.is_empty());
        Ok(())
    }

    #[test]
    fn reports_similar_functions_using_scan_thresholds() -> Result<()> {
        let root = test_root("similar-functions");
        fs::create_dir_all(root.join("src"))?;
        fs::write(
            root.join("src/app.js"),
            r#"
function alpha(items) {
  let total = 0;
  for (const item of items) {
    if (item.score > 10) {
      total += item.score * 2;
    } else {
      total += item.score;
    }
  }
  return total;
}

function beta(records) {
  let sum = 1;
  for (const record of records) {
    if (record.score > 20) {
      sum += record.score * 2;
    } else {
      sum += record.score;
    }
  }
  return sum;
}

function gamma(rows) {
  let acc = 2;
  for (const row of rows) {
    if (row.score > 30) {
      acc += row.score * 2;
    } else {
      acc += row.score;
    }
  }
  return acc;
}
"#,
        )?;

        let mut args = scan_args(root.clone(), false);
        args.min_function_tokens = 12;
        let findings = scan_path(&args)?;

        args.min_similar_functions = 4;
        let stricter_findings = scan_path(&args)?;

        fs::remove_dir_all(root)?;

        let similar_findings = findings
            .iter()
            .filter(|finding| finding.kind == FindingKind::SimilarFunctions)
            .collect::<Vec<_>>();
        assert_eq!(similar_findings.len(), 1);
        assert_eq!(similar_findings[0].magnitude, Some(3));
        assert!(
            stricter_findings
                .iter()
                .all(|finding| !finding.message.contains("structurally similar"))
        );
        Ok(())
    }

    #[test]
    fn excludes_test_files_from_similarity_by_default() -> Result<()> {
        let root = test_root("exclude-test-similarity");
        fs::create_dir_all(root.join("src"))?;
        fs::write(
            root.join("src/app_test.go"),
            r#"
package app

func Alpha(items []Item) int {
    total := 0
    for _, item := range items {
        if item.Score > 10 {
            total += item.Score * 2
        } else {
            total += item.Score
        }
    }
    return total
}

func Beta(records []Item) int {
    sum := 1
    for _, record := range records {
        if record.Score > 20 {
            sum += record.Score * 2
        } else {
            sum += record.Score
        }
    }
    return sum
}

func Gamma(rows []Item) int {
    acc := 2
    for _, row := range rows {
        if row.Score > 30 {
            acc += row.Score * 2
        } else {
            acc += row.Score
        }
    }
    return acc
}
"#,
        )?;

        let mut args = scan_args(root.clone(), false);
        args.min_function_tokens = 12;
        let default_findings = scan_path(&args)?;

        args.include_test_similarity = true;
        let included_findings = scan_path(&args)?;

        fs::remove_dir_all(root)?;

        assert!(
            default_findings
                .iter()
                .all(|finding| finding.kind != FindingKind::SimilarFunctions)
        );
        assert!(
            included_findings
                .iter()
                .any(|finding| finding.kind == FindingKind::SimilarFunctions)
        );
        Ok(())
    }

    #[test]
    fn recognizes_common_test_source_names() {
        assert!(is_test_source(Path::new("src/app_test.go")));
        assert!(is_test_source(Path::new("src/app.test.ts")));
        assert!(is_test_source(Path::new("src/app.spec.tsx")));
        assert!(is_test_source(Path::new("tests/app.rs")));
        assert!(is_test_source(Path::new("src/test_app.py")));
        assert!(!is_test_source(Path::new("src/app.go")));
    }

    #[test]
    fn writer_progress_outputs_messages() {
        let mut progress = WriterProgress::new(Vec::new());

        progress.report("Scanning example");
        progress.report("Finished scan");

        let output = String::from_utf8(progress.into_inner()).unwrap();
        assert!(output.contains("Scanning example"));
        assert!(output.contains("Finished scan"));
    }

    #[test]
    fn scan_report_outputs_percent_progress_when_enabled() -> Result<()> {
        let root = test_root("percent-progress");
        fs::create_dir_all(root.join("src"))?;
        fs::write(
            root.join("src/one.rs"),
            "fn one() -> i32 { let value = 1; value }\n",
        )?;
        fs::write(
            root.join("src/two.rs"),
            "fn two() -> i32 { let value = 2; value }\n",
        )?;

        let mut progress = WriterProgress::new(Vec::new());
        let mut args = scan_args(root.clone(), false);
        args.min_function_tokens = 1;
        let _ = scan_report(&args, &mut progress)?;

        fs::remove_dir_all(root)?;

        let output = String::from_utf8(progress.into_inner()).unwrap();
        assert!(output.contains("[ 50%] Scanning source files (1/2)"));
        assert!(output.contains("[100%] Scanning source files (2/2)"));
        assert!(output.contains("[ 50%] Analyzing similar functions: extracting candidates (1/2)"));
        assert!(output.contains("[100%] Analyzing similar functions: extracting candidates (2/2)"));
        assert!(output.contains("[100%] Analyzing similar functions: comparing candidates (1/1)"));
        Ok(())
    }
}
