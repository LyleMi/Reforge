use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use crate::detectors::similarity::SourceFile;

use super::DependencyGraph;

pub(super) fn build_dependency_graph(
    sources: &[SourceFile],
    root: &Path,
) -> (DependencyGraph, BTreeMap<String, usize>) {
    let root = normalize_path(root);
    let index = source_index(sources);
    let csharp_types = csharp_type_index(sources);
    let mut graph = DependencyGraph::default();
    let mut unresolved_by_file = BTreeMap::new();

    for source in sources {
        let unresolved = add_source_dependencies(source, &root, &index, &csharp_types, &mut graph);
        if unresolved > 0 {
            unresolved_by_file.insert(source.display_path.clone(), unresolved);
        }
    }

    (graph, unresolved_by_file)
}

fn add_source_dependencies(
    source: &SourceFile,
    root: &Path,
    index: &BTreeMap<PathBuf, String>,
    csharp_types: &CSharpTypeIndex,
    graph: &mut DependencyGraph,
) -> usize {
    graph.add_node(source.display_path.clone());
    let language = Language::for_path(&source.path);
    if language == Language::CSharp {
        for target in resolve_csharp_dependencies(source, csharp_types) {
            if target != source.display_path {
                graph.add_edge(source.display_path.clone(), target);
            }
        }
        return 0;
    }
    let vue_source = crate::language::vue_script_source(&source.path, &source.source);
    let dependency_source = vue_source.as_deref().unwrap_or(&source.source);
    let mut unresolved = 0;
    for specifier in import_specifiers(dependency_source, language) {
        match resolve_import(source, &specifier, language, root, index) {
            Some(target) => graph.add_edge(source.display_path.clone(), target),
            None if is_local_specifier(&specifier, language) => unresolved += 1,
            None => {}
        }
    }
    unresolved
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
            Some("c" | "cc" | "cpp") => Self::CLike,
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
        Language::Rust => rust_import_specifier(trimmed),
        Language::JavaScript => javascript_import_specifier(trimmed),
        Language::Python => python_import_specifier(trimmed),
        Language::Ruby => ruby_import_specifier(trimmed),
        Language::CLike => c_like_import_specifier(trimmed),
        Language::CSharp => csharp_import_specifier(trimmed),
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

#[derive(Default)]
struct CSharpTypeIndex {
    by_namespace: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    by_qualified_name: BTreeMap<String, Vec<String>>,
}

fn csharp_type_index(sources: &[SourceFile]) -> CSharpTypeIndex {
    let mut index = CSharpTypeIndex::default();
    for source in sources
        .iter()
        .filter(|source| Language::for_path(&source.path) == Language::CSharp)
    {
        let code = csharp_code_only(&source.source);
        let namespaces = csharp_declared_namespaces(&code);
        let namespace = namespaces.first().cloned().unwrap_or_default();
        for type_name in csharp_declared_types(&code) {
            let paths = index
                .by_namespace
                .entry(namespace.clone())
                .or_default()
                .entry(type_name.clone())
                .or_default();
            if !paths.contains(&source.display_path) {
                paths.push(source.display_path.clone());
            }
            let qualified = if namespace.is_empty() {
                type_name
            } else {
                format!("{namespace}.{type_name}")
            };
            let paths = index.by_qualified_name.entry(qualified).or_default();
            if !paths.contains(&source.display_path) {
                paths.push(source.display_path.clone());
            }
        }
    }
    index
}

fn csharp_declared_types(source: &str) -> Vec<String> {
    let tokens = csharp_identifiers(source);
    tokens
        .windows(2)
        .filter(|window| {
            matches!(
                window[0].as_str(),
                "class" | "struct" | "interface" | "enum" | "record"
            )
        })
        .map(|window| window[1].clone())
        .collect()
}

fn csharp_identifiers(source: &str) -> Vec<String> {
    source
        .split(|character: char| !(character == '_' || character.is_ascii_alphanumeric()))
        .filter(|value| {
            !value.is_empty()
                && value
                    .chars()
                    .next()
                    .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        })
        .map(str::to_string)
        .collect()
}

fn resolve_csharp_dependencies(source: &SourceFile, index: &CSharpTypeIndex) -> Vec<String> {
    let code = csharp_code_only(&source.source);
    let declared_namespaces = csharp_declared_namespaces(&code);
    let identifiers = csharp_identifiers(&code)
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let (imported_namespaces, aliases) = csharp_imports(&code);
    let mut targets = std::collections::BTreeSet::new();
    add_namespace_targets(
        declared_namespaces.iter().chain(imported_namespaces.iter()),
        &identifiers,
        index,
        &mut targets,
    );
    add_alias_targets(aliases, &identifiers, index, &mut targets);
    for (qualified, paths) in &index.by_qualified_name {
        if code.contains(qualified) {
            targets.extend(paths.iter().cloned());
        }
    }
    targets.into_iter().collect()
}

fn csharp_imports(code: &str) -> (Vec<String>, Vec<(String, String)>) {
    let mut namespaces = Vec::new();
    let mut aliases = Vec::new();
    for line in code.lines() {
        let trimmed = line.trim();
        let Some(specifier) = csharp_import_specifier(trimmed) else {
            continue;
        };
        match trimmed
            .strip_prefix("using ")
            .and_then(|value| value.split_once('='))
        {
            Some((left, _)) => aliases.push((left.trim().to_string(), specifier)),
            None => namespaces.push(specifier),
        }
    }
    (namespaces, aliases)
}

fn add_namespace_targets<'a>(
    namespaces: impl Iterator<Item = &'a String>,
    identifiers: &std::collections::BTreeSet<String>,
    index: &CSharpTypeIndex,
    targets: &mut std::collections::BTreeSet<String>,
) {
    for namespace in namespaces {
        let Some(types) = index.by_namespace.get(namespace) else {
            continue;
        };
        for (type_name, paths) in types {
            if identifiers.contains(type_name) {
                targets.extend(paths.iter().cloned());
            }
        }
    }
}

