use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::*;

fn source(path: &str, contents: &str) -> SourceFile {
    SourceFile {
        path: PathBuf::from(path),
        display_path: path.replace('\\', "/"),
        source: Arc::from(contents),
    }
}

fn metric_value(finding: &Finding, name: &str) -> Option<usize> {
    finding
        .metrics
        .iter()
        .find(|metric| metric.name == name)
        .map(|metric| metric.value)
}

#[test]
fn detects_resolved_javascript_cycle() {
    let sources = vec![
        source("project/src/a.ts", "import { b } from './b';\n"),
        source("project/src/b.ts", "import { a } from './a';\n"),
    ];

    let scan = scan_dependency_graph_report(&sources, Path::new("project"));
    let findings = &scan.findings;

    let cycle = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::DependencyCycle)
        .expect("cycle should be reported");
    assert_eq!(cycle.related_locations.len(), 2);
    assert_eq!(cycle.metrics[0].name, "cycle_files");
    assert_eq!(metric_value(cycle, "cycle_edges"), Some(2));
    assert_eq!(metric_value(cycle, "cycle_density_percent"), Some(100));
    assert_eq!(scan.snapshot.nodes.len(), 2);
    assert_eq!(scan.snapshot.edges.len(), 2);
    assert!(
        scan.snapshot
            .nodes
            .iter()
            .any(|node| node.path == "project/src/a.ts" && node.fan_in == 1 && node.fan_out == 1)
    );
    assert!(
        scan.snapshot
            .edges
            .iter()
            .any(|edge| edge.from == "project/src/a.ts" && edge.to == "project/src/b.ts")
    );
}

#[test]
fn ignores_unresolved_external_imports() {
    let sources = vec![source(
        "project/src/a.ts",
        "import express from 'express';\nimport local from './missing';\n",
    )];

    let findings = scan_dependency_graph(&sources, Path::new("project"));

    assert!(findings.is_empty());
}

#[test]
fn detects_dependency_hub_with_high_fan_out() {
    let mut sources = vec![source(
        "project/src/hub.ts",
        "import './a';\nimport './b';\nimport './c';\nimport './d';\nimport './e';\nimport './f';\n",
    )];
    for name in ["a", "b", "c", "d", "e", "f", "quiet"] {
        sources.push(source(
            &format!("project/src/{name}.ts"),
            "export const value = 1;\n",
        ));
    }

    let findings = scan_dependency_graph(&sources, Path::new("project"));

    let hub = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::DependencyHub)
        .expect("hub should be reported");
    assert_eq!(hub.path, "project/src/hub.ts");
    assert_eq!(metric_value(hub, "fan_out"), Some(6));
}

#[test]
fn reports_dependency_hub_graph_complexity_metrics() {
    let mut sources = vec![
        source(
            "project/src/hub.ts",
            "import './a';\nimport './b';\nimport './c';\nimport './d';\nimport './e';\nimport './f';\n",
        ),
        source("project/src/a.ts", "import './g';\n"),
        source("project/src/g.ts", "import './h';\n"),
        source("project/src/caller_one.ts", "import './hub';\n"),
        source("project/src/caller_two.ts", "import './hub';\n"),
    ];
    for name in ["b", "c", "d", "e", "f", "h"] {
        sources.push(source(
            &format!("project/src/{name}.ts"),
            "export const value = 1;\n",
        ));
    }

    let findings = scan_dependency_graph(&sources, Path::new("project"));

    let hub = findings
        .iter()
        .find(|finding| {
            finding.kind == FindingKind::DependencyHub && finding.path == "project/src/hub.ts"
        })
        .expect("hub should be reported");
    assert_eq!(metric_value(hub, "fan_out"), Some(6));
    assert_eq!(metric_value(hub, "fan_in"), Some(2));
    assert_eq!(metric_value(hub, "transitive_fan_out"), Some(8));
    assert_eq!(metric_value(hub, "dependency_depth"), Some(3));
    assert_eq!(metric_value(hub, "instability_percent"), Some(75));
}

#[test]
fn dependency_depth_collapses_cycles_before_measuring_paths() {
    let mut graph = DependencyGraph::default();
    graph.add_edge("hub".to_string(), "a".to_string());
    graph.add_edge("a".to_string(), "b".to_string());
    graph.add_edge("b".to_string(), "a".to_string());
    graph.add_edge("b".to_string(), "leaf".to_string());

    let depths = dependency_depths(&graph);

    assert_eq!(depths.get("hub"), Some(&2));
    assert_eq!(depths.get("a"), Some(&1));
    assert_eq!(depths.get("b"), Some(&1));
    assert_eq!(depths.get("leaf"), Some(&0));
}
