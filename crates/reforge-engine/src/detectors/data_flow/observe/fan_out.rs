use super::*;

pub(super) struct FanOutObservation {
    witness: ObservedPath,
    sink_count: usize,
    branch_count: usize,
    paths: Vec<ObservedPath>,
}

pub(super) fn fan_out_observation(
    observed: &SourcePaths,
    graph: &FlowGraph,
    min_sinks: usize,
    min_modules: usize,
) -> Option<FanOutObservation> {
    let paths = eligible_paths(observed, graph);
    let sink_count = unique_sink_functions(&paths, graph);
    let module_count = unique_sink_modules(&paths, graph);
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

fn eligible_paths(observed: &SourcePaths, graph: &FlowGraph) -> Vec<ObservedPath> {
    observed
        .paths
        .iter()
        .filter(|path| function_hops(path, graph) > 0)
        .cloned()
        .collect()
}

fn unique_sink_functions(paths: &[ObservedPath], graph: &FlowGraph) -> usize {
    paths
        .iter()
        .map(|path| graph.nodes[path.sink].function.as_str())
        .collect::<BTreeSet<_>>()
        .len()
}

fn unique_sink_modules(paths: &[ObservedPath], graph: &FlowGraph) -> usize {
    paths
        .iter()
        .map(|path| graph.nodes[path.sink].module.as_str())
        .collect::<BTreeSet<_>>()
        .len()
}

pub(super) fn fan_out_detection(
    observation: FanOutObservation,
    graph: &FlowGraph,
    config: &DataFlowConfig,
) -> DetectedEvidence {
    let witness_path = &observation.witness;
    let paths = &observation.paths;
    let source = &graph.nodes[witness_path.source];
    let modules = unique_sink_modules(paths, graph);
    let max_steps = paths
        .iter()
        .map(|path| path.edges.len())
        .max()
        .unwrap_or_default();
    let metrics = fan_out_measurements(&observation, modules, max_steps, config);
    let related = sink_locations(paths, graph);
    let mut detection = DetectedEvidence::from(
        DetectedEvidenceInput::new(
            Rule::FlowFanOut,
            source.path.clone(),
            Some(source.line),
            format!(
                "value {} fans out to {} independent sinks across {modules} modules",
                source.name, observation.sink_count
            ),
            metrics,
        )
        .with_related_locations(related)
        .with_subject(
            crate::model::DetectedSubject::Symbol {
                declaration_kind: "function".into(),
                name: source.function.clone(),
                signature: None,
            },
            source.language.clone(),
        ),
    );
    detection.flow_witness = Some(witness("flow_fan_out", witness_path, graph));
    detection.normalize_flow_anchor();
    detection
}

fn fan_out_measurements(
    observation: &FanOutObservation,
    modules: usize,
    max_steps: usize,
    config: &DataFlowConfig,
) -> Vec<DetectedMeasurement> {
    vec![
        DetectedMeasurement::threshold(
            MetricId::FlowSinkCount,
            observation.sink_count,
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
    ]
}

fn sink_locations(paths: &[ObservedPath], graph: &FlowGraph) -> Vec<RelatedLocation> {
    let mut related = paths
        .iter()
        .map(|path| {
            let sink = &graph.nodes[path.sink];
            RelatedLocation {
                path: sink.path.clone(),
                line: sink.line,
                name: Some(format!("sink: {}", sink.name)),
                entity_key: Some(sink.id.clone()),
            }
        })
        .collect::<Vec<_>>();
    related.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.line.cmp(&right.line))
    });
    related.dedup();
    related
}
