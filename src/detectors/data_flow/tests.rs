use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::detectors::similarity::{SourceFile, parse_source_file};
use crate::model::{FindingKind, FlowAnalysisStatus, FlowEdgeKind};
use crate::scan::config::{DataFlowBoundaryConfig, DataFlowConfig, DataFlowMode};

use super::scan_data_flow;

const PROJECT_ROOT: &str = "/project";

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
        mode: DataFlowMode::Policy,
        max_hops,
        boundaries: vec![DataFlowBoundaryConfig {
            name: "http-client".into(),
            protected_paths: vec!["src/application".into()],
            adapter_paths: vec!["src/adapters/http".into()],
            sink_symbols: vec!["crate::transport::send".into()],
            exempt_paths: Vec::new(),
        }],
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
    assert_eq!(scan.findings.len(), 1);
    let finding = &scan.findings[0];
    assert_eq!(finding.kind, FindingKind::AdapterFlowBypass);
    let witness = finding.flow_witness.as_ref().unwrap();
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

        assert_eq!(scan.findings.len(), 1, "crate root {root_file}");
        let witness = scan.findings[0].flow_witness.as_ref().unwrap();
        assert_eq!(witness.source.function, "crate::route");
        assert_eq!(witness.sink.function, "crate::transport::send");
    }
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
    assert!(scan.findings.is_empty());
    assert_eq!(scan.summary.status, FlowAnalysisStatus::Observed);
}

#[test]
fn observes_without_emitting_policy_findings() {
    let files = vec![parsed(
        "/project/src/application/mod.rs",
        "pub fn route(input: String) { let alias = input; drop(alias); }",
    )];
    let config = DataFlowConfig {
        mode: DataFlowMode::Observe,
        max_hops: 4,
        boundaries: Vec::new(),
    };
    let scan = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &config).unwrap();
    assert!(scan.findings.is_empty());
    assert_eq!(scan.summary.functions_analyzed, 1);
    assert_eq!(scan.summary.status, FlowAnalysisStatus::Partial);
}

#[test]
fn max_hops_truncation_is_reported_without_a_speculative_finding() {
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
    assert!(scan.findings.is_empty());
    assert!(scan.summary.truncated_paths > 0);
    assert_eq!(scan.summary.status, FlowAnalysisStatus::Partial);
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
    assert_eq!(first.findings[0].id, second.findings[0].id);

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
    assert_eq!(first.findings[0].id, wrapped.findings[0].id);
    assert_ne!(
        first.findings[0].flow_witness.as_ref().unwrap().path_steps,
        wrapped.findings[0]
            .flow_witness
            .as_ref()
            .unwrap()
            .path_steps
    );

    assert_identity_survives_checkout_move(&first.findings[0].id);
}

fn assert_identity_survives_checkout_move(expected: &crate::model::EvidenceId) {
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
    assert_eq!(expected, &moved_root.findings[0].id);
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
    assert_eq!(scan.findings.len(), 2);
    assert!(scan.findings.iter().all(|finding| {
        finding
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
    assert!(scan.findings.is_empty());
    assert_eq!(scan.summary.status, FlowAnalysisStatus::Partial);
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
    assert_eq!(scan.findings.len(), 1);
    assert_eq!(
        scan.findings[0].flow_witness.as_ref().unwrap().source.name,
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
    assert!(scan.findings.is_empty());
    assert_eq!(scan.summary.status, FlowAnalysisStatus::Partial);
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
    assert!(scan.findings.is_empty());
    assert_eq!(scan.summary.status, FlowAnalysisStatus::Partial);
    assert!(scan.summary.unresolved_edges >= 2);
}
