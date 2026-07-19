use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::model::{
    DependencyGraphEdge, DependencyGraphNode, DependencyGraphSnapshot, Finding, FindingKind,
    FindingMetric, MetricId, RelatedLocation,
};
use crate::scanner::FindingInput;

use super::similarity::SourceFile;

mod resolution;

use resolution::build_dependency_graph;

const MIN_HUB_FILES: usize = 8;
const MIN_HUB_DEGREE: usize = 6;
const MIN_TRANSITIVE_REACH: usize = MIN_HUB_DEGREE * 2;
const MIN_DEPENDENCY_DEPTH: usize = 4;
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

    fn snapshot(&self) -> DependencyGraphSnapshot {
        let fan_in = fan_in_counts(self);
        let mut nodes = self
            .nodes
            .values()
            .map(|node| DependencyGraphNode {
                path: node.path.clone(),
                fan_in: fan_in.get(&node.path).copied().unwrap_or(0),
                fan_out: node.edges.len(),
            })
            .collect::<Vec<_>>();
        nodes.sort_by(|left, right| left.path.cmp(&right.path));

        let mut edges = self
            .nodes
            .values()
            .flat_map(|node| {
                node.edges.iter().map(|target| DependencyGraphEdge {
                    from: node.path.clone(),
                    to: target.clone(),
                })
            })
            .collect::<Vec<_>>();
        edges.sort_by(|left, right| {
            left.from
                .cmp(&right.from)
                .then_with(|| left.to.cmp(&right.to))
        });

        DependencyGraphSnapshot { nodes, edges }
    }
}

#[cfg(test)]
pub(crate) fn scan_dependency_graph(sources: &[SourceFile], root: &Path) -> Vec<Finding> {
    scan_dependency_graph_report(sources, root).findings
}

