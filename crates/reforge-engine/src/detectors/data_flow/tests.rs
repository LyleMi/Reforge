use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::detectors::similarity::{SourceFile, parse_source_file};
use crate::model::{FlowEdgeKind, Rule};
use crate::scan::config::{DataFlowBoundaryConfig, DataFlowConfig};

use super::scan_data_flow;

const PROJECT_ROOT: &str = "/project";
const ACCEPTING_SINK: &str = "pub fn send(value: String) { let _accepted = value; }";

mod observe_fixtures;

fn parsed(path: &str, source: &str) -> crate::detectors::similarity::ParsedSourceFile {
    parse_source_file(SourceFile {
        path: PathBuf::from(path),
        display_path: path.to_string(),
        source: Arc::from(source),
    })
    .unwrap()
    .unwrap()
}

fn policy(max_hops: usize) -> DataFlowConfig {
    DataFlowConfig {
        max_function_hops: max_hops,
        boundaries: vec![DataFlowBoundaryConfig {
            name: "http-client".into(),
            protected_paths: vec!["src/application".into()],
            adapter_paths: vec!["src/adapters/http".into()],
            sink_symbols: vec!["crate::transport::send".into()],
            exempt_paths: Vec::new(),
        }],
        ..DataFlowConfig::default()
    }
}

#[test]
fn reports_exact_local_and_interprocedural_bypass_witness() {
    let files = vec![
        parsed(
            "/project/src/application/mod.rs",
            "pub fn route(input: String) { let alias = input; crate::transport::send(alias); }",
        ),
        parsed(
            "/project/src/transport.rs",
            "pub fn send(value: String) { let _accepted = value; }",
        ),
    ];
    let scan = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &policy(4)).unwrap();
    assert_eq!(scan.detections.len(), 1);
    let detection = &scan.detections[0];
    assert_eq!(detection.kind, Rule::AdapterFlowBypass);
    let witness = detection.flow_witness.as_ref().unwrap();
    assert_eq!(witness.source.name, "input");
    assert_eq!(witness.sink.name, "value");
    assert!(
        witness
            .ordered_steps
            .iter()
            .all(|step| step.resolution == crate::model::FlowResolution::Exact)
    );
    assert!(
        witness
            .ordered_steps
            .iter()
            .any(|step| step.kind == FlowEdgeKind::Assignment)
    );
}

#[test]
fn resolves_crate_root_callers_and_sinks_in_lib_and_main() {
    for root_file in ["lib.rs", "main.rs"] {
        let files = vec![
            parsed(
                &format!("/project/src/{root_file}"),
                "pub fn route(input: String) { crate::transport::send(input); }",
            ),
            parsed(
                "/project/src/transport.rs",
                "pub fn send(value: String) { let _accepted = value; }",
            ),
        ];
        let mut config = policy(4);
        config.boundaries[0].protected_paths = vec![format!("src/{root_file}")];

        let scan = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &config).unwrap();

        assert_eq!(scan.detections.len(), 1, "crate root {root_file}");
        let witness = scan.detections[0].flow_witness.as_ref().unwrap();
        assert_eq!(witness.source.function, "crate::route");
        assert_eq!(witness.sink.function, "crate::transport::send");
    }
}

