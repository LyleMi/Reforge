use std::fs;
use std::path::Path;

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
        for entry in WalkDir::new(&root).into_iter().filter_entry(|entry| {
            let is_root = entry.path() == root.as_path();
            is_root
                || ((args.include_hidden || !is_hidden(entry))
                    && (args.include_generated || !is_default_excluded_dir(entry)))
        }) {
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
}
