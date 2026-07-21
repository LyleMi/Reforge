mod compose;
mod coverage;
mod model;
mod policy;
mod rust;

use std::path::Path;

use anyhow::Result;

use crate::detectors::similarity::ParsedSourceFile;
use crate::model::{Finding, FlowAnalysisSummary, ParseFailure};
use crate::scan::config::{DataFlowConfig, DataFlowMode};

#[derive(Debug, Default)]
pub(crate) struct DataFlowScan {
    pub findings: Vec<Finding>,
    pub summary: FlowAnalysisSummary,
}

pub(crate) fn scan_data_flow(
    root: &Path,
    files: &[ParsedSourceFile],
    parse_failures: &[ParseFailure],
    config: &DataFlowConfig,
) -> Result<DataFlowScan> {
    if config.mode == DataFlowMode::Off {
        return Ok(DataFlowScan::default());
    }
    let graph = rust::build_graph(root, files);
    let policy_result = if config.mode == DataFlowMode::Policy {
        policy::evaluate_policies(root, &graph, &config.boundaries, config.max_hops)?
    } else {
        policy::PolicyResult {
            findings: Vec::new(),
            truncated_paths: 0,
        }
    };
    Ok(DataFlowScan {
        findings: policy_result.findings,
        summary: coverage::coverage_summary(
            &graph,
            true,
            policy_result.truncated_paths,
            parse_failures
                .iter()
                .filter(|failure| failure.language == "rust")
                .count(),
        ),
    })
}

#[cfg(test)]
mod tests;