#[test]
fn resolves_workspace_crates_without_cross_crate_symbol_collisions() -> anyhow::Result<()> {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let root = std::env::temp_dir().join(format!("reforge-flow-workspace-{suffix}"));
    let api = root.join("crates/api");
    let other = root.join("crates/other");
    std::fs::create_dir_all(api.join("src/application"))?;
    std::fs::create_dir_all(other.join("src"))?;
    std::fs::write(
        api.join("Cargo.toml"),
        "[package]\nname='api'\nversion='0.1.0'\n",
    )?;
    std::fs::write(
        other.join("Cargo.toml"),
        "[package]\nname='other'\nversion='0.1.0'\n",
    )?;

    let files = vec![
        parsed(
            &api.join("src/application/mod.rs").to_string_lossy(),
            "pub fn route(input: String) { crate::transport::send(input); }",
        ),
        parsed(
            &api.join("src/transport.rs").to_string_lossy(),
            ACCEPTING_SINK,
        ),
        parsed(
            &other.join("src/transport.rs").to_string_lossy(),
            ACCEPTING_SINK,
        ),
    ];
    let mut config = policy(4);
    config.boundaries[0].protected_paths = vec!["crates/api/src/application".into()];

    let scan = scan_data_flow(&root, &files, &[], &config)?;

    assert_eq!(scan.detections.len(), 1);
    assert_eq!(scan.summary.unresolved_edges, 0);
    assert_eq!(
        scan.detections[0]
            .flow_witness
            .as_ref()
            .unwrap()
            .sink
            .function,
        "crate::transport::send"
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn accepts_a_path_that_crosses_the_declared_adapter() {
    let files = vec![
        parsed(
            "/project/src/application/mod.rs",
            "pub fn route(input: String) { crate::adapters::http::deliver(input); }",
        ),
        parsed(
            "/project/src/adapters/http/mod.rs",
            "pub fn deliver(value: String) { crate::transport::send(value); }",
        ),
        parsed(
            "/project/src/transport.rs",
            "pub fn send(value: String) { let _accepted = value; }",
        ),
    ];
    let scan = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &policy(4)).unwrap();
    assert!(
        scan.detections
            .iter()
            .all(|detection| detection.kind != Rule::AdapterFlowBypass)
    );
}

#[test]
fn observes_without_emitting_policy_detections() {
    let files = vec![parsed(
        "/project/src/application/mod.rs",
        "pub fn route(input: String) { let alias = input; drop(alias); }",
    )];
    let config = DataFlowConfig {
        boundaries: Vec::new(),
        ..DataFlowConfig::default()
    };
    let scan = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &config).unwrap();
    assert!(scan.detections.is_empty());
    assert_eq!(scan.summary.functions_analyzed, 1);
}

fn observe_config() -> DataFlowConfig {
    DataFlowConfig {
        max_function_hops: 8,
        max_module_hops: 8,
        max_path_steps: 30,
        max_sinks_per_source: 100,
        ..DataFlowConfig::default()
    }
}

#[test]
fn relay_requires_all_inclusive_minimums() {
    let files = vec![
        parsed(
            "/project/src/root.py",
            "def root(x):\n    return first(x)\n",
        ),
        parsed(
            "/project/src/first.py",
            "def first(x):\n    return second(x)\n",
        ),
        parsed(
            "/project/src/second.py",
            "def second(x):\n    return third(x)\n",
        ),
        parsed(
            "/project/src/third.py",
            "def third(x):\n    return fourth(x)\n",
        ),
        parsed(
            "/project/src/fourth.py",
            "def fourth(x):\n    consumed = x\n",
        ),
    ];
    let scan = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &observe_config()).unwrap();
    let detection = scan
        .detections
        .iter()
        .find(|detection| detection.kind == Rule::ExcessiveRelay)
        .unwrap_or_else(|| panic!("missing relay detection: {scan:#?}"));
    let witness = detection.flow_witness.as_ref().unwrap();
    assert_eq!(witness.function_hops, 4);
    assert!(witness.module_hops >= 2);
    assert_eq!(witness.resolution, crate::model::FlowResolution::Exact);
}

#[test]
fn detects_flow_fan_out_with_ordered_witness() {
    let files = vec![
        parsed(
            "/project/src/fanout.py",
            "def root(value):\n    one(value)\n    two(value)\n    three(value)\n    four(value)\n",
        ),
        parsed("/project/src/one.py", "def one(value):\n    used = value\n"),
        parsed("/project/src/two.py", "def two(value):\n    used = value\n"),
        parsed(
            "/project/src/three.py",
            "def three(value):\n    used = value\n",
        ),
        parsed(
            "/project/src/four.py",
            "def four(value):\n    used = value\n",
        ),
    ];
    let scan = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &observe_config()).unwrap();
    let detection = scan
        .detections
        .iter()
        .find(|detection| detection.kind == Rule::FlowFanOut)
        .expect("fan-out should be reported");
    let witness = detection.flow_witness.as_ref().unwrap();
    assert_eq!(witness.source.name, "value");
    assert!(!witness.ordered_steps.is_empty());
    assert!(detection.metrics.iter().any(|metric| {
        metric.name == crate::model::MetricId::FlowSinkCount && metric.value == 4
    }));
}

