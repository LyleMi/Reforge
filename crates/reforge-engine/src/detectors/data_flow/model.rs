use std::collections::BTreeMap;

use crate::model::{
    FlowEdgeKind, FlowLocation, FlowProgram, FlowProgramEdge, FlowResolution, FlowUnresolvedRecord,
};

pub(super) type NodeId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CallTransition {
    None,
    Enter,
    Return,
}

#[derive(Debug, Clone)]
pub(super) struct FlowEdge {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: FlowEdgeKind,
    pub resolution: FlowResolution,
    pub path: String,
    pub line: usize,
    pub name: String,
    pub call_site: Option<String>,
    pub transition: CallTransition,
}

#[derive(Debug, Clone)]
pub(super) struct FunctionRecord {
    pub symbol: String,
    pub crate_key: String,
    pub module: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub parameter_nodes: Vec<NodeId>,
    pub parameter_groups: Vec<Vec<NodeId>>,
    pub parameter_groups_exact: Vec<bool>,
    pub return_node: NodeId,
}

#[derive(Debug, Clone)]
pub(super) struct CallRecord {
    pub target: String,
    pub function_index: usize,
    pub path: String,
    pub line: usize,
}

#[derive(Debug, Default)]
pub(super) struct FlowGraph {
    pub nodes: Vec<FlowLocation>,
    pub edges: Vec<FlowEdge>,
    pub functions: Vec<FunctionRecord>,
    pub functions_by_symbol: BTreeMap<String, Vec<usize>>,
    pub imports: BTreeMap<String, BTreeMap<String, String>>,
    pub calls: Vec<CallRecord>,
    pub unresolved_reasons: BTreeMap<String, usize>,
}

impl FlowGraph {
    pub fn unresolved(&mut self, reason: impl Into<String>) {
        *self.unresolved_reasons.entry(reason.into()).or_insert(0) += 1;
    }

    pub fn add_edge(&mut self, edge: FlowEdge) {
        if edge.from != edge.to {
            self.edges.push(edge);
        }
    }

    pub fn finish(&mut self) {
        self.edges.sort_by(|left, right| {
            self.nodes[left.from]
                .id
                .cmp(&self.nodes[right.from].id)
                .then_with(|| self.nodes[left.to].id.cmp(&self.nodes[right.to].id))
                .then_with(|| left.kind.cmp(&right.kind))
                .then_with(|| left.line.cmp(&right.line))
        });
        self.edges.dedup_by(|left, right| {
            left.from == right.from
                && left.to == right.to
                && left.kind == right.kind
                && left.call_site == right.call_site
        });
        self.calls.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.line.cmp(&right.line))
                .then_with(|| left.target.cmp(&right.target))
        });
    }
}

pub(super) fn program_snapshot(graph: &FlowGraph) -> FlowProgram {
    let (incoming, outgoing) = node_degrees(graph);
    let sources = graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(index, node)| {
            node.kind == crate::model::FlowNodeKind::Parameter && incoming[*index] == 0
        })
        .map(|(_, node)| node.id.clone())
        .collect();
    let sinks = graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(index, _)| outgoing[*index] == 0)
        .map(|(_, node)| node.id.clone())
        .collect();
    let edges = snapshot_edges(graph);
    let mutations = edge_ids(&edges, |kind| {
        matches!(kind, FlowEdgeKind::Mutation | FlowEdgeKind::FieldWrite)
    });
    let transformations = edge_ids(&edges, |kind| {
        matches!(
            kind,
            FlowEdgeKind::Transformation | FlowEdgeKind::Construction
        )
    });
    FlowProgram {
        modules: unique(graph.nodes.iter().map(|node| node.module.clone())),
        functions: unique(
            graph
                .functions
                .iter()
                .map(|function| function.symbol.clone()),
        ),
        nodes: graph.nodes.clone(),
        edges,
        sources,
        sinks,
        mutations,
        transformations,
        unresolved: unresolved_records(graph),
    }
}

fn unique(values: impl Iterator<Item = String>) -> Vec<String> {
    let mut values = values.collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn node_degrees(graph: &FlowGraph) -> (Vec<usize>, Vec<usize>) {
    let mut incoming = vec![0usize; graph.nodes.len()];
    let mut outgoing = vec![0usize; graph.nodes.len()];
    for edge in &graph.edges {
        incoming[edge.to] += 1;
        outgoing[edge.from] += 1;
    }
    (incoming, outgoing)
}

fn snapshot_edges(graph: &FlowGraph) -> Vec<FlowProgramEdge> {
    graph
        .edges
        .iter()
        .enumerate()
        .map(|(index, edge)| {
            let from = &graph.nodes[edge.from];
            let to = &graph.nodes[edge.to];
            FlowProgramEdge {
                id: format!("flow-edge:{}:{}:{index}", from.id, to.id),
                kind: edge.kind,
                resolution: edge.resolution,
                language: from.language.clone(),
                from: from.id.clone(),
                to: to.id.clone(),
                path: edge.path.clone(),
                line: edge.line,
                symbol: to.function.clone(),
                module: to.module.clone(),
                detail: edge.name.clone(),
            }
        })
        .collect()
}

fn edge_ids(edges: &[FlowProgramEdge], selected: impl Fn(FlowEdgeKind) -> bool) -> Vec<String> {
    edges
        .iter()
        .filter(|edge| selected(edge.kind))
        .map(|edge| edge.id.clone())
        .collect()
}

fn unresolved_records(graph: &FlowGraph) -> Vec<FlowUnresolvedRecord> {
    graph
        .unresolved_reasons
        .iter()
        .map(|(reason, count)| FlowUnresolvedRecord {
            resolution: if reason.contains("unsupported") {
                FlowResolution::Unsupported
            } else {
                FlowResolution::Unresolved
            },
            reason: reason.clone(),
            count: *count,
        })
        .collect()
}
