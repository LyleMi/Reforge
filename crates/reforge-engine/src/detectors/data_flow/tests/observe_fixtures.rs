use super::*;

#[test]
fn dynamic_frontends_use_static_imports_to_disambiguate_project_calls() {
    let fixtures = [
        vec![
            parsed(
                "/project/src/root.js",
                "import { relay as chosen } from './right';\nfunction root(x){ return chosen(x); }",
            ),
            parsed(
                "/project/src/right.js",
                "function relay(x){ const used=x; }",
            ),
            parsed(
                "/project/src/wrong.js",
                "function relay(x){ const used=x; }",
            ),
        ],
        vec![
            parsed(
                "/project/src/root.py",
                "from right import relay as chosen\ndef root(x):\n    return chosen(x)\n",
            ),
            parsed("/project/src/right.py", "def relay(x):\n    used=x\n"),
            parsed("/project/src/wrong.py", "def relay(x):\n    used=x\n"),
        ],
    ];
    for files in fixtures {
        let language = files[0].file.path.display().to_string();
        let scan = scan_data_flow(Path::new(PROJECT_ROOT), &files, &[], &observe_config()).unwrap();
        assert_eq!(scan.summary.unresolved_edges, 0, "{language}");
    }
}

#[test]
fn dynamic_method_dispatch_is_a_language_specific_limitation() {
    for (path, source) in [
        (
            "/project/src/dynamic.js",
            "function route(x){ return client.send(x); }",
        ),
        (
            "/project/src/dynamic.py",
            "def route(x):\n    return client.send(x)\n",
        ),
    ] {
        let scan = scan_data_flow(
            Path::new(PROJECT_ROOT),
            &[parsed(path, source)],
            &[],
            &observe_config(),
        )
        .unwrap();
        assert!(scan.detections.is_empty());
        assert!(scan.summary.unresolved_edges > 0);
    }
}

#[test]
fn same_module_and_short_relays_do_not_report() {
    let same_module = [
        (
            "relay_a.rs",
            "fn a(x:String){b(x)} fn b(x:String){c(x)} fn c(x:String){drop(x)}",
        ),
        (
            "relay_b.rs",
            "fn d(x:String){e(x)} fn e(x:String){f(x)} fn f(x:String){drop(x)}",
        ),
        (
            "relay_c.js",
            "function g(x){return h(x)} function h(x){return i(x)} function i(x){const y=x}",
        ),
        (
            "relay_d.ts",
            "function j(x:string){return k(x)} function k(x:string){return l(x)} function l(x:string){const y=x}",
        ),
        (
            "relay_e.py",
            "def m(x):\n return n(x)\ndef n(x):\n return o(x)\ndef o(x):\n y=x\n",
        ),
    ];
    for (name, source) in same_module {
        let path = format!("/project/src/{name}");
        let scan = scan_data_flow(
            Path::new(PROJECT_ROOT),
            &[parsed(&path, source)],
            &[],
            &observe_config(),
        )
        .unwrap();
        assert!(
            scan.detections
                .iter()
                .all(|detection| detection.kind != Rule::ExcessiveRelay),
            "same-module microfixture {name}"
        );
    }

    let negatives = [
        ("short_a.rs", "fn a(x:String){b(x)} fn b(x:String){drop(x)}"),
        ("short_b.rs", "fn c(x:String){d(x)} fn d(x:String){drop(x)}"),
        (
            "short_c.js",
            "function e(x){return f(x)} function f(x){const y=x}",
        ),
        (
            "short_d.ts",
            "function g(x:string){return h(x)} function h(x:string){const y=x}",
        ),
        ("short_e.py", "def i(x):\n return j(x)\ndef j(x):\n y=x\n"),
    ];
    for (name, source) in negatives {
        let path = format!("/project/src/{name}");
        let scan = scan_data_flow(
            Path::new(PROJECT_ROOT),
            &[parsed(&path, source)],
            &[],
            &observe_config(),
        )
        .unwrap();
        assert!(
            scan.detections
                .iter()
                .all(|detection| detection.kind != Rule::ExcessiveRelay),
            "negative microfixture {name}"
        );
    }
}

#[test]
fn stable_fan_out_rule_has_five_positive_and_five_negative_microfixtures() {
    for case in 0..5 {
        let positive = fan_out_microfixture(case, 4);
        let scan =
            scan_data_flow(Path::new(PROJECT_ROOT), &positive, &[], &observe_config()).unwrap();
        assert!(
            scan.detections
                .iter()
                .any(|detection| detection.kind == Rule::FlowFanOut),
            "positive fan-out microfixture {case}"
        );

        let negative = fan_out_microfixture(case + 10, 3);
        let scan =
            scan_data_flow(Path::new(PROJECT_ROOT), &negative, &[], &observe_config()).unwrap();
        assert!(
            scan.detections
                .iter()
                .all(|detection| detection.kind != Rule::FlowFanOut),
            "negative fan-out microfixture {case}"
        );
    }
}

fn fan_out_microfixture(
    case: usize,
    sinks: usize,
) -> Vec<crate::detectors::similarity::ParsedSourceFile> {
    let calls = (0..sinks)
        .map(|sink| format!("    sink_{case}_{sink}(value)"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut files = vec![parsed(
        &format!("/project/src/root_{case}.py"),
        &format!("def root_{case}(value):\n{calls}\n"),
    )];
    files.extend((0..sinks).map(|sink| {
        parsed(
            &format!("/project/src/sink_{case}_{sink}.py"),
            &format!("def sink_{case}_{sink}(value):\n    used = value\n"),
        )
    }));
    files
}
