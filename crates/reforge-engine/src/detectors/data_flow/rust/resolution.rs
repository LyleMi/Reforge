use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use super::FlowGraph;

#[derive(Debug, Clone)]
pub(super) struct FileModule {
    pub crate_key: String,
    pub symbol: String,
}

pub(super) fn file_module(root: &Path, path: &Path) -> FileModule {
    let crate_root = nearest_cargo_root(root, path).unwrap_or_else(|| root.to_path_buf());
    let relative = path.strip_prefix(&crate_root).unwrap_or(path);
    let mut parts = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str().map(ToString::to_string),
            _ => None,
        })
        .collect::<Vec<_>>();
    if parts.first().is_some_and(|part| part == "src") {
        parts.remove(0);
    }
    let Some(file) = parts.pop() else {
        return FileModule {
            crate_key: crate_key(root, &crate_root),
            symbol: "crate".into(),
        };
    };
    let stem = file.strip_suffix(".rs").unwrap_or(&file);
    if !matches!(stem, "lib" | "main" | "mod") {
        parts.push(stem.to_string());
    }
    let symbol = if parts.is_empty() {
        "crate".into()
    } else {
        format!("crate::{}", parts.join("::"))
    };
    FileModule {
        crate_key: crate_key(root, &crate_root),
        symbol,
    }
}

pub(super) fn resolve_function(
    raw: &str,
    crate_key: &str,
    module: &str,
    graph: &FlowGraph,
) -> Option<(String, usize)> {
    let mut candidates = BTreeSet::new();
    if raw.contains("::") {
        candidates.insert(canonical_path(raw, module));
        if !raw.starts_with("crate::") && !raw.starts_with("self::") && !raw.starts_with("super::")
        {
            candidates.insert(format!("crate::{raw}"));
        }
    } else {
        candidates.insert(format!("{module}::{raw}"));
        if let Some(target) = graph
            .imports
            .get(module)
            .and_then(|imports| imports.get(raw))
        {
            candidates.insert(target.clone());
        }
    }
    let resolved = candidates
        .into_iter()
        .filter_map(|candidate| resolve_candidate(&candidate, crate_key, graph))
        .collect::<Vec<_>>();
    (resolved.len() == 1).then(|| resolved[0].clone())
}

fn resolve_candidate(
    candidate: &str,
    crate_key: &str,
    graph: &FlowGraph,
) -> Option<(String, usize)> {
    let mut current = candidate.to_string();
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(current.clone()) {
            return None;
        }
        if let Some(indices) = graph
            .functions_by_symbol
            .get(&resolution_key(crate_key, &current))
        {
            return (indices.len() == 1).then_some((current, indices[0]));
        }
        let (module, name) = current.rsplit_once("::")?;
        current = graph
            .imports
            .get(&resolution_key(crate_key, module))?
            .get(name)?
            .clone();
    }
}

pub(super) fn canonical_path(raw: &str, module: &str) -> String {
    if raw == "crate" || raw.starts_with("crate::") {
        return raw.to_string();
    }
    if raw == "self" {
        return module.to_string();
    }
    if let Some(rest) = raw.strip_prefix("self::") {
        return format!("{module}::{rest}");
    }
    if raw == "super" || raw.starts_with("super::") {
        return canonical_super_path(raw, module);
    }
    format!("{module}::{raw}")
}

fn canonical_super_path(raw: &str, module: &str) -> String {
    let mut base = module.to_string();
    let mut rest = raw;
    while rest == "super" || rest.starts_with("super::") {
        if let Some((parent, _)) = base.rsplit_once("::") {
            base = parent.to_string();
        }
        rest = rest.strip_prefix("super").unwrap_or(rest);
        rest = rest.strip_prefix("::").unwrap_or(rest);
        if rest.is_empty() {
            break;
        }
    }
    if rest.is_empty() {
        base
    } else {
        format!("{base}::{rest}")
    }
}

pub(super) fn stable_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

pub(super) fn resolution_key(crate_key: &str, symbol: &str) -> String {
    format!("{crate_key}\0{symbol}")
}

fn nearest_cargo_root(root: &Path, path: &Path) -> Option<PathBuf> {
    let boundary = if root.is_file() { root.parent()? } else { root };
    let mut current = path.parent()?;
    loop {
        if current.join("Cargo.toml").is_file() {
            return Some(current.to_path_buf());
        }
        if current == boundary || !current.starts_with(boundary) {
            return None;
        }
        current = current.parent()?;
    }
}

fn crate_key(root: &Path, crate_root: &Path) -> String {
    let relative = stable_path(root, crate_root);
    if relative.is_empty() {
        ".".into()
    } else {
        relative
    }
}
