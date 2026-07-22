use std::collections::BTreeMap;
use std::path::Path;

use super::SourceScan;

pub(super) fn relativize_scan_paths(root: &Path, scan: &mut SourceScan) {
    relativize_findings(root, &mut scan.findings);
    relativize_sources(root, scan);
    relativize_metrics(root, &mut scan.raw_metrics);
    relativize_dependency_graph(root, scan);
    relativize_failures(root, scan);
}

fn relativize_findings(root: &Path, findings: &mut [crate::model::Finding]) {
    for finding in findings {
        relativize_finding(root, finding);
    }
}

fn relativize_finding(root: &Path, finding: &mut crate::model::Finding) {
    finding.path = relative_path(root, &finding.path);
    for location in &mut finding.related_locations {
        location.path = relative_path(root, &location.path);
    }
    if let Some(witness) = &mut finding.flow_witness {
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

fn relativize_sources(root: &Path, scan: &mut SourceScan) {
    for source in &mut scan.structure_sources {
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

fn relativize_dependency_graph(root: &Path, scan: &mut SourceScan) {
    for node in &mut scan.dependency_graph.nodes {
        node.path = relative_path(root, &node.path);
    }
    for edge in &mut scan.dependency_graph.edges {
        edge.from = relative_path(root, &edge.from);
        edge.to = relative_path(root, &edge.to);
    }
}

fn relativize_failures(root: &Path, scan: &mut SourceScan) {
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
    if !Path::new(&path).is_absolute() {
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
