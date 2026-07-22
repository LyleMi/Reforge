use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should follow the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("reforge-cli-{label}-{nonce}"));
        fs::create_dir_all(&path).expect("temporary directory should be created");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_reforge"))
        .args(args)
        .output()
        .expect("reforge process should start")
}

fn scan_json(root: &Path, extra: &[&str]) -> Output {
    let root = root.to_string_lossy();
    let mut args = vec![
        "scan",
        root.as_ref(),
        "--output",
        "json",
        "--churn",
        "off",
        "--reproducible",
        "--progress",
        "never",
    ];
    args.extend_from_slice(extra);
    run(&args)
}

fn json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "scan failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("stdout should be a JSON report")
}

fn write_long_function(root: &Path, leading: &str) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/lib.rs"),
        format!(
            "{leading}fn stable_name() {{\n    let a = 1;\n    let b = 2;\n    let _ = a + b;\n}}\n"
        ),
    )
    .unwrap();
}

#[test]
fn json_uses_stdout_and_progress_uses_stderr() {
    let root = TempDir::new("streams");
    write_long_function(root.path(), "");
    let root_text = root.path().to_string_lossy();
    let output = run(&[
        "scan",
        root_text.as_ref(),
        "--output",
        "json",
        "--churn",
        "off",
        "--progress",
        "always",
    ]);

    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema_version"], 24);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Scanning"));
    assert!(stderr.contains("Finished scan"));
}

#[test]
fn baseline_gate_writes_report_before_returning_failure() {
    let root = TempDir::new("gate");
    fs::create_dir_all(root.path().join("src")).unwrap();
    let baseline_path = root.path().join("baseline.json");
    let baseline = scan_json(
        root.path(),
        &["--max-function-lines", "1", "--only", "long_function"],
    );
    fs::write(&baseline_path, &baseline.stdout).unwrap();
    write_long_function(root.path(), "");
    let output_path = root.path().join("current.json");
    let root_text = root.path().to_string_lossy();
    let baseline_text = baseline_path.to_string_lossy();
    let output_text = output_path.to_string_lossy();

    let output = run(&[
        "scan",
        root_text.as_ref(),
        "--max-function-lines",
        "1",
        "--only",
        "long_function",
        "--baseline",
        baseline_text.as_ref(),
        "--baseline-mode",
        "new",
        "--fail-on-findings",
        "--output",
        "json",
        "--output-file",
        output_text.as_ref(),
        "--churn",
        "off",
        "--progress",
        "never",
    ]);

    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&fs::read(output_path).unwrap()).unwrap();
    assert_eq!(report["schema_version"], 24);
    assert!(
        !report["baseline_comparison"]["findings"]["added"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn schema_23_baseline_is_rejected_with_regeneration_guidance() {
    let root = TempDir::new("old-baseline");
    let baseline = root.path().join("baseline.json");
    fs::write(&baseline, r#"{"schema_version":23}"#).unwrap();
    let root_text = root.path().to_string_lossy();
    let baseline_text = baseline.to_string_lossy();
    let output = run(&[
        "scan",
        root_text.as_ref(),
        "--baseline",
        baseline_text.as_ref(),
        "--output",
        "json",
        "--progress",
        "never",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("schema 23 baselines are incompatible"));
    assert!(stderr.contains("regenerated"));
}

#[test]
fn stable_ids_and_paths_match_across_checkouts_and_ignore_leading_comments() {
    let first = TempDir::new("checkout-a");
    let second = TempDir::new("checkout-b");
    write_long_function(first.path(), "");
    write_long_function(second.path(), "// inserted comment\n\n");

    let first_report = json(&scan_json(
        first.path(),
        &["--max-function-lines", "1", "--only", "long_function"],
    ));
    let second_report = json(&scan_json(
        second.path(),
        &["--max-function-lines", "1", "--only", "long_function"],
    ));
    let first_finding = &first_report["findings"][0];
    let second_finding = &second_report["findings"][0];

    assert_eq!(first_finding["path"], "src/lib.rs");
    assert_eq!(first_finding["path"], second_finding["path"]);
    assert_eq!(first_finding["anchor"], second_finding["anchor"]);
    assert_eq!(first_finding["id"], second_finding["id"]);
    assert_eq!(
        first_report["issues"][0]["id"],
        second_report["issues"][0]["id"]
    );
    assert!(first_finding["id"].as_str().unwrap().starts_with("rf4-"));
    assert!(
        first_report["issues"][0]["id"]
            .as_str()
            .unwrap()
            .starts_with("ri4-")
    );
}

#[test]
fn invalid_source_is_partial_and_reproducible_instead_of_fatal() {
    let root = TempDir::new("invalid-source");
    write_long_function(root.path(), "");
    fs::write(root.path().join("src/bad.rs"), [0x80, 0x81]).unwrap();

    let first = scan_json(root.path(), &[]);
    let second = scan_json(root.path(), &[]);
    let report = json(&first);

    assert_eq!(first.stdout, second.stdout);
    assert_eq!(report["stats"]["source_files_discovered"], 2);
    assert_eq!(report["stats"]["source_files_analyzed"], 1);
    assert_eq!(
        report["coverage_summary"]["source_failures"][0]["path"],
        "src/bad.rs"
    );
    assert_eq!(
        report["coverage_summary"]["source_failures"][0]["reason"],
        "unsupported_encoding"
    );
    assert!(
        report["coverage_manifest"]
            .as_array()
            .unwrap()
            .iter()
            .any(|cell| cell["status"] == "partially_observed")
    );
}

#[test]
fn overloaded_symbols_and_distinct_groups_do_not_collide() {
    let overloads = TempDir::new("overloads");
    fs::write(
        overloads.path().join("types.cs"),
        "class A {\n void Run() {\n  int a = 1;\n  int b = 2;\n  int c = a + b;\n }\n}\nclass B {\n void Run() {\n  int a = 1;\n  int b = 2;\n  int c = a + b;\n }\n}\n",
    )
    .unwrap();
    let overload_report = json(&scan_json(
        overloads.path(),
        &["--max-function-lines", "1", "--only", "long_function"],
    ));
    let overload_findings = overload_report["findings"].as_array().unwrap();
    assert_eq!(overload_findings.len(), 2);
    assert_ne!(
        overload_findings[0]["anchor"],
        overload_findings[1]["anchor"]
    );
    assert_ne!(overload_findings[0]["id"], overload_findings[1]["id"]);

    let groups = TempDir::new("groups");
    fs::write(
        groups.path().join("groups.rs"),
        r#"
fn a1(x: i32) -> i32 { let y = x + 1; y * 2 }
fn a2(x: i32) -> i32 { let y = x + 1; y * 2 }
fn a3(x: i32) -> i32 { let y = x + 1; y * 2 }
fn b1(x: i32) -> i32 { if x > 0 { x } else { -x } }
fn b2(x: i32) -> i32 { if x > 0 { x } else { -x } }
fn b3(x: i32) -> i32 { if x > 0 { x } else { -x } }
"#,
    )
    .unwrap();
    let group_report = json(&scan_json(
        groups.path(),
        &[
            "--min-function-tokens",
            "1",
            "--min-similar-functions",
            "3",
            "--function-similarity",
            "1",
            "--only",
            "similar_functions",
        ],
    ));
    let group_findings = group_report["findings"].as_array().unwrap();
    assert_eq!(group_findings.len(), 2);
    assert_ne!(group_findings[0]["anchor"], group_findings[1]["anchor"]);
    assert_ne!(group_findings[0]["id"], group_findings[1]["id"]);
}
