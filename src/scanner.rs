use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use walkdir::{DirEntry, WalkDir};

use crate::cli::ScanArgs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    pub path: String,
    pub line: usize,
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
        for entry in WalkDir::new(&root)
            .into_iter()
            .filter_entry(|entry| args.include_hidden || !is_hidden(entry))
        {
            let entry = entry?;

            if entry.file_type().is_file() && is_supported_source(entry.path()) {
                scan_file(entry.path(), args.max_file_lines, &mut findings)?;
            }
        }
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
            line: 1,
            message: format!("file has {line_count} lines; consider splitting responsibilities"),
        });
    }

    for (index, line) in source.lines().enumerate() {
        if has_debt_marker(line) {
            findings.push(Finding {
                severity: Severity::Info,
                path: display_path(path),
                line: index + 1,
                message: "technical-debt marker found".to_string(),
            });
        }
    }

    Ok(())
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

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