#[test]
fn max_hops_truncation_is_reported_without_a_speculative_detection() {
    let files = vec![
        parsed(
            "/project/src/application/mod.rs",
            "pub fn route(input: String) { crate::middle::one(input); }",
        ),
        parsed(
            "/project/src/middle.rs",
            "pub fn one(value: String) { crate::middle::two(value); } pub fn two(value: String) { crate::transport::send(value); }",
        ),
        parsed(
            "/project/src/transport.rs",
            "pub fn send(value: String) { let _accepted = value; }",
        ),
    ];
    let scan = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &policy(1)).unwrap();
    assert!(
        scan.detections
            .iter()
            .all(|detection| detection.kind != Rule::AdapterFlowBypass)
    );
    assert!(scan.summary.truncated_paths > 0);
}

#[test]
fn formatting_and_comments_preserve_witness_identity() {
    let transport = parsed(
        "/project/src/transport.rs",
        "pub fn send(value: String) { let _accepted = value; }",
    );
    let first = scan_data_flow(
        Path::new(PROJECT_ROOT),
        &[
            parsed(
                "/project/src/application/mod.rs",
                "pub fn route(input: String) { crate::transport::send(input); }",
            ),
            transport.clone(),
        ],
        &[],
        &policy(4),
    )
    .unwrap();
    let second = scan_data_flow(
        Path::new(PROJECT_ROOT),
        &[
            parsed(
                "/project/src/application/mod.rs",
                "pub fn route(input: String) {\n  // routing\n  crate::transport::send(input);\n}",
            ),
            transport,
        ],
        &[],
        &policy(4),
    )
    .unwrap();
    assert_eq!(
        first.detections[0].semantic_anchor,
        second.detections[0].semantic_anchor
    );

    let wrapped = scan_data_flow(
        Path::new(PROJECT_ROOT),
        &[
            parsed(
                "/project/src/application/mod.rs",
                "pub fn route(input: String) { crate::wrapper::forward(input); }",
            ),
            parsed(
                "/project/src/wrapper.rs",
                "pub fn forward(value: String) { crate::transport::send(value); }",
            ),
            parsed(
                "/project/src/unrelated.rs",
                "pub fn untouched() -> usize { 1 }",
            ),
            parsed(
                "/project/src/transport.rs",
                "pub fn send(value: String) { let _accepted = value; }",
            ),
        ],
        &[],
        &policy(4),
    )
    .unwrap();
    assert_eq!(
        first.detections[0].semantic_anchor,
        wrapped.detections[0].semantic_anchor
    );
    assert_ne!(
        first.detections[0]
            .flow_witness
            .as_ref()
            .unwrap()
            .path_steps,
        wrapped.detections[0]
            .flow_witness
            .as_ref()
            .unwrap()
            .path_steps
    );

    assert_identity_survives_checkout_move(&first.detections[0].semantic_anchor);
}

fn assert_identity_survives_checkout_move(expected: &str) {
    let moved_root = scan_data_flow(
        Path::new("/other-checkout"),
        &[
            parsed(
                "/other-checkout/src/application/mod.rs",
                "pub fn route(input: String) { crate::transport::send(input); }",
            ),
            parsed(
                "/other-checkout/src/transport.rs",
                "pub fn send(value: String) { let _accepted = value; }",
            ),
        ],
        &[],
        &policy(4),
    )
    .unwrap();
    assert_eq!(expected, moved_root.detections[0].semantic_anchor);
}

