use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use crate::model::{Finding, FindingKind, FindingMetric, RelatedLocation};
use crate::scanner::{FindingInput, finding};

use super::similarity::SourceFile;

const MIN_HUB_FILES: usize = 8;
const MIN_HUB_DEGREE: usize = 6;
const HUB_OUTLIER_MULTIPLIER: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
struct GraphNode {
    path: String,
    edges: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DependencyGraph {
    nodes: BTreeMap<String, GraphNode>,
}

impl DependencyGraph {
    fn add_node(&mut self, path: String) {
        self.nodes.entry(path.clone()).or_insert(GraphNode {
            path,
            edges: BTreeSet::new(),
        });
    }

    fn add_edge(&mut self, from: String, to: String) {
        if from == to {
            return;
        }

        self.add_node(from.clone());
        self.add_node(to.clone());
        if let Some(node) = self.nodes.get_mut(&from) {
            node.edges.insert(to);
        }
    }
}

pub(crate) fn scan_dependency_graph(sources: &[SourceFile], root: &Path) -> Vec<Finding> {
    let graph = build_dependency_graph(sources, root);
    let mut findings = dependency_cycle_findings(&graph);
    findings.extend(dependency_hub_findings(&graph));
    findings
}

fn build_dependency_graph(sources: &[SourceFile], root: &Path) -> DependencyGraph {
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

fn dependency_cycle_findings(graph: &DependencyGraph) -> Vec<Finding> {
    strongly_connected_components(graph)
        .into_iter()
        .filter(|component| component.len() > 1)
        .map(|mut component| {
            component.sort();
            let primary_path = component[0].clone();
            let related_locations = component
                .iter()
                .map(|path| RelatedLocation {
                    path: path.clone(),
                    line: 1,
                    name: Some("cycle member".to_string()),
                })
                .collect::<Vec<_>>();

            finding(
                FindingInput::new(
                    FindingKind::DependencyCycle,
                    primary_path,
                    Some(1),
                    format!(
                        "dependency cycle spans {} source files",
                        related_locations.len()
                    ),
                    vec![FindingMetric::threshold(
                        "cycle_files",
                        related_locations.len(),
                        2,
                        "files",
                    )],
                )
                .with_related_locations(related_locations),
            )
        })
        .collect()
}

fn dependency_hub_findings(graph: &DependencyGraph) -> Vec<Finding> {
    if graph.nodes.len() < MIN_HUB_FILES {
        return Vec::new();
    }

    let fan_in = fan_in_counts(graph);
    let fan_out_values = graph
        .nodes
        .values()
        .map(|node| node.edges.len())
        .collect::<Vec<_>>();
    let fan_in_values = graph
        .nodes
        .keys()
        .map(|path| fan_in.get(path).copied().unwrap_or(0))
        .collect::<Vec<_>>();
    let fan_out_baseline = percentile(&fan_out_values, 0.75);
    let fan_in_baseline = percentile(&fan_in_values, 0.75);

    graph
        .nodes
        .values()
        .filter_map(|node| {
            let fan_out = node.edges.len();
            let fan_in = fan_in.get(&node.path).copied().unwrap_or(0);
            if !is_hub_degree(fan_out, fan_out_baseline) && !is_hub_degree(fan_in, fan_in_baseline)
            {
                return None;
            }

            let mut metrics = Vec::new();
            if fan_out > 0 {
                metrics.push(FindingMetric::threshold(
                    "fan_out",
                    fan_out,
                    MIN_HUB_DEGREE,
                    "resolved dependencies",
                ));
            }
            if fan_in > 0 {
                metrics.push(FindingMetric::threshold(
                    "fan_in",
                    fan_in,
                    MIN_HUB_DEGREE,
                    "resolved dependents",
                ));
            }

            Some(finding(FindingInput::new(
                FindingKind::DependencyHub,
                node.path.clone(),
                Some(1),
                format!("dependency hub has fan-in {fan_in} and fan-out {fan_out}"),
                metrics,
            )))
        })
        .collect()
}

fn fan_in_counts(graph: &DependencyGraph) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::<String, usize>::new();
    for node in graph.nodes.values() {
        for target in &node.edges {
            *counts.entry(target.clone()).or_default() += 1;
        }
    }
    counts
}

fn is_hub_degree(degree: usize, baseline: usize) -> bool {
    degree >= MIN_HUB_DEGREE && degree >= baseline.saturating_mul(HUB_OUTLIER_MULTIPLIER).max(1)
}

fn percentile(values: &[usize], percentile: f64) -> usize {
    if values.is_empty() {
        return 0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() - 1) as f64 * percentile).ceil() as usize;
    sorted[index.min(sorted.len() - 1)]
}

fn strongly_connected_components(graph: &DependencyGraph) -> Vec<Vec<String>> {
    Tarjan::new(graph).components()
}

