use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::evidence_analysis::DetectedEvidenceInput;
use crate::model::{
    DetectedEvidence, DetectedMeasurement, FlowEdgeKind, FlowResolution, FlowWitness,
    FlowWitnessStep, MetricId, RelatedLocation, Rule,
};
use crate::scan::config::DataFlowConfig;

use super::model::{CallTransition, FlowGraph, NodeId};

pub(super) struct ObserveResult {
    pub detections: Vec<DetectedEvidence>,
    pub truncated_paths: usize,
    pub evaluated_sources: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SearchState {
    node: NodeId,
    stack: Vec<String>,
}

#[derive(Debug, Clone)]
struct ObservedPath {
    source: NodeId,
    sink: NodeId,
    edges: Vec<usize>,
}

struct SourcePaths {
    paths: Vec<ObservedPath>,
    branch_nodes: BTreeSet<NodeId>,
    truncated: usize,
}

pub(super) fn evaluate(graph: &FlowGraph, config: &DataFlowConfig) -> ObserveResult {
    let outgoing = outgoing_edges(graph);
    let incoming = incoming_counts(graph);
    let sources = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            (node.kind == crate::model::FlowNodeKind::Parameter
                && incoming.get(&index).copied().unwrap_or_default() == 0)
                .then_some(node)
                .filter(|node| !crate::scan::is_test_source(std::path::Path::new(&node.path)))
                .map(|_| index)
        })
        .collect::<Vec<_>>();
    let mut detections = Vec::new();
    let mut truncated_paths = 0;
    let mut remaining_budget = config.work_budget;
    let mut evaluated_sources = 0;

    for source in sources {
        evaluated_sources += 1;
        let result = evaluate_source(graph, source, &outgoing, config, &mut remaining_budget);
        truncated_paths += result.truncated;
        detections.extend(result.detections);
        if remaining_budget == 0 {
            break;
        }
    }
    detections.sort_by(|left, right| left.semantic_anchor.cmp(&right.semantic_anchor));
    ObserveResult {
        detections,
        truncated_paths,
        evaluated_sources,
    }
}

struct SourceEvaluation {
    detections: Vec<DetectedEvidence>,
    truncated: usize,
}

fn evaluate_source(
    graph: &FlowGraph,
    source: NodeId,
    outgoing: &BTreeMap<NodeId, Vec<usize>>,
    config: &DataFlowConfig,
    remaining_budget: &mut usize,
) -> SourceEvaluation {
    let mut search = PathSearch {
        graph,
        source,
        outgoing,
        max_steps: config.max_path_steps,
        max_paths: config.max_paths_per_source,
        max_function_hops: config.max_function_hops,
        max_module_hops: config.max_module_hops,
        max_sinks: config.max_sinks_per_source,
        budget: remaining_budget,
    };
    let observed = enumerate_paths(&mut search);
    if observed.truncated > 0 {
        return SourceEvaluation {
            detections: Vec::new(),
            truncated: observed.truncated,
        };
    }
    let mut detections = observed
        .paths
        .iter()
        .filter(|path| excessive_relay(path, graph, config))
        .max_by(|left, right| relay_key(left, graph).cmp(&relay_key(right, graph)))
        .map(|path| relay_detection(path, graph, config))
        .into_iter()
        .collect::<Vec<_>>();
    if let Some(fan_out) =
        fan_out_observation(&observed, graph, config.min_sinks, config.min_modules)
    {
        detections.push(fan_out_detection(fan_out, graph, config));
    }
    SourceEvaluation {
        detections,
        truncated: 0,
    }
}

struct FanOutObservation {
    witness: ObservedPath,
    sink_count: usize,
    branch_count: usize,
    paths: Vec<ObservedPath>,
}

