use std::path::Path;

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::evidence_analysis::DetectedEvidenceInput;
use crate::model::{
    DetectedEvidence, DetectedMeasurement, FlowLocation, FlowWitness, FlowWitnessStep, MetricId,
    RelatedLocation, Rule,
};
use crate::scan::config::DataFlowBoundaryConfig;

use super::compose::{ExactPath, shortest_exact_path};
use super::model::FlowGraph;

pub(super) struct PolicyResult {
    pub detections: Vec<DetectedEvidence>,
    pub truncated_paths: usize,
    pub protected_sources: usize,
}

struct EffectiveBoundary<'a> {
    config: &'a DataFlowBoundaryConfig,
    protected: PathSet,
    adapters: PathSet,
    exempt: PathSet,
}

struct CandidatePath {
    path: ExactPath,
    conforming: bool,
}

pub(super) fn evaluate_policies(
    root: &Path,
    graph: &FlowGraph,
    policies: &[DataFlowBoundaryConfig],
    max_hops: usize,
) -> Result<PolicyResult> {
    let policies = policies
        .iter()
        .map(|config| EffectiveBoundary::new(root, config))
        .collect::<Result<Vec<_>>>()?;
    let mut detections = Vec::new();
    let mut truncated_paths = 0;
    let mut protected_sources = 0;

    for policy in policies {
        let (mut policy_detections, truncated, sources) =
            evaluate_boundary(&policy, graph, max_hops);
        detections.append(&mut policy_detections);
        truncated_paths += truncated;
        protected_sources += sources;
    }
    detections.sort_by(|left, right| left.semantic_anchor.cmp(&right.semantic_anchor));
    Ok(PolicyResult {
        detections,
        truncated_paths,
        protected_sources,
    })
}

fn evaluate_boundary(
    policy: &EffectiveBoundary<'_>,
    graph: &FlowGraph,
    max_hops: usize,
) -> (Vec<DetectedEvidence>, usize, usize) {
    let sources = policy_sources(policy, graph);
    let source_count = sources.len();
    let sinks = policy_sinks(policy, graph);
    let (mut candidates, truncated) = candidate_paths(policy, graph, sources, &sinks, max_hops);
    candidates.sort_by(|left, right| {
        graph.nodes[left.path.source]
            .id
            .cmp(&graph.nodes[right.path.source].id)
            .then_with(|| {
                graph.nodes[left.path.sink]
                    .id
                    .cmp(&graph.nodes[right.path.sink].id)
            })
            .then_with(|| left.path.edges.len().cmp(&right.path.edges.len()))
    });
    candidates.dedup_by(|left, right| {
        left.path.source == right.path.source && left.path.sink == right.path.sink
    });
    (
        detections_for_candidates(policy, graph, &candidates),
        truncated,
        source_count,
    )
}

fn policy_sources(policy: &EffectiveBoundary<'_>, graph: &FlowGraph) -> Vec<usize> {
    graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            (policy.protected.matches(&node.path)
                && !policy.exempt.matches(&node.path)
                && node.kind == crate::model::FlowNodeKind::Parameter)
                .then_some(index)
        })
        .collect()
}

fn policy_sinks(policy: &EffectiveBoundary<'_>, graph: &FlowGraph) -> Vec<usize> {
    let mut sinks = graph
        .calls
        .iter()
        .filter(|call| policy.config.sink_symbols.contains(&call.target))
        .filter(|call| !policy.exempt.matches(&call.path))
        .flat_map(|call| {
            graph.functions[call.function_index]
                .parameter_nodes
                .iter()
                .copied()
        })
        .collect::<Vec<_>>();
    sinks.sort_unstable();
    sinks.dedup();
    sinks
}

fn candidate_paths(
    policy: &EffectiveBoundary<'_>,
    graph: &FlowGraph,
    sources: Vec<usize>,
    sinks: &[usize],
    max_hops: usize,
) -> (Vec<CandidatePath>, usize) {
    let mut candidates = Vec::new();
    let mut truncated_paths = 0;
    for source in sources {
        for sink in sinks.iter().copied() {
            let (path, truncated) = shortest_exact_path(graph, source, sink, max_hops);
            truncated_paths += truncated;
            if let Some(path) = path {
                candidates.push(CandidatePath {
                    conforming: path_visits(&path, graph, &policy.adapters),
                    path,
                });
            }
        }
    }
    (candidates, truncated_paths)
}

fn detections_for_candidates(
    policy: &EffectiveBoundary<'_>,
    graph: &FlowGraph,
    candidates: &[CandidatePath],
) -> Vec<DetectedEvidence> {
    let conforming_path = candidates
        .iter()
        .filter(|candidate| candidate.conforming)
        .min_by_key(|candidate| candidate.path.edges.len())
        .map(|candidate| related_locations(&candidate.path, graph));
    let bypass_count = candidates
        .iter()
        .filter(|candidate| !candidate.conforming)
        .count();
    candidates
        .iter()
        .filter(|candidate| !candidate.conforming)
        .map(|candidate| {
            detection_for_path(
                candidate,
                graph,
                policy.config,
                conforming_path.clone(),
                bypass_count,
            )
        })
        .collect()
}

impl EffectiveBoundary<'_> {
    fn new<'a>(root: &Path, config: &'a DataFlowBoundaryConfig) -> Result<EffectiveBoundary<'a>> {
        Ok(EffectiveBoundary {
            config,
            protected: PathSet::new(root, &config.protected_paths)?,
            adapters: PathSet::new(root, &config.adapter_paths)?,
            exempt: PathSet::new(root, &config.exempt_paths)?,
        })
    }
}

