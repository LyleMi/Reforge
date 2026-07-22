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
    let (graph, unresolved_by_file) = build_dependency_graph(sources, root);
    let unresolved_edges = unresolved_by_file.values().sum();
    let mut findings = dependency_cycle_findings(&graph);
    findings.extend(dependency_hub_findings(&graph));
    DependencyGraphScan {
        snapshot: graph.snapshot(),
        findings,
        unresolved_edges,
        unresolved_by_file,
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct DependencyGraphScan {
    pub snapshot: DependencyGraphSnapshot,
    pub findings: Vec<Finding>,
    pub unresolved_edges: usize,
    pub unresolved_by_file: BTreeMap<String, usize>,
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
        .filter(|node| !is_rust_module_index(&node.path))
        .filter_map(|node| dependency_hub_finding(graph, node, &context))
        .collect()
}

fn is_rust_module_index(path: &str) -> bool {
    Path::new(path).file_name().and_then(|name| name.to_str()) == Some("mod.rs")
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

include!("dependency_graph_algorithms.rs");

#[cfg(test)]
#[path = "../dependency_graph_tests.rs"]
mod tests;
