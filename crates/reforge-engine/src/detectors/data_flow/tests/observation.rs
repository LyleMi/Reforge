use super::*;

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
