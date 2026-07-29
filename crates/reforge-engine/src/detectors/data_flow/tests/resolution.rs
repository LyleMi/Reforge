use super::*;

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
    let error = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &policy(4)).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("matched 2 public source symbols")
    );
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
    config.boundaries[0].sinks.push(DataFlowSinkConfig {
        path: "src/transport.rs".into(),
        symbol: "crate::transport::pair".into(),
    });
    let scan = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &config).unwrap();
    assert!(scan.detections.is_empty());
    assert!(scan.summary.unresolved_edges >= 2);
}