fn fan_out_observation(
    observed: &SourcePaths,
    graph: &FlowGraph,
    min_sinks: usize,
    min_modules: usize,
) -> Option<FanOutObservation> {
    let paths = observed
        .paths
        .iter()
        .filter(|path| function_hops(path, graph) > 0)
        .cloned()
        .collect::<Vec<_>>();
    let sink_count = paths
        .iter()
        .map(|path| graph.nodes[path.sink].function.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let module_count = paths
        .iter()
        .map(|path| graph.nodes[path.sink].module.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    if sink_count < min_sinks || module_count < min_modules {
        return None;
    }
    let witness = paths.iter().max_by(|left, right| {
        left.edges
            .len()
            .cmp(&right.edges.len())
            .then_with(|| graph.nodes[right.sink].id.cmp(&graph.nodes[left.sink].id))
    })?;
    Some(FanOutObservation {
        witness: witness.clone(),
        sink_count,
        branch_count: observed.branch_nodes.len(),
        paths,
    })
}

fn outgoing_edges(graph: &FlowGraph) -> BTreeMap<NodeId, Vec<usize>> {
    let mut outgoing = BTreeMap::<NodeId, Vec<usize>>::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.resolution == FlowResolution::Exact {
            outgoing.entry(edge.from).or_default().push(index);
        }
    }
    outgoing
}

fn incoming_counts(graph: &FlowGraph) -> BTreeMap<NodeId, usize> {
    let mut incoming = BTreeMap::new();
    for edge in &graph.edges {
        if edge.resolution == FlowResolution::Exact {
            *incoming.entry(edge.to).or_insert(0) += 1;
        }
    }
    incoming
}

struct PathSearch<'a> {
    graph: &'a FlowGraph,
    source: NodeId,
    outgoing: &'a BTreeMap<NodeId, Vec<usize>>,
    max_steps: usize,
    max_paths: usize,
    max_function_hops: usize,
    max_module_hops: usize,
    max_sinks: usize,
    budget: &'a mut usize,
}

struct PathAccumulator {
    queue: VecDeque<(SearchState, Vec<usize>)>,
    paths: Vec<ObservedPath>,
    branch_nodes: BTreeSet<NodeId>,
    truncated: usize,
}

fn enumerate_paths(search: &mut PathSearch<'_>) -> SourcePaths {
    let start = SearchState {
        node: search.source,
        stack: Vec::new(),
    };
    let mut accumulator = PathAccumulator {
        queue: VecDeque::from([(start, Vec::new())]),
        paths: Vec::new(),
        branch_nodes: BTreeSet::new(),
        truncated: 0,
    };
    while let Some((state, path)) = accumulator.queue.pop_front() {
        if !visit_search_state(search, &mut accumulator, state, path) {
            break;
        }
    }
    SourcePaths {
        paths: accumulator.paths,
        branch_nodes: accumulator.branch_nodes,
        truncated: accumulator.truncated,
    }
}

fn visit_search_state(
    search: &mut PathSearch<'_>,
    accumulator: &mut PathAccumulator,
    state: SearchState,
    path: Vec<usize>,
) -> bool {
    if *search.budget == 0 {
        accumulator.truncated += 1 + accumulator.queue.len();
        return false;
    }
    *search.budget -= 1;
    let edges = search
        .outgoing
        .get(&state.node)
        .cloned()
        .unwrap_or_default();
    if edges.is_empty() {
        let full = record_terminal(search, state.node, path, &mut accumulator.paths);
        accumulator.truncated += usize::from(full) * accumulator.queue.len();
        return !full;
    }
    if edges.len() > 1 {
        accumulator.branch_nodes.insert(state.node);
    }
    if path.len() >= search.max_steps {
        accumulator.truncated += edges.len();
        return true;
    }
    for edge_index in edges {
        enqueue_edge(search, accumulator, &state, &path, edge_index);
    }
    true
}

fn enqueue_edge(
    search: &PathSearch<'_>,
    accumulator: &mut PathAccumulator,
    state: &SearchState,
    path: &[usize],
    edge_index: usize,
) {
    let mut candidate = path.to_vec();
    candidate.push(edge_index);
    if function_hops_for_edges(&candidate, search.graph) > search.max_function_hops
        || module_hops_for_edges(&candidate, search.graph) > search.max_module_hops
    {
        accumulator.truncated += 1;
        return;
    }
    if let Some(next) = advance_path(search.graph, state, path, edge_index) {
        accumulator.queue.push_back(next);
    }
}