pub(crate) fn scan_dependency_graph_report(
    sources: &[SourceFile],
    root: &Path,
) -> DependencyGraphScan {
    let (graph, unresolved_edges) = build_dependency_graph(sources, root);
    let mut findings = dependency_cycle_findings(&graph);
    findings.extend(dependency_hub_findings(&graph));
    DependencyGraphScan {
        snapshot: graph.snapshot(),
        findings,
        unresolved_edges,
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct DependencyGraphScan {
    pub snapshot: DependencyGraphSnapshot,
    pub findings: Vec<Finding>,
    pub unresolved_edges: usize,
}

fn dependency_cycle_findings(graph: &DependencyGraph) -> Vec<Finding> {
    strongly_connected_components(graph)
        .into_iter()
        .filter(|component| component.len() > 1)
        .map(|mut component| {
            component.sort();
            let primary_path = component[0].clone();
            let cycle_edges = internal_edge_count(graph, &component);
            let cycle_density = edge_density_percent(cycle_edges, component.len());
            let related_locations = component
                .iter()
                .map(|path| RelatedLocation {
                    path: path.clone(),
                    line: 1,
                    name: Some("cycle member".to_string()),
                })
                .collect::<Vec<_>>();

            Finding::from(
                FindingInput::new(
                    FindingKind::DependencyCycle,
                    primary_path,
                    Some(1),
                    format!(
                        "dependency cycle spans {} source files with {cycle_edges} internal dependency edges",
                        related_locations.len(),
                    ),
                    vec![
                        FindingMetric::threshold(
                            MetricId::DependencyCycleFiles,
                            related_locations.len(),
                            2,
                            "files",
                        ),
                        FindingMetric::threshold(
                            MetricId::DependencyCycleEdges,
                            cycle_edges,
                            related_locations.len(),
                            "internal edges",
                        ),
                        FindingMetric::measurement(
                            MetricId::DependencyCycleDensityPercent,
                            cycle_density,
                            "percent",
                        ),
                    ],
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

    let context = HubContext::new(graph);
    graph
        .nodes
        .values()
        .filter_map(|node| dependency_hub_finding(graph, node, &context))
        .collect()
}

struct HubContext {
    fan_in: BTreeMap<String, usize>,
    reverse_edges: BTreeMap<String, BTreeSet<String>>,
    dependency_depths: BTreeMap<String, usize>,
    fan_out_baseline: usize,
    fan_in_baseline: usize,
}

impl HubContext {
    fn new(graph: &DependencyGraph) -> Self {
        let fan_in = fan_in_counts(graph);
        let reverse_edges = reverse_edges(graph);
        let dependency_depths = dependency_depths(graph);
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

        Self {
            fan_in,
            reverse_edges,
            dependency_depths,
            fan_out_baseline,
            fan_in_baseline,
        }
    }
}

fn dependency_hub_finding(
    graph: &DependencyGraph,
    node: &GraphNode,
    context: &HubContext,
) -> Option<Finding> {
    let fan_out = node.edges.len();
    let fan_in = context.fan_in.get(&node.path).copied().unwrap_or(0);
    if !is_dependency_hub(fan_in, fan_out, context) {
        return None;
    }

    let transitive_fan_out = reachable_count(graph, &node.path);
    let transitive_fan_in = reverse_reachable_count(&context.reverse_edges, &node.path);
    let dependency_depth = context
        .dependency_depths
        .get(&node.path)
        .copied()
        .unwrap_or(0);

    Some(Finding::from(FindingInput::new(
        FindingKind::DependencyHub,
        node.path.clone(),
        Some(1),
        dependency_hub_message(
            fan_in,
            fan_out,
            transitive_fan_in,
            transitive_fan_out,
            dependency_depth,
        ),
        dependency_hub_metrics(
            fan_in,
            fan_out,
            transitive_fan_in,
            transitive_fan_out,
            dependency_depth,
        ),
    )))
}

fn is_dependency_hub(fan_in: usize, fan_out: usize, context: &HubContext) -> bool {
    is_hub_degree(fan_out, context.fan_out_baseline)
        || is_hub_degree(fan_in, context.fan_in_baseline)
}

fn dependency_hub_metrics(
    fan_in: usize,
    fan_out: usize,
    transitive_fan_in: usize,
    transitive_fan_out: usize,
    dependency_depth: usize,
) -> Vec<FindingMetric> {
    let mut metrics = Vec::new();
    push_direct_coupling_metrics(&mut metrics, fan_in, fan_out);
    push_transitive_coupling_metrics(
        &mut metrics,
        fan_in,
        fan_out,
        transitive_fan_in,
        transitive_fan_out,
    );
    if dependency_depth > 1 {
        metrics.push(FindingMetric::threshold(
            MetricId::DependencyDepth,
            dependency_depth,
            MIN_DEPENDENCY_DEPTH,
            "edges",
        ));
    }
    metrics.push(FindingMetric::measurement(
        MetricId::DependencyInstabilityPercent,
        instability_percent(fan_in, fan_out),
        "percent",
    ));
    metrics
}

fn push_direct_coupling_metrics(metrics: &mut Vec<FindingMetric>, fan_in: usize, fan_out: usize) {
    if fan_out > 0 {
        metrics.push(FindingMetric::threshold(
            MetricId::DependencyFanOut,
            fan_out,
            MIN_HUB_DEGREE,
            "resolved dependencies",
        ));
    }
    if fan_in > 0 {
        metrics.push(FindingMetric::threshold(
            MetricId::DependencyFanIn,
            fan_in,
            MIN_HUB_DEGREE,
            "resolved dependents",
        ));
    }
}

fn push_transitive_coupling_metrics(
    metrics: &mut Vec<FindingMetric>,
    fan_in: usize,
    fan_out: usize,
    transitive_fan_in: usize,
    transitive_fan_out: usize,
) {
    if transitive_fan_out > fan_out {
        metrics.push(FindingMetric::threshold(
            MetricId::DependencyTransitiveFanOut,
            transitive_fan_out,
            MIN_TRANSITIVE_REACH,
            "reachable dependencies",
        ));
    }
    if transitive_fan_in > fan_in {
        metrics.push(FindingMetric::threshold(
            MetricId::DependencyTransitiveFanIn,
            transitive_fan_in,
            MIN_TRANSITIVE_REACH,
            "reachable dependents",
        ));
    }
}

fn dependency_hub_message(
    fan_in: usize,
    fan_out: usize,
    transitive_fan_in: usize,
    transitive_fan_out: usize,
    dependency_depth: usize,
) -> String {
    format!(
        "dependency hub has fan-in {fan_in}, fan-out {fan_out}, transitive fan-in {transitive_fan_in}, transitive fan-out {transitive_fan_out}, and dependency depth {dependency_depth}"
    )
}

fn internal_edge_count(graph: &DependencyGraph, paths: &[String]) -> usize {
    let members = paths.iter().map(String::as_str).collect::<BTreeSet<_>>();
    paths
        .iter()
        .filter_map(|path| graph.nodes.get(path))
        .map(|node| {
            node.edges
                .iter()
                .filter(|target| members.contains(target.as_str()))
                .count()
        })
        .sum()
}

fn edge_density_percent(edge_count: usize, node_count: usize) -> usize {
    let possible_edges = node_count.saturating_mul(node_count.saturating_sub(1));
    if possible_edges == 0 {
        return 0;
    }

    ((edge_count * 100) + (possible_edges / 2)) / possible_edges
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

fn reverse_edges(graph: &DependencyGraph) -> BTreeMap<String, BTreeSet<String>> {
    let mut reverse = graph
        .nodes
        .keys()
        .map(|path| (path.clone(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();

    for node in graph.nodes.values() {
        for target in &node.edges {
            reverse
                .entry(target.clone())
                .or_default()
                .insert(node.path.clone());
        }
    }

    reverse
}

fn reachable_count(graph: &DependencyGraph, start: &str) -> usize {
    reachable_paths(start, |path| {
        graph
            .nodes
            .get(path)
            .map(|node| node.edges.iter().cloned().collect())
            .unwrap_or_default()
    })
}

fn reverse_reachable_count(
    reverse_edges: &BTreeMap<String, BTreeSet<String>>,
    start: &str,
) -> usize {
    reachable_paths(start, |path| {
        reverse_edges
            .get(path)
            .map(|edges| edges.iter().cloned().collect())
            .unwrap_or_default()
    })
}

fn reachable_paths(start: &str, mut edges_for: impl FnMut(&str) -> Vec<String>) -> usize {
    let mut seen = BTreeSet::<String>::new();
    let mut stack = edges_for(start);

    while let Some(path) = stack.pop() {
        if !seen.insert(path.clone()) {
            continue;
        }

        for target in edges_for(&path) {
            if target != start && !seen.contains(&target) {
                stack.push(target);
            }
        }
    }

    seen.remove(start);
    seen.len()
}

fn dependency_depths(graph: &DependencyGraph) -> BTreeMap<String, usize> {
    let components = strongly_connected_components(graph);
    let mut component_by_path = BTreeMap::<String, usize>::new();
    for (component_index, component) in components.iter().enumerate() {
        for path in component {
            component_by_path.insert(path.clone(), component_index);
        }
    }

    let mut component_edges = vec![BTreeSet::<usize>::new(); components.len()];
    for node in graph.nodes.values() {
        let Some(&source_component) = component_by_path.get(&node.path) else {
            continue;
        };
        for target in &node.edges {
            let Some(&target_component) = component_by_path.get(target) else {
                continue;
            };
            if source_component != target_component {
                component_edges[source_component].insert(target_component);
            }
        }
    }

    let mut memo = vec![None; components.len()];
    for component_index in 0..components.len() {
        component_dependency_depth(component_index, &component_edges, &mut memo);
    }

    component_by_path
        .into_iter()
        .map(|(path, component_index)| (path, memo[component_index].unwrap_or(0)))
        .collect()
}

fn component_dependency_depth(
    component: usize,
    edges: &[BTreeSet<usize>],
    memo: &mut [Option<usize>],
) -> usize {
    if let Some(depth) = memo[component] {
        return depth;
    }

    let depth = edges[component]
        .iter()
        .map(|target| 1 + component_dependency_depth(*target, edges, memo))
        .max()
        .unwrap_or(0);
    memo[component] = Some(depth);
    depth
}

fn instability_percent(fan_in: usize, fan_out: usize) -> usize {
    let total = fan_in + fan_out;
    if total == 0 {
        return 0;
    }

    ((fan_out * 100) + (total / 2)) / total
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

#[cfg(test)]
#[path = "../dependency_graph_tests.rs"]
mod tests;
