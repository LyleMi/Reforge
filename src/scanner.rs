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
    SimilarFunctionOptions, SourceFile, is_supported_similarity_source,
    scan_similar_functions_report,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    LargeFile,
    LargeDirectory,
    DebtMarker,
    SimilarFunctions,
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
}

pub struct NoopProgress;

impl ProgressSink for NoopProgress {
    fn report(&mut self, _message: &str) {}
}

pub struct StderrProgress;

impl StderrProgress {
    pub fn new() -> Self {
        Self
    }
}

impl ProgressSink for StderrProgress {
    fn report(&mut self, message: &str) {
        let _ = writeln!(std::io::stderr(), "{message}");
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
    let mut stats = ScanStats::default();

    progress.report(&format!("Scanning {}", display_path(&root)));

    if root.is_file() {
        scan_file(
            &root,
            args.max_file_lines,
            args.include_test_similarity,
            &mut findings,
            &mut similarity_sources,
            &mut stats,
        )?;
    } else {
        let mut directory_source_files = BTreeMap::new();

        for entry in WalkDir::new(&root).into_iter().filter_entry(|entry| {
            let is_root = entry.path() == root.as_path();
            is_root
                || ((args.include_hidden || !is_hidden(entry))
                    && (args.include_generated || !is_default_excluded_dir(entry)))
        }) {
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
                    &mut stats,
                )?;
                count_source_file_parent(entry.path(), &mut directory_source_files);
            }
        }

        scan_directories(&directory_source_files, args.max_dir_files, &mut findings);
    }

    progress.report(&format!(
        "Analyzing similar functions in {} files",
        similarity_sources.len()
    ));

    let similarity_scan = scan_similar_functions_report(
        &similarity_sources,
        &SimilarFunctionOptions {
            min_group_size: args.min_similar_functions,
            min_tokens: args.min_function_tokens,
            threshold: args.function_similarity,
        },
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

    Ok(ScanReport {
        summary,
        stats,
        findings,
    })
}

fn scan_file(
    path: &Path,
    max_file_lines: usize,
    include_test_similarity: bool,
    findings: &mut Vec<Finding>,
    similarity_sources: &mut Vec<SourceFile>,
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

    if is_supported_similarity_source(path) && (include_test_similarity || !is_test_source(path)) {
        similarity_sources.push(SourceFile {
            path: path.to_path_buf(),
            display_path: display_path(path),
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

fn is_test_source(path: &Path) -> bool {
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
            output: crate::cli::OutputFormat::Human,
            progress: crate::cli::ProgressMode::Auto,
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

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, FindingKind::SimilarFunctions);
        assert_eq!(findings[0].magnitude, Some(3));
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
}
