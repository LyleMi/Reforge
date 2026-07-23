use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use crate::detectors::similarity::SourceFile;

use super::DependencyGraph;

pub(super) fn build_dependency_graph(
    sources: &[SourceFile],
    _root: &Path,
) -> (DependencyGraph, BTreeMap<String, usize>) {
    let index = source_index(sources);
    let rust_include_contexts = rust_include_contexts(sources, &index);
    let csharp_types = csharp_type_index(sources);
    let mut graph = DependencyGraph::default();
    let mut unresolved_by_file = BTreeMap::new();

    for source in sources {
        let unresolved = add_source_dependencies(
            source,
            &index,
            &rust_include_contexts,
            &csharp_types,
            &mut graph,
        );
        if unresolved > 0 {
            unresolved_by_file.insert(source.display_path.clone(), unresolved);
        }
    }

    (graph, unresolved_by_file)
}

fn add_source_dependencies(
    source: &SourceFile,
    index: &BTreeMap<PathBuf, String>,
    rust_include_contexts: &BTreeMap<PathBuf, PathBuf>,
    csharp_types: &CSharpTypeIndex,
    graph: &mut DependencyGraph,
) -> usize {
    graph.add_node(source.display_path.clone());
    let language = Language::for_path(&source.path);
    if language == Language::Rust {
        return add_rust_dependencies(source, index, rust_include_contexts, graph);
    }
    if language == Language::CSharp {
        for target in resolve_csharp_dependencies(source, csharp_types) {
            if target != source.display_path {
                graph.add_edge(source.display_path.clone(), target);
            }
        }
        return 0;
    }
    let vue_source = crate::lang::vue_script_source(&source.path, &source.source);
    let dependency_source = vue_source.as_deref().unwrap_or(&source.source);
    let mut unresolved = 0;
    for specifier in import_specifiers(dependency_source, language) {
        match resolve_import(source, &specifier, language, index) {
            Some(target) => graph.add_edge(source.display_path.clone(), target),
            None if is_unresolved_local_specifier(&specifier, language) => unresolved += 1,
            None => {}
        }
    }
    unresolved
}

fn add_rust_dependencies(
    source: &SourceFile,
    index: &BTreeMap<PathBuf, String>,
    include_contexts: &BTreeMap<PathBuf, PathBuf>,
    graph: &mut DependencyGraph,
) -> usize {
    let mut unresolved = 0;
    for included in rust_include_specifiers(&source.source) {
        match source.path.parent().and_then(|parent| {
            resolve_file_candidate(&parent.join(included), Language::Rust, index)
        }) {
            Some(target) => graph.add_edge(source.display_path.clone(), target),
            None => unresolved += 1,
        }
    }
    for module in rust_module_specifiers(&source.source) {
        let target = match module {
            RustModuleSpecifier::Standard(module) => {
                resolve_rust_module(source, &module, index, include_contexts)
            }
            RustModuleSpecifier::PathOverride(path) => source.path.parent().and_then(|parent| {
                resolve_file_candidate(&parent.join(path), Language::Rust, index)
            }),
        };
        match target {
            Some(target) => graph.add_edge(source.display_path.clone(), target),
            None => unresolved += 1,
        }
    }
    unresolved
}

fn rust_include_contexts(
    sources: &[SourceFile],
    index: &BTreeMap<PathBuf, String>,
) -> BTreeMap<PathBuf, PathBuf> {
    let mut contexts = BTreeMap::new();
    for source in sources
        .iter()
        .filter(|source| Language::for_path(&source.path) == Language::Rust)
    {
        let Some(parent) = source.path.parent() else {
            continue;
        };
        let context = rust_module_directory(&source.path);
        for included in rust_include_specifiers(&source.source) {
            let included = normalize_index_path(&parent.join(included));
            if index.contains_key(&included) {
                contexts.insert(included, context.clone());
            }
        }
    }
    contexts
}

fn rust_include_specifiers(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let include = line.trim().strip_prefix("include!(")?;
            quoted_after(include)
        })
        .collect()
}

fn is_unresolved_local_specifier(specifier: &str, language: Language) -> bool {
    is_local_specifier(specifier, language)
        && Path::new(specifier)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_none_or(|extension| language_extensions(language).contains(&extension))
}

fn is_local_specifier(specifier: &str, language: Language) -> bool {
    match language {
        Language::JavaScript => specifier.starts_with('.'),
        Language::Python | Language::Ruby | Language::Rust | Language::CLike => true,
        Language::CSharp | Language::Other => false,
    }
}

