use std::collections::BTreeMap;
use std::path::Path;

use super::WorkspaceIndex;

pub(super) fn relativize_scan_paths(root: &Path, scan: &mut WorkspaceIndex) {
    relativize_detections(root, &mut scan.detections);
    relativize_sources(root, scan);
    relativize_metrics(root, &mut scan.raw_metrics);
    relativize_dependency_graph(root, scan);
    relativize_failures(root, scan);
    relativize_flow_program(root, scan);
}

fn relativize_flow_program(root: &Path, scan: &mut WorkspaceIndex) {
    let Some(program) = &mut scan.flow_analysis.program else {
        return;
    };
    let node_ids = relativize_flow_nodes(root, &mut program.nodes);
    remap_ids(&mut program.sources, &node_ids);
    remap_ids(&mut program.sinks, &node_ids);
    let edge_ids = relativize_flow_edges(root, &mut program.edges, &node_ids);
    remap_ids(&mut program.mutations, &edge_ids);
    remap_ids(&mut program.transformations, &edge_ids);
}

fn relativize_flow_nodes(
    root: &Path,
    nodes: &mut [crate::model::FlowLocation],
) -> BTreeMap<String, String> {
    let mut node_ids = BTreeMap::new();
    for (index, node) in nodes.iter_mut().enumerate() {
        node.path = relative_path(root, &node.path);
        let id = format!(
            "flow-node:{}:{}:{}:{}:{index}",
            node.path, node.line, node.function, node.name
        );
        node_ids.insert(std::mem::replace(&mut node.id, id.clone()), id);
    }
    node_ids
}

fn relativize_flow_edges(
    root: &Path,
    edges: &mut [crate::model::FlowProgramEdge],
    node_ids: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut edge_ids = BTreeMap::new();
    for (index, edge) in edges.iter_mut().enumerate() {
        edge.path = relative_path(root, &edge.path);
        if let Some(relative) = node_ids.get(&edge.from) {
            edge.from = relative.clone();
        }
        if let Some(relative) = node_ids.get(&edge.to) {
            edge.to = relative.clone();
        }
        let id = format!("flow-edge:{}:{}:{index}", edge.from, edge.to);
        edge_ids.insert(std::mem::replace(&mut edge.id, id.clone()), id);
    }
    edge_ids
}

fn remap_ids(ids: &mut [String], replacements: &BTreeMap<String, String>) {
    for id in ids {
        if let Some(relative) = replacements.get(id) {
            *id = relative.clone();
        }
    }
}

fn relativize_detections(root: &Path, detections: &mut [crate::model::DetectedEvidence]) {
    for detection in detections {
        relativize_detection(root, detection);
    }
}

fn relativize_detection(root: &Path, detection: &mut crate::model::DetectedEvidence) {
    detection.path = relative_path(root, &detection.path);
    for location in &mut detection.related_locations {
        location.path = relative_path(root, &location.path);
    }
    if let Some(witness) = &mut detection.flow_witness {
        relativize_witness(root, witness);
    }
}

fn relativize_witness(root: &Path, witness: &mut crate::model::FlowWitness) {
    witness.source.path = relative_path(root, &witness.source.path);
    witness.sink.path = relative_path(root, &witness.sink.path);
    for step in &mut witness.ordered_steps {
        step.path = relative_path(root, &step.path);
    }
    if let Some(path) = &mut witness.conforming_path {
        for location in path {
            location.path = relative_path(root, &location.path);
        }
    }
}

fn relativize_sources(root: &Path, scan: &mut WorkspaceIndex) {
    for source in &mut scan.codebase_sources {
        source.display_path = relative_path(root, &source.display_path);
    }
    for source in &mut scan.parsed_sources {
        source.file.display_path = relative_path(root, &source.file.display_path);
    }
}

fn relativize_metrics(root: &Path, metrics: &mut crate::model::RawMetrics) {
    for metric in &mut metrics.directories {
        metric.path = relative_path(root, &metric.path);
    }
    for metric in &mut metrics.files {
        metric.path = relative_path(root, &metric.path);
    }
    for metric in &mut metrics.functions {
        metric.path = relative_path(root, &metric.path);
    }
    for metric in &mut metrics.types {
        metric.path = relative_path(root, &metric.path);
    }
}

fn relativize_dependency_graph(root: &Path, scan: &mut WorkspaceIndex) {
    for node in &mut scan.dependency_graph.nodes {
        node.path = relative_path(root, &node.path);
    }
    for edge in &mut scan.dependency_graph.edges {
        edge.from = relative_path(root, &edge.from);
        edge.to = relative_path(root, &edge.to);
    }
}

fn relativize_failures(root: &Path, scan: &mut WorkspaceIndex) {
    for failure in &mut scan.parse_failures {
        failure.path = relative_path(root, &failure.path);
    }
    for failure in &mut scan.source_failures {
        failure.path = relative_path(root, &failure.path);
    }
    scan.unresolved_dependency_edges_by_file =
        std::mem::take(&mut scan.unresolved_dependency_edges_by_file)
            .into_iter()
            .map(|(path, count)| (relative_path(root, &path), count))
            .collect::<BTreeMap<_, _>>();
}

fn relative_path(root: &Path, path: &str) -> String {
    let path = crate::pathing::normalize_path_text(path);
    let is_windows_absolute = path.as_bytes().get(1) == Some(&b':') || path.starts_with("//");
    if !Path::new(&path).is_absolute() && !is_windows_absolute {
        return path.trim_start_matches("./").to_string();
    }
    let base = if root.is_file() {
        root.parent().unwrap_or(root)
    } else {
        root
    };
    let base = crate::pathing::display_path(base)
        .trim_end_matches('/')
        .to_string();
    if path == base {
        return ".".into();
    }
    path.strip_prefix(&base)
        .and_then(|suffix| suffix.strip_prefix('/'))
        .unwrap_or(&path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_checkout_paths_relative_and_uses_forward_slashes() {
        let root = Path::new(r"C:\work\repo");
        assert_eq!(
            relative_path(root, r"C:\work\repo\src\main.rs"),
            "src/main.rs"
        );
        assert_eq!(
            relative_path(root, "already/relative.rs"),
            "already/relative.rs"
        );
    }
}