struct PathSet {
    root: String,
    patterns: GlobSet,
    empty: bool,
}

impl PathSet {
    fn new(root: &Path, patterns: &[String]) -> Result<Self> {
        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            let normalized = normalize(pattern);
            builder.add(
                Glob::new(&normalized).with_context(|| format!("invalid path glob {pattern:?}"))?,
            );
            if !normalized
                .chars()
                .any(|ch| matches!(ch, '*' | '?' | '[' | '{'))
            {
                builder.add(Glob::new(&format!(
                    "{}/**",
                    normalized.trim_end_matches('/')
                ))?);
            }
        }
        Ok(Self {
            root: normalize(&root.to_string_lossy()),
            patterns: builder.build()?,
            empty: patterns.is_empty(),
        })
    }

    fn matches(&self, path: &str) -> bool {
        if self.empty {
            return false;
        }
        let path = normalize(path);
        let relative = path
            .strip_prefix(&self.root)
            .unwrap_or(&path)
            .trim_start_matches('/');
        self.patterns.is_match(relative)
    }
}

fn path_visits(path: &ExactPath, graph: &FlowGraph, adapters: &PathSet) -> bool {
    adapters.matches(&graph.nodes[path.source].path)
        || adapters.matches(&graph.nodes[path.sink].path)
        || path.edges.iter().any(|index| {
            let edge = &graph.edges[*index];
            adapters.matches(&graph.nodes[edge.from].path)
                || adapters.matches(&graph.nodes[edge.to].path)
        })
}

fn detection_for_path(
    candidate: &CandidatePath,
    graph: &FlowGraph,
    policy: &DataFlowBoundaryConfig,
    conforming_path: Option<Vec<RelatedLocation>>,
    bypass_count: usize,
) -> DetectedEvidence {
    let source = graph.nodes[candidate.path.source].clone();
    let sink = graph.nodes[candidate.path.sink].clone();
    let steps = witness_steps(&candidate.path, graph);
    let module_hops = module_hops(&candidate.path, graph);
    let call_edges = steps
        .iter()
        .filter(|step| step.kind == crate::model::FlowEdgeKind::ArgumentToParameter)
        .count();
    let related = related_locations(&candidate.path, graph);
    let witness = FlowWitness {
        policy: policy.name.clone(),
        source: source.clone(),
        ordered_steps: steps,
        sink: sink.clone(),
        module_hops,
        function_hops: call_edges,
        call_edges,
        path_steps: candidate.path.edges.len(),
        truncated: false,
        resolution: crate::model::FlowResolution::Exact,
        limitations: Vec::new(),
        conforming_path: conforming_path.clone(),
    };
    let metrics = path_measurements(
        &candidate.path,
        module_hops,
        call_edges,
        conforming_path.is_some(),
        bypass_count,
    );
    let mut detection = DetectedEvidence::from(
        DetectedEvidenceInput::new(
            Rule::AdapterFlowBypass,
            source.path.clone(),
            Some(source.line),
            format!(
                "value {} reaches {} without crossing adapter policy {:?}",
                source.name, sink.function, policy.name
            ),
            metrics,
        )
        .with_related_locations(related),
    );
    detection.flow_witness = Some(witness);
    detection.normalize_flow_anchor();
    detection
}

fn witness_steps(path: &ExactPath, graph: &FlowGraph) -> Vec<FlowWitnessStep> {
    path.edges
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
        .collect()
}

fn path_measurements(
    path: &ExactPath,
    module_hops: usize,
    call_edges: usize,
    has_conforming_path: bool,
    bypass_count: usize,
) -> Vec<DetectedMeasurement> {
    vec![
        DetectedMeasurement::measurement(
            MetricId::FlowModuleHops,
            module_hops,
            "module transitions",
        ),
        DetectedMeasurement::measurement(MetricId::FlowCallEdges, call_edges, "call edges"),
        DetectedMeasurement::measurement(MetricId::FlowPathSteps, path.edges.len(), "steps"),
        DetectedMeasurement::measurement(MetricId::FlowUnresolvedEdges, 0, "edges"),
        DetectedMeasurement::measurement(
            MetricId::FlowPolicyConformingPaths,
            usize::from(has_conforming_path),
            "paths",
        ),
        DetectedMeasurement::measurement(MetricId::FlowPolicyBypassPaths, bypass_count, "paths"),
    ]
}

fn related_locations(path: &ExactPath, graph: &FlowGraph) -> Vec<RelatedLocation> {
    let mut locations = vec![location(&graph.nodes[path.source], "source")];
    for edge_index in &path.edges {
        let edge = &graph.edges[*edge_index];
        locations.push(location(&graph.nodes[edge.to], &edge.name));
    }
    locations
}

fn location(node: &FlowLocation, step: &str) -> RelatedLocation {
    RelatedLocation {
        path: node.path.clone(),
        line: node.line,
        name: Some(format!("{step}: {}", node.name)),
    }
}

fn module_hops(path: &ExactPath, graph: &FlowGraph) -> usize {
    path.edges
        .iter()
        .filter(|index| {
            let edge = &graph.edges[**index];
            graph.nodes[edge.from].module != graph.nodes[edge.to].module
        })
        .count()
}

fn normalize(path: &str) -> String {
    crate::pathing::normalize_path_text(path)
        .trim_start_matches("./")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_sets_match_display_paths_without_windows_verbatim_prefixes() -> Result<()> {
        let paths = PathSet::new(
            Path::new(r"\\?\C:\project"),
            &["src/application".to_string()],
        )?;

        assert!(paths.matches("C:/project/src/application/mod.rs"));
        assert!(!paths.matches("C:/project/src/transport.rs"));
        Ok(())
    }
}
