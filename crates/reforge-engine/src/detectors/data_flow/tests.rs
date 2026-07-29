use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::detectors::similarity::{SourceFile, parse_source_file};
use crate::model::{FlowEdgeKind, Rule};
use crate::scan::config::{DataFlowBoundaryConfig, DataFlowConfig, DataFlowSinkConfig};

use super::scan_data_flow;

const PROJECT_ROOT: &str = "/project";
const ACCEPTING_SINK: &str = "pub fn send(value: String) { let _accepted = value; }";

mod identity;
mod observation;
mod observe_fixtures;
mod resolution;

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
            language: "rust".into(),
            protected_paths: vec!["src/application".into()],
            adapter_paths: vec!["src/adapters/http".into()],
            sinks: vec![DataFlowSinkConfig {
                path: "src/transport.rs".into(),
                symbol: "crate::transport::send".into(),
            }],
            exempt_paths: Vec::new(),
        }],
        ..DataFlowConfig::default()
    }
}

fn dynamic_policy(language: &str, sink_path: &str, sink_symbol: &str) -> DataFlowConfig {
    DataFlowConfig {
        max_function_hops: 4,
        boundaries: vec![DataFlowBoundaryConfig {
            name: "dynamic-client".into(),
            language: language.into(),
            protected_paths: vec!["src/application*".into()],
            adapter_paths: vec!["src/adapters".into()],
            sinks: vec![DataFlowSinkConfig {
                path: sink_path.into(),
                symbol: sink_symbol.into(),
            }],
            exempt_paths: Vec::new(),
        }],
        ..DataFlowConfig::default()
    }
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
fn policy_sink_must_be_a_public_source_symbol() {
    let files = vec![
        parsed(
            "/project/src/application/mod.rs",
            "pub fn route(input: String) { crate::transport::send(input); }",
        ),
        parsed(
            "/project/src/transport.rs",
            "fn send(value: String) { let _accepted = value; }",
        ),
    ];
    let error = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &policy(4)).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("matched 0 public source symbols")
    );
}

#[test]
fn all_five_policy_languages_match_public_frontend_symbols() {
    let cases = [
        (
            "javascript",
            vec![
                parsed(
                    "/project/src/application.js",
                    "import { send } from './transport.js';\nexport function route(input) { send(input); }",
                ),
                parsed(
                    "/project/src/transport.js",
                    "export function send(value) { return value; }",
                ),
            ],
            "src/transport.js",
            "javascript:src/transport.js::send",
        ),
        (
            "python",
            vec![
                parsed(
                    "/project/src/application.py",
                    "from transport import send\ndef route(input):\n    send(input)\n",
                ),
                parsed(
                    "/project/src/transport.py",
                    "def send(value):\n    return value\n",
                ),
            ],
            "src/transport.py",
            "python:src/transport.py::send",
        ),
        (
            "typescript",
            vec![
                parsed(
                    "/project/src/application.ts",
                    "import { send } from './transport.ts';\nexport function route(input: string) { send(input); }",
                ),
                parsed(
                    "/project/src/transport.ts",
                    "export const send = (value: string): string => value;",
                ),
            ],
            "src/transport.ts",
            "typescript:src/transport.ts::send",
        ),
        (
            "tsx",
            vec![
                parsed(
                    "/project/src/application.tsx",
                    "import { send } from './transport.tsx';\nexport function route(input: string) { send(input); return <span>{input}</span>; }",
                ),
                parsed(
                    "/project/src/transport.tsx",
                    "export const send = (value: string): string => value;",
                ),
            ],
            "src/transport.tsx",
            "tsx:src/transport.tsx::send",
        ),
    ];
    for (language, files, path, symbol) in cases {
        let scan = scan_data_flow(
            Path::new(PROJECT_ROOT),
            &files,
            &[],
            &dynamic_policy(language, path, symbol),
        )
        .unwrap();
        assert_eq!(scan.detections.len(), 1, "{language}: {:?}", scan.summary);
        assert_eq!(
            scan.detections[0].kind,
            Rule::AdapterFlowBypass,
            "{language}"
        );
    }
}

#[test]
fn nested_dynamic_functions_resolve_in_their_lexical_scope() {
    let cases = [
        (
            "javascript",
            vec![
                parsed(
                    "/project/src/application.js",
                    "import { send } from './transport.js';\nexport function route(input) { const relay = function named(value) { send(value); }; relay(input); }",
                ),
                parsed(
                    "/project/src/transport.js",
                    "export const send = (value) => value;",
                ),
            ],
            "src/transport.js",
            "javascript:src/transport.js::send",
        ),
        (
            "python",
            vec![
                parsed(
                    "/project/src/application.py",
                    "from transport import send\ndef route(input):\n    def relay(value):\n        send(value)\n    relay(input)\n",
                ),
                parsed(
                    "/project/src/transport.py",
                    "def send(value):\n    return value\n",
                ),
            ],
            "src/transport.py",
            "python:src/transport.py::send",
        ),
    ];
    for (language, files, path, symbol) in cases {
        let scan = scan_data_flow(
            Path::new(PROJECT_ROOT),
            &files,
            &[],
            &dynamic_policy(language, path, symbol),
        )
        .unwrap();
        assert!(
            !scan.detections.is_empty(),
            "{language}: {:?}",
            scan.summary
        );
        assert!(
            scan.detections
                .iter()
                .flat_map(|detection| {
                    detection
                        .flow_witness
                        .as_ref()
                        .unwrap()
                        .ordered_steps
                        .iter()
                })
                .all(|step| step.resolution == crate::model::FlowResolution::Exact),
            "{language}"
        );
    }
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
    config.boundaries[0].sinks[0].path = "crates/api/src/transport.rs".into();

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