struct Tarjan<'a> {
    graph: &'a DependencyGraph,
    index: usize,
    stack: Vec<String>,
    indices: BTreeMap<String, usize>,
    lowlinks: BTreeMap<String, usize>,
    on_stack: BTreeSet<String>,
    components: Vec<Vec<String>>,
}

impl<'a> Tarjan<'a> {
    fn new(graph: &'a DependencyGraph) -> Self {
        Self {
            graph,
            index: 0,
            stack: Vec::new(),
            indices: BTreeMap::new(),
            lowlinks: BTreeMap::new(),
            on_stack: BTreeSet::new(),
            components: Vec::new(),
        }
    }

    fn components(mut self) -> Vec<Vec<String>> {
        for path in self.graph.nodes.keys() {
            if !self.indices.contains_key(path) {
                self.connect(path);
            }
        }
        self.components
    }

    fn connect(&mut self, path: &str) {
        self.push_path(path);
        for target in self.edges_for(path) {
            self.visit_edge(path, &target);
        }
        self.emit_component_if_root(path);
    }

    fn push_path(&mut self, path: &str) {
        self.indices.insert(path.to_string(), self.index);
        self.lowlinks.insert(path.to_string(), self.index);
        self.index += 1;
        self.stack.push(path.to_string());
        self.on_stack.insert(path.to_string());
    }

    fn edges_for(&self, path: &str) -> Vec<String> {
        self.graph
            .nodes
            .get(path)
            .map(|node| node.edges.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn visit_edge(&mut self, path: &str, target: &str) {
        if !self.indices.contains_key(target) {
            self.connect(target);
            self.merge_child_lowlink(path, target);
        } else if self.on_stack.contains(target) {
            self.merge_stack_index(path, target);
        }
    }

    fn merge_child_lowlink(&mut self, path: &str, target: &str) {
        let target_lowlink = self.lowlinks[target];
        let path_lowlink = self.lowlinks.get_mut(path).expect("path should be known");
        *path_lowlink = (*path_lowlink).min(target_lowlink);
    }

    fn merge_stack_index(&mut self, path: &str, target: &str) {
        let target_index = self.indices[target];
        let path_lowlink = self.lowlinks.get_mut(path).expect("path should be known");
        *path_lowlink = (*path_lowlink).min(target_index);
    }

    fn emit_component_if_root(&mut self, path: &str) {
        if self.indices[path] == self.lowlinks[path] {
            let component = self.pop_component(path);
            self.components.push(component);
        }
    }

    fn pop_component(&mut self, root: &str) -> Vec<String> {
        let mut component = Vec::new();
        while let Some(member) = self.stack.pop() {
            self.on_stack.remove(&member);
            let is_root = member == root;
            component.push(member);
            if is_root {
                break;
            }
        }
        component
    }
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn source(path: &str, contents: &str) -> SourceFile {
        SourceFile {
            path: PathBuf::from(path),
            display_path: path.replace('\\', "/"),
            source: Arc::from(contents),
        }
    }

    #[test]
    fn detects_resolved_javascript_cycle() {
        let sources = vec![
            source("project/src/a.ts", "import { b } from './b';\n"),
            source("project/src/b.ts", "import { a } from './a';\n"),
        ];

        let findings = scan_dependency_graph(&sources, Path::new("project"));

        let cycle = findings
            .iter()
            .find(|finding| finding.kind == FindingKind::DependencyCycle)
            .expect("cycle should be reported");
        assert_eq!(cycle.related_locations.len(), 2);
        assert_eq!(cycle.metrics[0].name, "cycle_files");
    }

    #[test]
    fn ignores_unresolved_external_imports() {
        let sources = vec![source(
            "project/src/a.ts",
            "import express from 'express';\nimport local from './missing';\n",
        )];

        let findings = scan_dependency_graph(&sources, Path::new("project"));

        assert!(findings.is_empty());
    }

    #[test]
    fn detects_dependency_hub_with_high_fan_out() {
        let mut sources = vec![source(
            "project/src/hub.ts",
            "import './a';\nimport './b';\nimport './c';\nimport './d';\nimport './e';\nimport './f';\n",
        )];
        for name in ["a", "b", "c", "d", "e", "f", "quiet"] {
            sources.push(source(
                &format!("project/src/{name}.ts"),
                "export const value = 1;\n",
            ));
        }

        let findings = scan_dependency_graph(&sources, Path::new("project"));

        let hub = findings
            .iter()
            .find(|finding| finding.kind == FindingKind::DependencyHub)
            .expect("hub should be reported");
        assert_eq!(hub.path, "project/src/hub.ts");
        assert_eq!(
            hub.metrics
                .iter()
                .find(|metric| metric.name == "fan_out")
                .map(|metric| metric.value),
            Some(6)
        );
    }
}