#[test]
fn resolves_imports_reexports_self_and_super_paths() {
    let files = vec![
        parsed(
            "/project/src/application/mod.rs",
            "use crate::api::send; pub fn imported(input: String) { send(input); } pub fn local(input: String) { self::helper(input); } fn helper(value: String) { super::transport::send(value); }",
        ),
        parsed("/project/src/api.rs", "pub use crate::transport::send;"),
        parsed(
            "/project/src/transport.rs",
            "pub fn send(value: String) { let _accepted = value; }",
        ),
    ];
    let scan = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &policy(4)).unwrap();
    let mut source_functions = scan
        .detections
        .iter()
        .filter_map(|detection| detection.flow_witness.as_ref())
        .map(|witness| witness.source.function.as_str())
        .collect::<Vec<_>>();
    source_functions.sort_unstable();
    assert_eq!(
        source_functions,
        [
            "crate::application::helper",
            "crate::application::imported",
            "crate::application::local",
        ]
    );
    assert!(scan.detections.iter().all(|detection| {
        detection
            .flow_witness
            .as_ref()
            .is_some_and(|witness| witness.call_edges <= 2)
    }));
}

#[test]
fn stops_exact_flow_at_transforms_methods_and_macros() {
    let files = vec![
        parsed(
            "/project/src/application/mod.rs",
            "pub fn transformed(input: String) { crate::transport::send(normalize(input)); } pub fn method(input: String) { crate::transport::send(input.trim().to_string()); } pub fn macro_value(input: String) { crate::transport::send(format!(\"{input}\")); }",
        ),
        parsed(
            "/project/src/transport.rs",
            "pub fn send(value: String) { let _accepted = value; }",
        ),
    ];
    let scan = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &policy(4)).unwrap();
    assert!(scan.detections.is_empty());
    assert!(scan.summary.unresolved_edges >= 3);
}

#[test]
fn tuple_destructuring_shadowing_and_references_keep_exact_aliases() {
    let files = vec![
        parsed(
            "/project/src/application/mod.rs",
            "pub fn route(first: String, second: String) { let (chosen, _other) = (first, second); { let chosen = &chosen; crate::transport::send(chosen); } }",
        ),
        parsed(
            "/project/src/transport.rs",
            "pub fn send(value: &String) { let _accepted = value; }",
        ),
    ];
    let scan = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &policy(4)).unwrap();
    assert_eq!(scan.detections.len(), 1);
    assert_eq!(
        scan.detections[0]
            .flow_witness
            .as_ref()
            .unwrap()
            .source
            .name,
        "first"
    );
}

#[test]
fn ambiguous_targets_are_coverage_only() {
    let files = vec![
        parsed(
            "/project/src/application/mod.rs",
            "pub fn route(input: String) { crate::transport::send(input); }",
        ),
        parsed(
            "/project/src/transport.rs",
            "pub fn send(value: String) { let _accepted = value; }",
        ),
        parsed(
            "/project/src/transport.rs",
            "pub fn send(value: String) { drop(value); }",
        ),
    ];
    let scan = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &policy(4)).unwrap();
    assert!(scan.detections.is_empty());
    assert!(scan.summary.unresolved_edges > 0);
}

#[test]
fn destructured_parameters_and_control_merges_are_coverage_only() {
    let files = vec![
        parsed(
            "/project/src/application/mod.rs",
            "pub fn destructured(input: String) { crate::transport::pair((input, String::new())); } pub fn branch(input: String, flag: bool) { let value = if flag { input } else { String::new() }; crate::transport::send(value); }",
        ),
        parsed(
            "/project/src/transport.rs",
            "pub fn pair((first, second): (String, String)) { let _accepted = (first, second); } pub fn send(value: String) { let _accepted = value; }",
        ),
    ];
    let mut config = policy(4);
    config.boundaries[0]
        .sink_symbols
        .push("crate::transport::pair".into());
    let scan = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &config).unwrap();
    assert!(scan.detections.is_empty());
    assert!(scan.summary.unresolved_edges >= 2);
}
