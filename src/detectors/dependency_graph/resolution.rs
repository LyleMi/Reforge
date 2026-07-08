use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use crate::detectors::similarity::SourceFile;

use super::DependencyGraph;

pub(super) fn build_dependency_graph(sources: &[SourceFile], root: &Path) -> DependencyGraph {
    let root = normalize_path(root);
    let index = source_index(sources);
    let mut graph = DependencyGraph::default();

    for source in sources {
        graph.add_node(source.display_path.clone());
        let language = Language::for_path(&source.path);
        for specifier in import_specifiers(&source.source, language) {
            if let Some(target) =
                resolve_import(source, specifier.as_str(), language, &root, &index)
            {
                graph.add_edge(source.display_path.clone(), target);
            }
        }
    }

    graph
}

fn source_index(sources: &[SourceFile]) -> BTreeMap<PathBuf, String> {
    sources
        .iter()
        .map(|source| (normalize_path(&source.path), source.display_path.clone()))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Language {
    Rust,
    JavaScript,
    Python,
    Ruby,
    CLike,
    Other,
}

impl Language {
    fn for_path(path: &Path) -> Self {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("rs") => Self::Rust,
            Some("js" | "jsx" | "ts" | "tsx") => Self::JavaScript,
            Some("py") => Self::Python,
            Some("rb") => Self::Ruby,
            Some("c" | "cc" | "cpp") => Self::CLike,
            _ => Self::Other,
        }
    }
}

fn import_specifiers(source: &str, language: Language) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| import_specifier_from_line(line, language))
        .collect()
}

fn import_specifier_from_line(line: &str, language: Language) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
    {
        return None;
    }

    match language {
        Language::Rust => rust_import_specifier(trimmed),
        Language::JavaScript => javascript_import_specifier(trimmed),
        Language::Python => python_import_specifier(trimmed),
        Language::Ruby => ruby_import_specifier(trimmed),
        Language::CLike => c_like_import_specifier(trimmed),
        Language::Other => None,
    }
}

fn rust_import_specifier(line: &str) -> Option<String> {
    let rest = line.strip_prefix("mod ")?;
    let module = rest
        .trim()
        .trim_end_matches(';')
        .split_whitespace()
        .next()?;
    identifier_like(module).then(|| format!("./{module}"))
}

fn javascript_import_specifier(line: &str) -> Option<String> {
    if !(line.starts_with("import ")
        || line.starts_with("export ")
        || line.contains("require(")
        || line.contains("import("))
    {
        return None;
    }

    if let Some(from_index) = line.find(" from ") {
        return quoted_after(&line[from_index + " from ".len()..]);
    }

    if line.starts_with("import ") || line.starts_with("export ") {
        return quoted_after(line);
    }

    line.find("require(")
        .or_else(|| line.find("import("))
        .and_then(|index| quoted_after(&line[index..]))
}

fn python_import_specifier(line: &str) -> Option<String> {
    let rest = line.strip_prefix("from ")?;
    let (module, imported) = rest.split_once(" import ")?;
    if !module.starts_with('.') {
        return None;
    }

    if module.chars().all(|character| character == '.') {
        let imported_name = imported.split(',').next()?.trim();
        if identifier_like(imported_name) {
            return Some(format!("{module}{imported_name}"));
        }
    }

    Some(module.to_string())
}

fn ruby_import_specifier(line: &str) -> Option<String> {
    let rest = line.strip_prefix("require_relative ")?;
    quoted_after(rest)
}

fn c_like_import_specifier(line: &str) -> Option<String> {
    let rest = line.strip_prefix("#include ")?;
    quoted_after(rest)
}

fn quoted_after(value: &str) -> Option<String> {
    let start = value.find(['"', '\''])?;
    let quote = value.as_bytes()[start] as char;
    let rest = &value[start + 1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn identifier_like(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn resolve_import(
    source: &SourceFile,
    specifier: &str,
    language: Language,
    root: &Path,
    index: &BTreeMap<PathBuf, String>,
) -> Option<String> {
    match language {
        Language::Rust => resolve_rust_import(source, specifier, root, index),
        Language::JavaScript | Language::Ruby | Language::CLike => {
            resolve_relative_import(source.path.parent()?, specifier, language, index)
        }
        Language::Python => resolve_python_import(source.path.parent()?, specifier, index),
        Language::Other => None,
    }
}

fn resolve_rust_import(
    source: &SourceFile,
    specifier: &str,
    root: &Path,
    index: &BTreeMap<PathBuf, String>,
) -> Option<String> {
    if specifier.starts_with("./") {
        return resolve_relative_import(source.path.parent()?, specifier, Language::Rust, index);
    }

    let module = specifier.strip_prefix("crate::")?;
    let crate_root = if root.join("src").is_dir() {
        root.join("src")
    } else {
        root.to_path_buf()
    };
    resolve_module_path(&crate_root, module.split("::"), Language::Rust, index)
}

fn resolve_python_import(
    base_dir: &Path,
    specifier: &str,
    index: &BTreeMap<PathBuf, String>,
) -> Option<String> {
    if !specifier.starts_with('.') {
        return None;
    }

    let dot_count = specifier
        .chars()
        .take_while(|character| *character == '.')
        .count();
    let mut directory = normalize_path(base_dir);
    for _ in 1..dot_count {
        directory.pop();
    }

    let module = specifier.trim_start_matches('.');
    if module.is_empty() {
        return None;
    }

    resolve_module_path(&directory, module.split('.'), Language::Python, index)
}

fn resolve_module_path<'a>(
    root: &Path,
    segments: impl Iterator<Item = &'a str>,
    language: Language,
    index: &BTreeMap<PathBuf, String>,
) -> Option<String> {
    let segments = segments
        .filter(|segment| identifier_like(segment))
        .collect::<Vec<_>>();

    for end in (1..=segments.len()).rev() {
        let candidate = segments[..end]
            .iter()
            .fold(root.to_path_buf(), |path, segment| path.join(segment));
        if let Some(target) = resolve_file_candidate(&candidate, language, index) {
            return Some(target);
        }
    }

    None
}

fn resolve_relative_import(
    base_dir: &Path,
    specifier: &str,
    language: Language,
    index: &BTreeMap<PathBuf, String>,
) -> Option<String> {
    if !specifier.starts_with('.') && language != Language::CLike {
        return None;
    }

    let candidate = normalize_path(&base_dir.join(specifier));
    resolve_file_candidate(&candidate, language, index)
}

fn resolve_file_candidate(
    candidate: &Path,
    language: Language,
    index: &BTreeMap<PathBuf, String>,
) -> Option<String> {
    let candidate = normalize_path(candidate);
    if let Some(path) = index.get(&candidate) {
        return Some(path.clone());
    }

    if candidate.extension().is_none() {
        for extension in language_extensions(language) {
            let with_extension = candidate.with_extension(extension);
            if let Some(path) = index.get(&normalize_path(&with_extension)) {
                return Some(path.clone());
            }
        }

        for extension in language_extensions(language) {
            let index_candidate = candidate.join(format!("index.{extension}"));
            if let Some(path) = index.get(&normalize_path(&index_candidate)) {
                return Some(path.clone());
            }
        }
    }

    None
}

fn language_extensions(language: Language) -> &'static [&'static str] {
    match language {
        Language::Rust => &["rs"],
        Language::JavaScript => &["ts", "tsx", "js", "jsx"],
        Language::Python => &["py"],
        Language::Ruby => &["rb"],
        Language::CLike => &["c", "cc", "cpp"],
        Language::Other => &[],
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}