fn add_alias_targets(
    aliases: Vec<(String, String)>,
    identifiers: &std::collections::BTreeSet<String>,
    index: &CSharpTypeIndex,
    targets: &mut std::collections::BTreeSet<String>,
) {
    for (alias, qualified) in aliases {
        if !identifiers.contains(&alias) {
            continue;
        }
        if let Some(paths) = index.by_qualified_name.get(&qualified) {
            targets.extend(paths.iter().cloned());
        }
    }
}

#[derive(Clone, Copy)]
enum CSharpLexState {
    Code,
    LineComment,
    BlockComment,
    String,
    Character,
}

fn csharp_code_only(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut state = CSharpLexState::Code;
    while let Some(character) = chars.next() {
        state = mask_csharp_character(state, character, &mut chars, &mut output);
    }
    output
}

fn mask_csharp_character(
    state: CSharpLexState,
    character: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    output: &mut String,
) -> CSharpLexState {
    match state {
        CSharpLexState::Code => mask_csharp_code(character, chars, output),
        CSharpLexState::LineComment => mask_csharp_line_comment(character, output),
        CSharpLexState::BlockComment => mask_csharp_block_comment(character, chars, output),
        CSharpLexState::String => mask_csharp_quoted(character, '"', chars, output),
        CSharpLexState::Character => mask_csharp_quoted(character, '\'', chars, output),
    }
}

fn mask_csharp_code(
    character: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    output: &mut String,
) -> CSharpLexState {
    let next_state = match (character, chars.peek()) {
        ('/', Some('/')) => Some(CSharpLexState::LineComment),
        ('/', Some('*')) => Some(CSharpLexState::BlockComment),
        ('"', _) => Some(CSharpLexState::String),
        ('\'', _) => Some(CSharpLexState::Character),
        _ => None,
    };
    if let Some(next_state) = next_state {
        output.push(' ');
        if matches!(
            next_state,
            CSharpLexState::LineComment | CSharpLexState::BlockComment
        ) {
            output.push(' ');
            chars.next();
        }
        next_state
    } else {
        output.push(character);
        CSharpLexState::Code
    }
}

fn mask_csharp_line_comment(character: char, output: &mut String) -> CSharpLexState {
    if character == '\n' {
        output.push('\n');
        CSharpLexState::Code
    } else {
        output.push(' ');
        CSharpLexState::LineComment
    }
}

fn mask_csharp_block_comment(
    character: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    output: &mut String,
) -> CSharpLexState {
    if character == '*' && chars.peek() == Some(&'/') {
        output.push_str("  ");
        chars.next();
        CSharpLexState::Code
    } else {
        output.push(if character == '\n' { '\n' } else { ' ' });
        CSharpLexState::BlockComment
    }
}

fn mask_csharp_quoted(
    character: char,
    quote: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    output: &mut String,
) -> CSharpLexState {
    if character == '\\' {
        output.push(' ');
        if let Some(escaped) = chars.next() {
            output.push(if escaped == '\n' { '\n' } else { ' ' });
        }
    } else {
        output.push(if character == '\n' { '\n' } else { ' ' });
    }
    if character == quote || character == '\n' {
        CSharpLexState::Code
    } else if quote == '"' {
        CSharpLexState::String
    } else {
        CSharpLexState::Character
    }
}

fn csharp_declared_namespaces(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("namespace ")?;
            let namespace = rest
                .chars()
                .take_while(|character| {
                    *character == '.' || *character == '_' || character.is_ascii_alphanumeric()
                })
                .collect::<String>();
            namespace_like(&namespace).then_some(namespace)
        })
        .collect()
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
        Language::CSharp => None,
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
        Language::JavaScript => &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "vue"],
        Language::Python => &["py"],
        Language::Ruby => &["rb"],
        Language::CLike => &["c", "cc", "cpp"],
        Language::CSharp => &["cs", "csx"],
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
