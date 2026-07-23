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

fn metric_value(detection: &DetectedEvidence, name: &str) -> Option<usize> {
    detection
        .metrics
        .iter()
        .find(|metric| metric.name.as_str() == name)
        .map(|metric| metric.value)
}

#[test]
fn detects_resolved_javascript_cycle() {
    let sources = vec![
        source("project/src/a.ts", "import { b } from './b';\n"),
        source("project/src/b.ts", "import { a } from './a';\n"),
    ];

    let scan = scan_dependency_graph_report(&sources, Path::new("project"));
    let detections = &scan.detections;

    let cycle = detections
        .iter()
        .find(|detection| detection.kind == Rule::DependencyCycle)
        .expect("cycle should be reported");
    assert_eq!(cycle.related_locations.len(), 2);
    assert_eq!(cycle.metrics[0].name, MetricId::DependencyCycleFiles);
    assert_eq!(metric_value(cycle, "dependency.cycle_edges"), Some(2));
    assert_eq!(
        metric_value(cycle, "dependency.cycle_density_percent"),
        Some(100)
    );
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
fn resolves_imports_from_vue_script_blocks() {
    let sources = vec![
        source(
            "project/src/App.vue",
            "<template><Widget /></template>\n<script setup lang=\"ts\">\nimport Widget from './Widget.vue';\n</script>\n",
        ),
        source(
            "project/src/Widget.vue",
            "<script setup lang=\"ts\">\nexport const label = 'widget';\n</script>\n",
        ),
    ];

    let scan = scan_dependency_graph_report(&sources, Path::new("project"));

    assert_eq!(scan.snapshot.edges.len(), 1, "{:#?}", scan.snapshot);
    assert_eq!(scan.snapshot.edges[0].from, "project/src/App.vue");
    assert_eq!(scan.snapshot.edges[0].to, "project/src/Widget.vue");
}

#[test]
fn resolves_rust_module_layouts_and_ignores_inline_modules() {
    let sources = vec![
        source(
            "project/src/main.rs",
            "mod direct;\nmod directory;\npub(crate) mod visible;\nmod inline { pub fn value() {} }\n",
        ),
        source("project/src/direct.rs", "mod child;\n"),
        source("project/src/direct/child.rs", "pub fn child() {}\n"),
        source("project/src/directory/mod.rs", "mod nested;\n"),
        source("project/src/directory/nested.rs", "pub fn nested() {}\n"),
        source("project/src/visible.rs", "pub fn visible() {}\n"),
        source(
            "project/src/custom.rs",
            "#[cfg(test)]\n#[path = \"../custom_tests.rs\"]\nmod tests;\n",
        ),
        source("project/custom_tests.rs", "#[test] fn works() {}\n"),
        source(
            "project/src/container.rs",
            "include!(\"fragments/types.rs\");\n",
        ),
        source("project/src/fragments/types.rs", "mod aggregation;\n"),
        source(
            "project/src/container/aggregation.rs",
            "pub fn aggregate() {}\n",
        ),
    ];

    let scan = scan_dependency_graph_report(&sources, Path::new("project"));

    assert_eq!(scan.unresolved_edges, 0, "{:#?}", scan.unresolved_by_file);
    assert_eq!(scan.snapshot.edges.len(), 8, "{:#?}", scan.snapshot);
}

#[test]
fn ignores_javascript_imports_of_non_source_assets() {
    let sources = vec![
        source(
            "project/src/main.tsx",
            "import { App } from './reportApp';\nimport './styles.css';\n",
        ),
        source("project/src/reportApp.tsx", "export const App = {};\n"),
    ];

    let scan = scan_dependency_graph_report(&sources, Path::new("project"));

    assert_eq!(scan.unresolved_edges, 0);
    assert_eq!(scan.snapshot.edges.len(), 1, "{:#?}", scan.snapshot);
}

#[test]
fn detects_resolved_csharp_namespace_cycle() {
    let sources = vec![
        source(
            "project/Assets/Core/A.cs",
            "using Project.Runtime;\nnamespace Project.Core { public class A { private B value; } }\n",
        ),
        source(
            "project/Assets/Runtime/B.cs",
            "using Project.Core;\nnamespace Project.Runtime;\npublic class B { private A value; }\n",
        ),
    ];

    let scan = scan_dependency_graph_report(&sources, Path::new("project"));

    assert_eq!(scan.snapshot.edges.len(), 2, "{:#?}", scan.snapshot);
    assert!(
        scan.detections
            .iter()
            .any(|detection| detection.kind == Rule::DependencyCycle)
    );
    assert_eq!(scan.unresolved_edges, 0);
}

#[test]
fn ignores_external_csharp_namespaces() {
    let sources = vec![source(
        "project/Assets/App.cs",
        "using System;\nusing UnityEngine;\nnamespace Project.App;\npublic class App {}\n",
    )];

    let scan = scan_dependency_graph_report(&sources, Path::new("project"));

    assert!(scan.snapshot.edges.is_empty());
    assert_eq!(scan.unresolved_edges, 0);
}

#[test]
fn ignores_csharp_types_mentioned_only_in_comments_and_strings() {
    let sources = vec![
        source(
            "project/Assets/Core/A.cs",
            "namespace Project.Core;\npublic class A {}\n",
        ),
        source(
            "project/Assets/App.cs",
            "namespace Project.App;\n// using Project.Core; A fake;\npublic class App { string text = \"Project.Core.A\"; }\n",
        ),
    ];

    let scan = scan_dependency_graph_report(&sources, Path::new("project"));

    assert!(scan.snapshot.edges.is_empty(), "{:#?}", scan.snapshot);
}

#[test]
fn ignores_unresolved_external_imports() {
    let sources = vec![source(
        "project/src/a.ts",
        "import express from 'express';\nimport local from './missing';\n",
    )];

    let scan = scan_dependency_graph_report(&sources, Path::new("project"));

    assert!(scan.detections.is_empty());
    assert_eq!(scan.unresolved_edges, 1);
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

    let detections = scan_dependency_graph(&sources, Path::new("project"));

    let hub = detections
        .iter()
        .find(|detection| detection.kind == Rule::DependencyHub)
        .expect("hub should be reported");
    assert_eq!(hub.path, "project/src/hub.ts");
    assert_eq!(metric_value(hub, "dependency.fan_out"), Some(6));
}

#[test]
fn ignores_rust_composition_roots_as_dependency_hubs() {
    let declarations = "mod a;\nmod b;\nmod c;\nmod d;\nmod e;\nmod f;\n";
    let mut sources = vec![source("project/src/facade/mod.rs", declarations)];
    for name in ["a", "b", "c", "d", "e", "f", "quiet"] {
        sources.push(source(
            &format!("project/src/facade/{name}.rs"),
            "pub const VALUE: usize = 1;\n",
        ));
    }

    let scan = scan_dependency_graph_report(&sources, Path::new("project"));

    assert_eq!(scan.unresolved_edges, 0);
    assert_eq!(scan.snapshot.edges.len(), 6);
    assert!(
        scan.detections
            .iter()
            .all(|detection| detection.kind != Rule::DependencyHub)
    );
    assert!(is_rust_composition_root("project/src/lib.rs"));
    assert!(is_rust_composition_root("project/src/main.rs"));
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

    let detections = scan_dependency_graph(&sources, Path::new("project"));

    let hub = detections
        .iter()
        .find(|detection| {
            detection.kind == Rule::DependencyHub && detection.path == "project/src/hub.ts"
        })
        .expect("hub should be reported");
    assert_eq!(metric_value(hub, "dependency.fan_out"), Some(6));
    assert_eq!(metric_value(hub, "dependency.fan_in"), Some(2));
    assert_eq!(metric_value(hub, "dependency.transitive_fan_out"), Some(8));
    assert_eq!(metric_value(hub, "dependency.depth"), Some(3));
    assert_eq!(
        metric_value(hub, "dependency.instability_percent"),
        Some(75)
    );
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