fn record_terminal(
    search: &PathSearch<'_>,
    sink: NodeId,
    edges: Vec<usize>,
    paths: &mut Vec<ObservedPath>,
) -> bool {
    if sink == search.source {
        return false;
    }
    paths.push(ObservedPath {
        source: search.source,
        sink,
        edges,
    });
    paths.len() >= search.max_paths
        || paths
            .iter()
            .map(|path| search.graph.nodes[path.sink].function.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            >= search.max_sinks
}

fn advance_path(
    graph: &FlowGraph,
    state: &SearchState,
    path: &[usize],
    edge_index: usize,
) -> Option<(SearchState, Vec<usize>)> {
    let edge = &graph.edges[edge_index];
    let mut next = SearchState {
        node: edge.to,
        stack: state.stack.clone(),
    };
    match edge.transition {
        CallTransition::None => {}
        CallTransition::Enter => next.stack.push(edge.call_site.clone().unwrap_or_default()),
        CallTransition::Return => {
            let call_site = edge.call_site.as_deref()?;
            (next.stack.last().map(String::as_str) == Some(call_site)).then_some(())?;
            next.stack.pop();
        }
    }
    if path.iter().any(|index| graph.edges[*index].to == next.node) {
        return None;
    }
    let mut next_path = path.to_vec();
    next_path.push(edge_index);
    Some((next, next_path))
}

fn excessive_relay(path: &ObservedPath, graph: &FlowGraph, config: &DataFlowConfig) -> bool {
    let function_hops = function_hops(path, graph);
    let module_hops = module_hops(path, graph);
    function_hops >= config.min_function_hops
        && module_hops >= config.min_module_hops
        && relay_ratio_percent(path, graph) >= config.min_relay_percent
}

fn relay_key(path: &ObservedPath, graph: &FlowGraph) -> (usize, usize, usize, String) {
    (
        function_hops(path, graph),
        module_hops(path, graph),
        path.edges.len(),
        graph.nodes[path.sink].id.clone(),
    )
}

fn relay_detection(
    path: &ObservedPath,
    graph: &FlowGraph,
    config: &DataFlowConfig,
) -> DetectedEvidence {
    let function_hops = function_hops(path, graph);
    let module_hops = module_hops(path, graph);
    let relay_ratio = relay_ratio_percent(path, graph);
    let source = &graph.nodes[path.source];
    let sink = &graph.nodes[path.sink];
    let metrics = vec![
        DetectedMeasurement::threshold(
            MetricId::FlowPathSteps,
            path.edges.len(),
            config.max_path_steps,
            "steps",
        ),
        DetectedMeasurement::threshold(
            MetricId::FlowFunctionHops,
            function_hops,
            config.min_function_hops,
            "functions",
        ),
        DetectedMeasurement::threshold(
            MetricId::FlowModuleHops,
            module_hops,
            config.min_module_hops,
            "modules",
        ),
        DetectedMeasurement::threshold(
            MetricId::FlowRelayRatioPercent,
            relay_ratio,
            config.min_relay_percent,
            "percent",
        ),
    ];
    let mut detection = DetectedEvidence::from(
        DetectedEvidenceInput::new(
            Rule::ExcessiveRelay,
            source.path.clone(),
            Some(source.line),
            format!(
                "value {} is relayed across {function_hops} functions and {module_hops} modules before reaching {}",
                source.name, sink.function
            ),
            metrics,
        )
        .with_related_locations(related_locations(path, graph)),
    );
    detection.flow_witness = Some(witness("excessive_relay", path, graph));
    detection.normalize_flow_anchor();
    detection
}

fn fan_out_detection(
    observation: FanOutObservation,
    graph: &FlowGraph,
    config: &DataFlowConfig,
) -> DetectedEvidence {
    let witness_path = &observation.witness;
    let paths = &observation.paths;
    let sink_count = observation.sink_count;
    let source = &graph.nodes[witness_path.source];
    let modules = paths
        .iter()
        .map(|path| graph.nodes[path.sink].module.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let max_steps = paths
        .iter()
        .map(|path| path.edges.len())
        .max()
        .unwrap_or_default();
    let metrics = vec![
        DetectedMeasurement::threshold(
            MetricId::FlowSinkCount,
            sink_count,
            config.min_sinks,
            "sinks",
        ),
        DetectedMeasurement::measurement(
            MetricId::FlowBranchCount,
            observation.branch_count,
            "branches",
        ),
        DetectedMeasurement::threshold(
            MetricId::FlowModuleCount,
            modules,
            config.min_modules,
            "modules",
        ),
        DetectedMeasurement::threshold(
            MetricId::FlowMaxPathSteps,
            max_steps,
            config.max_path_steps,
            "steps",
        ),
    ];
    let mut related = paths
        .iter()
        .map(|path| {
            let sink = &graph.nodes[path.sink];
            RelatedLocation {
                path: sink.path.clone(),
                line: sink.line,
                name: Some(format!("sink: {}", sink.name)),
            }
        })
        .collect::<Vec<_>>();
    related.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.line.cmp(&right.line))
    });
    related.dedup();
    let mut detection = DetectedEvidence::from(
        DetectedEvidenceInput::new(
            Rule::FlowFanOut,
            source.path.clone(),
            Some(source.line),
            format!(
                "value {} fans out to {sink_count} independent sinks across {modules} modules",
                source.name
            ),
            metrics,
        )
        .with_related_locations(related),
    );
    detection.flow_witness = Some(witness("flow_fan_out", witness_path, graph));
    detection.normalize_flow_anchor();
    detection
}

