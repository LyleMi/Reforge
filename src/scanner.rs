use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::{DirEntry, WalkDir};

use crate::cli::ScanArgs;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    pub path: String,
    pub line: Option<usize>,
    pub magnitude: Option<usize>,
    pub message: String,
}

pub fn scan_path(args: &ScanArgs) -> Result<Vec<Finding>> {
    let root = args
        .path
        .canonicalize()
        .with_context(|| format!("failed to resolve path {}", args.path.display()))?;

    let mut findings = Vec::new();

    if root.is_file() {
        scan_file(&root, args.max_file_lines, &mut findings)?;
    } else {
        let mut directory_source_files = BTreeMap::new();

        for entry in WalkDir::new(&root).into_iter().filter_entry(|entry| {
            let is_root = entry.path() == root.as_path();
            is_root
                || ((args.include_hidden || !is_hidden(entry))
                    && (args.include_generated || !is_default_excluded_dir(entry)))
        }) {
            let entry = entry?;

            if entry.file_type().is_file() && is_supported_source(entry.path()) {
                scan_file(entry.path(), args.max_file_lines, &mut findings)?;
                count_source_file_parent(entry.path(), &mut directory_source_files);
            }
        }

        scan_directories(&directory_source_files, args.max_dir_files, &mut findings);
    }

    Ok(findings)
}

fn scan_file(path: &Path, max_file_lines: usize, findings: &mut Vec<Finding>) -> Result<()> {
    if !is_supported_source(path) {
        return Ok(());
    }

    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read source file {}", path.display()))?;
    let line_count = source.lines().count();

    if line_count > max_file_lines {
        findings.push(Finding {
            severity: Severity::Warning,
            path: display_path(path),
            line: Some(1),
            magnitude: Some(line_count),
            message: format!("file has {line_count} lines; consider splitting responsibilities"),
        });
    }

    for (index, line) in source.lines().enumerate() {
        if has_debt_marker(line) {
            findings.push(Finding {
                severity: Severity::Info,
                path: display_path(path),
                line: Some(index + 1),
                magnitude: None,
                message: "technical-debt marker found".to_string(),
            });
        }
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
                severity: Severity::Warning,
                path: display_path(directory),
                line: None,
                magnitude: Some(*file_count),
                message: format!(
                    "directory contains {file_count} source files; consider grouping related responsibilities"
                ),
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

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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
}
