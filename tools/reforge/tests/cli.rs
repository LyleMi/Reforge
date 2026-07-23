use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_reforge"))
}

fn fixture(name: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("reforge-analyze-{name}-{suffix}"));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn first(x:String){second(x)} pub fn second(x:String){third(x)} pub fn third(x:String){drop(x)}\n",
    )
    .unwrap();
    root
}

fn analyze(root: &std::path::Path, analysis: Option<&str>, output: &std::path::Path) {
    let mut command = binary();
    command.args(["analyze", root.to_str().unwrap()]);
    if let Some(analysis) = analysis {
        command.args(["--analysis", analysis]);
    }
    let result = command
        .args([
            "--output",
            "json",
            "--output-file",
            output.to_str().unwrap(),
            "--reproducible",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn issue_ids(path: &std::path::Path) -> std::collections::BTreeSet<String> {
    let report: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| issue["id"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn help_exposes_the_single_analysis_vocabulary() {
    let output = binary().arg("--help").output().unwrap();
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    for command in ["analyze", "rules", "init", "config"] {
        assert!(help.contains(command));
    }
    assert!(help.contains("Codebase and Dataflow"));
    let analyze_help = binary().args(["analyze", "--help"]).output().unwrap();
    let analyze_help = String::from_utf8(analyze_help.stdout).unwrap();
    assert!(analyze_help.contains("--analysis"));
    assert!(!analyze_help.contains(&["--le", "ns"].concat()));
}

#[test]
fn combined_issue_set_is_the_union_and_inventory_is_not_duplicated() {
    let root = fixture("union");
    let codebase = root.join("codebase.json");
    let dataflow = root.join("dataflow.json");
    let combined = root.join("combined.json");
    analyze(&root, Some("codebase"), &codebase);
    analyze(&root, Some("dataflow"), &dataflow);
    let result = binary()
        .args([
            "analyze",
            root.to_str().unwrap(),
            "--analysis",
            "codebase",
            "--analysis",
            "dataflow",
            "--output",
            "json",
            "--output-file",
            combined.to_str().unwrap(),
            "--reproducible",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mut expected = issue_ids(&codebase);
    expected.extend(issue_ids(&dataflow));
    assert_eq!(issue_ids(&combined), expected);
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&combined).unwrap()).unwrap();
    assert_eq!(report["summary"]["scanned_files"], 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn default_report_contains_only_codebase_coverage() {
    let root = fixture("default-codebase");
    let output = root.join("default.json");
    analyze(&root, None, &output);
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();
    assert!(report["coverage"]["codebase"].is_object());
    assert!(report["coverage"]["dataflow"].is_null());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn removed_structure_vocabulary_is_rejected() {
    let root = fixture("removed-structure");
    let output = binary()
        .args(["analyze", root.to_str().unwrap(), "--analysis", "structure"])
        .output()
        .unwrap();
    assert!(!output.status.success());

    std::fs::write(
        root.join("reforge.toml"),
        "version = 1\n[analysis]\nenabled = [\"codebase\"]\n[structure]\nmax-file-lines = 10\n",
    )
    .unwrap();
    let output = binary()
        .args(["config", "validate", root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unknown configuration key `structure`")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn reproducible_json_is_byte_identical() {
    let root = fixture("reproducible");
    let first = root.join("first.json");
    let second = root.join("second.json");
    analyze(&root, None, &first);
    analyze(&root, None, &second);
    assert_eq!(
        std::fs::read(first).unwrap(),
        std::fs::read(second).unwrap()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn all_five_report_formats_are_emitted_from_the_same_command() {
    let root = fixture("formats");
    for (format, extension) in [
        ("human", "txt"),
        ("html", "html"),
        ("json", "json"),
        ("yaml", "yaml"),
        ("sarif", "sarif"),
    ] {
        let destination = root.join(format!("report.{extension}"));
        let output = binary()
            .args([
                "analyze",
                root.to_str().unwrap(),
                "--analysis",
                "codebase",
                "--output",
                format,
                "--output-file",
                destination.to_str().unwrap(),
                "--reproducible",
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "{format}");
        let bytes = std::fs::read(&destination).unwrap();
        assert!(!bytes.is_empty(), "{format}");
        match format {
            "json" | "sarif" => {
                let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                if format == "sarif" {
                    assert!(
                        value["runs"][0]["results"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .all(|result| result.get("level").is_none())
                    );
                }
            }
            "yaml" => {
                let _: serde_yaml::Value = serde_yaml::from_slice(&bytes).unwrap();
            }
            "html" => assert!(String::from_utf8_lossy(&bytes).contains("<title>Reforge report")),
            "human" => assert!(String::from_utf8_lossy(&bytes).contains("schema 26")),
            _ => unreachable!(),
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn baseline_with_a_different_analysis_set_is_rejected() {
    let root = fixture("baseline");
    let baseline = root.join("codebase.json");
    analyze(&root, Some("codebase"), &baseline);
    let output = binary()
        .args([
            "analyze",
            root.to_str().unwrap(),
            "--analysis",
            "dataflow",
            "--baseline",
            baseline.to_str().unwrap(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("analysis set does not match"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn old_producer_baseline_is_rejected_with_migration_guidance() {
    let root = fixture("old-baseline");
    let baseline = root.join("baseline.json");
    analyze(&root, Some("codebase"), &baseline);
    let mut report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&baseline).unwrap()).unwrap();
    report["producer"]["name"] = "reforge-scan.core".into();
    std::fs::write(&baseline, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    let output = binary()
        .args([
            "analyze",
            root.to_str().unwrap(),
            "--analysis",
            "codebase",
            "--baseline",
            baseline.to_str().unwrap(),
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(error.contains("baseline producer does not match"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn config_show_reports_effective_value_sources() {
    let root = fixture("config-show");
    let output = binary()
        .args([
            "config",
            "show",
            root.to_str().unwrap(),
            "--output",
            "json",
            "--set",
            "dataflow.search.max-path-steps=22",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let view: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(view["values"]["dataflow"]["search"]["max-path-steps"], 22);
    assert_eq!(
        view["sources"]["dataflow.search.max-path-steps"],
        "cli --set"
    );
    assert_eq!(
        view["sources"]["codebase.max-file-lines"],
        "built-in default"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn removed_commands_and_legacy_config_are_explicit_errors() {
    let root = fixture("config-migrate");
    std::fs::write(
        root.join("reforge-scan.toml"),
        "max-file-lines = 700\nignore-paths = ['vendor']\n",
    )
    .unwrap();
    std::fs::write(
        root.join("reforge-flow.toml"),
        "mode = 'observe'\nmax-hops = 4\nprotected-paths = []\nadapter-paths = []\nsink-symbols = []\nexemptions = []\nignore-paths = []\nsuppressions = []\n",
    )
    .unwrap();
    std::fs::write(
        root.join("reforge-unity.toml"),
        "[unity]\nmode = 'on'\nmax-scene-objects = 1200\n",
    )
    .unwrap();
    let migrated = binary()
        .args(["config", "migrate", root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!migrated.status.success());
    assert!(String::from_utf8_lossy(&migrated.stderr).contains("unrecognized subcommand"));
    for name in [
        "reforge-scan.toml",
        "reforge-flow.toml",
        "reforge-unity.toml",
    ] {
        assert!(root.join(name).is_file());
    }
    assert!(!root.join("reforge.toml").exists());
    let validated = binary()
        .args(["config", "validate", root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        validated.status.success(),
        "legacy files are not auto-discovered"
    );
    std::fs::remove_dir_all(root).unwrap();
}
