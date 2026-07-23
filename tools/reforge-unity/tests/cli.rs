use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn root(name: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("reforge-unity-{name}-{suffix}"));
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn optional_analyzer_fails_fast_outside_a_unity_root() {
    let root = root("not-root");
    let output = Command::new(env!("CARGO_BIN_EXE_reforge-unity"))
        .args(["analyze", root.to_str().unwrap(), "--output", "json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not a Unity project root"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn optional_analyzer_uses_the_unified_producer_and_namespace() {
    let root = root("root");
    std::fs::create_dir_all(root.join("Assets")).unwrap();
    std::fs::create_dir_all(root.join("ProjectSettings")).unwrap();
    std::fs::write(
        root.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 2022.3.0f1\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_reforge-unity"))
        .args([
            "analyze",
            root.to_str().unwrap(),
            "--output",
            "json",
            "--reproducible",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["producer"]["name"], "reforge.unity");
    assert!(report["coverage"]["unity"].is_object());
    assert!(report["coverage"]["codebase"].is_null());
    assert!(report["coverage"]["dataflow"].is_null());
    assert!(report["issues"].as_array().unwrap().iter().all(|issue| {
        issue["analysis"] == "unity"
            && issue["family"]
                .as_str()
                .is_some_and(|family| family.starts_with("reforge.unity."))
    }));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rules_exposes_unity_observation_contracts() {
    let output = Command::new(env!("CARGO_BIN_EXE_reforge-unity"))
        .args(["rules", "--output", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let rules: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(rules.as_array().is_some_and(|rules| {
        !rules.is_empty() && rules.iter().all(|rule| rule["observation"].is_object())
    }));
}