fn source_index(sources: &[SourceFile]) -> BTreeMap<PathBuf, String> {
    sources
        .iter()
        .map(|source| {
            (
                normalize_index_path(&source.path),
                source.display_path.clone(),
            )
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Language {
    Rust,
    JavaScript,
    Python,
    Ruby,
    CLike,
    CSharp,
    Other,
}

impl Language {
    fn for_path(path: &Path) -> Self {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("rs") => Self::Rust,
            Some("js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" | "vue") => {
                Self::JavaScript
            }
            Some("py") => Self::Python,
            Some("rb") => Self::Ruby,
            Some("c" | "h" | "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx") => Self::CLike,
            Some("cs" | "csx") => Self::CSharp,
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
        Language::Rust => None,
        Language::JavaScript => javascript_import_specifier(trimmed),
        Language::Python => python_import_specifier(trimmed),
        Language::Ruby => ruby_import_specifier(trimmed),
        Language::CLike => c_like_import_specifier(trimmed),
        Language::CSharp => csharp_import_specifier(trimmed),
        Language::Other => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RustModuleSpecifier {
    Standard(String),
    PathOverride(String),
}

fn rust_module_specifiers(source: &str) -> Vec<RustModuleSpecifier> {
    let mut modules = Vec::new();
    let mut path_override = None;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[") {
            if trimmed.starts_with("#[path") {
                path_override = quoted_after(trimmed);
            }
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if let Some(module) = rust_external_module(trimmed) {
            modules.push(path_override.take().map_or(
                RustModuleSpecifier::Standard(module),
                RustModuleSpecifier::PathOverride,
            ));
        } else {
            path_override = None;
        }
    }

    modules
}

fn rust_external_module(line: &str) -> Option<String> {
    let declaration = line.split_once("//").map_or(line, |(code, _)| code).trim();
    let declaration = declaration.strip_suffix(';')?.trim();
    let declaration = if let Some(rest) = declaration.strip_prefix("pub ") {
        rest
    } else if let Some(rest) = declaration.strip_prefix("pub(") {
        rest.split_once(')')?.1.trim()
    } else {
        declaration
    };
    let module = declaration.strip_prefix("mod ")?.trim();
    identifier_like(module).then(|| module.to_string())
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

fn csharp_import_specifier(line: &str) -> Option<String> {
    let rest = line
        .strip_prefix("global using ")
        .or_else(|| line.strip_prefix("using "))?;
    let rest = rest.strip_prefix("static ").unwrap_or(rest);
    let imported = rest.split_once('=').map_or(rest, |(_, target)| target);
    let imported = imported
        .split_once("//")
        .map_or(imported, |(target, _)| target)
        .trim()
        .trim_end_matches(';')
        .trim();
    namespace_like(imported).then(|| imported.to_string())
}

fn namespace_like(value: &str) -> bool {
    !value.is_empty()
        && value.split('.').all(identifier_like)
        && !value.starts_with('.')
        && !value.ends_with('.')
}

include!("resolution_csharp.rs");

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
    index: &BTreeMap<PathBuf, String>,
) -> Option<String> {
    match language {
        Language::Rust => None,
        Language::JavaScript | Language::Ruby | Language::CLike => {
            resolve_relative_import(source.path.parent()?, specifier, language, index)
        }
        Language::Python => resolve_python_import(source.path.parent()?, specifier, index),
        Language::CSharp => None,
        Language::Other => None,
    }
}

fn resolve_rust_module(
    source: &SourceFile,
    module: &str,
    index: &BTreeMap<PathBuf, String>,
    include_contexts: &BTreeMap<PathBuf, PathBuf>,
) -> Option<String> {
    let normalized = normalize_index_path(&source.path);
    let module_directory = include_contexts
        .get(&normalized)
        .cloned()
        .unwrap_or_else(|| rust_module_directory(&source.path));
    resolve_file_candidate(&module_directory.join(module), Language::Rust, index)
}

fn rust_module_directory(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    match path.file_stem().and_then(|stem| stem.to_str()) {
        Some("main" | "lib" | "mod") | None => parent.to_path_buf(),
        Some(stem) => parent.join(stem),
    }
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
    let mut directory = normalize_index_path(base_dir);
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

    let candidate = normalize_index_path(&base_dir.join(specifier));
    resolve_file_candidate(&candidate, language, index)
}

fn resolve_file_candidate(
    candidate: &Path,
    language: Language,
    index: &BTreeMap<PathBuf, String>,
) -> Option<String> {
    let candidate = normalize_index_path(candidate);
    if candidate.extension().is_some() {
        return indexed_path(&candidate, index);
    }
    std::iter::once(candidate.clone())
        .chain(extensionless_file_candidates(&candidate, language))
        .find_map(|path| indexed_path(&path, index))
}

fn indexed_path(candidate: &Path, index: &BTreeMap<PathBuf, String>) -> Option<String> {
    index.get(&normalize_index_path(candidate)).cloned()
}

fn extensionless_file_candidates(candidate: &Path, language: Language) -> Vec<PathBuf> {
    let extensions = language_extensions(language);
    let mut candidates = extensions
        .iter()
        .map(|extension| candidate.with_extension(extension))
        .collect::<Vec<_>>();
    if language == Language::Rust {
        candidates.push(candidate.join("mod.rs"));
    } else {
        candidates.extend(
            extensions
                .iter()
                .map(|extension| candidate.join(format!("index.{extension}"))),
        );
    }
    candidates
}

fn language_extensions(language: Language) -> &'static [&'static str] {
    match language {
        Language::Rust => &["rs"],
        Language::JavaScript => &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "vue"],
        Language::Python => &["py"],
        Language::Ruby => &["rb"],
        Language::CLike => &["c", "h", "cc", "cpp", "cxx", "hh", "hpp", "hxx"],
        Language::CSharp => &["cs", "csx"],
        Language::Other => &[],
    }
}

fn normalize_index_path(path: &Path) -> PathBuf {
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
