use super::*;
use crate::model::{FlowLocation, FlowNodeKind};

fn node(id: &str, function: &str) -> FlowLocation {
    FlowLocation {
        id: id.into(),
        kind: FlowNodeKind::Parameter,
        language: "rust".into(),
        path: "src/lib.rs".into(),
        line: 1,
        function: function.into(),
        module: function.into(),
        name: id.into(),
    }
}

#[test]
fn truncated_source_does_not_emit_exact_detection() {
    let mut graph = FlowGraph {
        nodes: vec![node("a", "a"), node("b", "b"), node("c", "c")],
        ..FlowGraph::default()
    };
    for (from, to) in [(0, 1), (1, 2)] {
        graph.add_edge(super::super::model::FlowEdge {
            from,
            to,
            kind: FlowEdgeKind::Assignment,
            resolution: FlowResolution::Exact,
            path: "src/lib.rs".into(),
            line: 1,
            name: "relay".into(),
            call_site: None,
            transition: CallTransition::None,
        });
    }
    graph.finish();
    let result = evaluate(
        &graph,
        &DataFlowConfig {
            max_path_steps: 1,
            ..DataFlowConfig::default()
        },
    );
    assert!(result.detections.is_empty());
    assert!(result.truncated_paths > 0);
}
