use crate::model::{FlowAnalysisStatus, FlowAnalysisSummary, FlowCapability, FlowCapabilityStatus};

use super::model::FlowGraph;

pub(super) fn coverage_summary(
    graph: &FlowGraph,
    enabled: bool,
    truncated_paths: usize,
    rust_parse_failures: usize,
) -> FlowAnalysisSummary {
    if !enabled {
        return FlowAnalysisSummary::default();
    }
    let unresolved_edges = graph.unresolved_reasons.values().sum();
    let mut reasons = graph
        .unresolved_reasons
        .iter()
        .map(|(reason, count)| format!("{count} occurrences: {reason}"))
        .collect::<Vec<_>>();
    if rust_parse_failures > 0 {
        reasons.push(format!(
            "{rust_parse_failures} Rust source files failed syntax parsing"
        ));
    }
    FlowAnalysisSummary {
        status: if unresolved_edges == 0 && truncated_paths == 0 && rust_parse_failures == 0 {
            FlowAnalysisStatus::Observed
        } else {
            FlowAnalysisStatus::Partial
        },
        functions_analyzed: graph.functions.len(),
        exact_edges: graph.edges.len(),
        unresolved_edges,
        truncated_paths,
        capabilities: vec![FlowCapability {
            language: "rust".into(),
            local_def_use: FlowCapabilityStatus::Supported,
            direct_calls: if unresolved_edges == 0 {
                FlowCapabilityStatus::Supported
            } else {
                FlowCapabilityStatus::Partial
            },
            fields: FlowCapabilityStatus::Unsupported,
            dynamic_dispatch: FlowCapabilityStatus::Unsupported,
            library_models: FlowCapabilityStatus::Unsupported,
            reasons,
        }],
    }
}
