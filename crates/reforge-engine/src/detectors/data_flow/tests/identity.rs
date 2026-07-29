use super::*;

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
