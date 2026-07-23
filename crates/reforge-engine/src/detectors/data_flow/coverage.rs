use crate::model::FlowAnalysisSummary;

use super::model::FlowGraph;

pub(super) fn coverage_summary(graph: &FlowGraph, truncated_paths: usize) -> FlowAnalysisSummary {
    FlowAnalysisSummary {
        functions_analyzed: graph.functions.len(),
        exact_edges: graph.edges.len(),
        unresolved_edges: graph.unresolved_reasons.values().sum(),
        truncated_paths,
        policy_configured: false,
        protected_sources_evaluated: 0,
        relay_sources_evaluated: 0,
        program: None,
    }
}