fn witness(rule: &str, path: &ObservedPath, graph: &FlowGraph) -> FlowWitness {
    FlowWitness {
        policy: format!("observe:{rule}"),
        source: graph.nodes[path.source].clone(),
        ordered_steps: path
            .edges
            .iter()
            .map(|index| {
                let edge = &graph.edges[*index];
                FlowWitnessStep {
                    kind: edge.kind,
                    resolution: edge.resolution,
                    from: graph.nodes[edge.from].id.clone(),
                    to: graph.nodes[edge.to].id.clone(),
                    path: edge.path.clone(),
                    line: edge.line,
                    name: edge.name.clone(),
                }
            })
            .collect(),
        sink: graph.nodes[path.sink].clone(),
        module_hops: module_hops(path, graph),
        function_hops: function_hops(path, graph),
        call_edges: function_hops(path, graph),
        path_steps: path.edges.len(),
        truncated: false,
        resolution: FlowResolution::Exact,
        limitations: Vec::new(),
        conforming_path: None,
    }
}

fn related_locations(path: &ObservedPath, graph: &FlowGraph) -> Vec<RelatedLocation> {
    let mut locations = vec![RelatedLocation {
        path: graph.nodes[path.source].path.clone(),
        line: graph.nodes[path.source].line,
        name: Some(format!("source: {}", graph.nodes[path.source].name)),
    }];
    locations.extend(path.edges.iter().map(|index| {
        let edge = &graph.edges[*index];
        RelatedLocation {
            path: graph.nodes[edge.to].path.clone(),
            line: graph.nodes[edge.to].line,
            name: Some(edge.name.clone()),
        }
    }));
    locations
}

fn function_hops(path: &ObservedPath, graph: &FlowGraph) -> usize {
    function_hops_for_edges(&path.edges, graph)
}

fn function_hops_for_edges(edges: &[usize], graph: &FlowGraph) -> usize {
    edges
        .iter()
        .filter(|index| graph.edges[**index].kind == FlowEdgeKind::ArgumentToParameter)
        .count()
}

fn module_hops(path: &ObservedPath, graph: &FlowGraph) -> usize {
    module_hops_for_edges(&path.edges, graph)
}

fn module_hops_for_edges(edges: &[usize], graph: &FlowGraph) -> usize {
    edges
        .iter()
        .filter(|index| {
            let edge = &graph.edges[**index];
            graph.nodes[edge.from].module != graph.nodes[edge.to].module
        })
        .count()
}

fn relay_ratio_percent(path: &ObservedPath, graph: &FlowGraph) -> usize {
    let hops = function_hops(path, graph);
    if hops == 0 {
        return 0;
    }
    let transformations = path
        .edges
        .iter()
        .filter(|index| {
            matches!(
                graph.edges[**index].kind,
                FlowEdgeKind::Transformation | FlowEdgeKind::Mutation | FlowEdgeKind::Construction
            )
        })
        .count();
    hops.saturating_sub(transformations.min(hops)) * 100 / hops
}

#[cfg(test)]
mod tests;
