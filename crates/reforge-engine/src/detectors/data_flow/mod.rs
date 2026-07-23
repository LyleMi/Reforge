mod compose;
mod coverage;
mod dynamic;
mod model;
mod observe;
mod policy;
mod rust;

use std::path::Path;

use anyhow::Result;

use crate::detectors::similarity::ParsedSourceFile;
use crate::model::{DetectedEvidence, FlowAnalysisSummary, ParseFailure};
use crate::scan::config::DataFlowConfig;

#[derive(Debug, Default)]
pub(crate) struct DataFlowScan {
    pub detections: Vec<DetectedEvidence>,
    pub summary: FlowAnalysisSummary,
}

#[cfg(test)]
pub(crate) fn scan_data_flow(
    root: &Path,
    files: &[ParsedSourceFile],
    parse_failures: &[ParseFailure],
    config: &DataFlowConfig,
) -> Result<DataFlowScan> {
    scan_data_flow_with_ir(root, files, parse_failures, config, false)
}

pub(crate) fn scan_data_flow_with_ir(
    root: &Path,
    files: &[ParsedSourceFile],
    _parse_failures: &[ParseFailure],
    config: &DataFlowConfig,
    materialize_flow_ir: bool,
) -> Result<DataFlowScan> {
    let mut graph = rust::build_graph(root, files);
    dynamic::extend_graph(root, files, &mut graph);
    graph.finish();
    let observe_result = observe::evaluate(&graph, config);
    let policy_result = if !config.boundaries.is_empty() {
        policy::evaluate_policies(root, &graph, &config.boundaries, config.max_function_hops)?
    } else {
        policy::PolicyResult {
            detections: Vec::new(),
            truncated_paths: 0,
            protected_sources: 0,
        }
    };
    let mut detections = observe_result.detections;
    detections.extend(policy_result.detections);
    detections.sort_by(|left, right| left.semantic_anchor.cmp(&right.semantic_anchor));
    let mut summary = coverage::coverage_summary(
        &graph,
        policy_result.truncated_paths + observe_result.truncated_paths,
    );
    summary.policy_configured = !config.boundaries.is_empty();
    summary.protected_sources_evaluated = policy_result.protected_sources;
    summary.relay_sources_evaluated = observe_result.evaluated_sources;
    if materialize_flow_ir {
        summary.program = Some(model::program_snapshot(&graph));
    }
    Ok(DataFlowScan {
        detections,
        summary,
    })
}

#[cfg(test)]
mod tests;
