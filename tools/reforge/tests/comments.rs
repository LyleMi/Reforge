use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct TestWorkspace(PathBuf);
impl TestWorkspace {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "reforge-comments-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("main.ts"), "// remove template\nfunction run() { return 1; }\n\n/** remove docs */\nfunction documented() {}\n").unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_reforge"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn picks_json_and_previews_then_applies_saved_plan() {
    let f = TestWorkspace::new();
    let original = fs::read(f.0.join("main.ts")).unwrap();
    let picked: serde_json::Value =
        serde_json::from_str(&success(f.run(&["comments", "pick", "--output", "json"]))).unwrap();
    assert_eq!(picked["comments"].as_array().unwrap().len(), 2);
    assert_eq!(picked["comments"][1]["protected"], true);
    let diff = success(f.run(&["comments", "clean", "--contains", "remove"]));
    assert!(diff.contains("-// remove template"));
    assert_eq!(fs::read(f.0.join("main.ts")).unwrap(), original);
    success(f.run(&[
        "comments",
        "clean",
        "--contains",
        "remove",
        "--output",
        "json",
        "--output-file",
        "plan.json",
    ]));
    success(f.run(&["comments", "clean", "--plan", "plan.json", "--apply"]));
    let text = fs::read_to_string(f.0.join("main.ts")).unwrap();
    assert!(!text.contains("remove template"));
    assert!(text.contains("remove docs"));
    assert!(
        !f.run(&["comments", "clean", "--plan", "plan.json", "--apply"])
            .status
            .success()
    );
}

#[test]
fn rejects_implicit_selection_empty_selectors_conflicting_plan_and_output_overwrite() {
    let f = TestWorkspace::new();
    for args in [
        vec!["comments", "clean", "--apply"],
        vec!["comments", "clean", "--contains", ""],
        vec!["comments", "clean", "--candidates", "--apply"],
        vec![
            "comments",
            "clean",
            "--plan",
            "plan.json",
            "--contains",
            "remove",
        ],
        vec!["comments", "pick", "--output-file", "main.ts"],
    ] {
        assert!(!f.run(&args).status.success(), "{args:?}");
    }
    fs::write(f.0.join("existing.json"), "keep").unwrap();
    assert!(
        !f.run(&[
            "comments",
            "pick",
            "--output",
            "json",
            "--output-file",
            "existing.json"
        ])
        .status
        .success()
    );
    assert_eq!(
        fs::read_to_string(f.0.join("existing.json")).unwrap(),
        "keep"
    );
}

#[test]
fn selectors_and_config_scope_are_honored() {
    let f = TestWorkspace::new();
    fs::write(
        f.0.join("reforge.toml"),
        "version = 2\n[scope]\nignore-paths = [\"main.ts\"]\n",
    )
    .unwrap();
    let output: serde_json::Value =
        serde_json::from_str(&success(f.run(&["comments", "pick", "--output", "json"]))).unwrap();
    assert!(output["comments"].as_array().unwrap().is_empty());
    fs::remove_file(f.0.join("reforge.toml")).unwrap();
    let output: serde_json::Value = serde_json::from_str(&success(f.run(&[
        "comments",
        "pick",
        "--kind",
        "line",
        "--text",
        "remove template",
        "--output",
        "json",
    ])))
    .unwrap();
    let id = output["comments"][0]["id"].as_str().unwrap();
    success(f.run(&["comments", "clean", "--id", id, "--apply"]));
    assert!(
        fs::read_to_string(f.0.join("main.ts"))
            .unwrap()
            .contains("remove docs")
    );
}

#[test]
fn unsupported_languages_and_parse_errors_have_receipts() {
    let f = TestWorkspace::new();
    fs::write(f.0.join("bad.ts"), "function {").unwrap();
    fs::write(f.0.join("other.vue"), "# ordinary\n").unwrap();
    let output: serde_json::Value =
        serde_json::from_str(&success(f.run(&["comments", "pick", "--output", "json"]))).unwrap();
    assert_eq!(output["skipped"].as_array().unwrap().len(), 2);
    assert!(
        output["skipped"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("parse error")
    );
}

#[test]
fn codebase_comment_rule_is_opt_in_and_dataflow_is_isolated() {
    let f = TestWorkspace::new();
    fs::write(
        f.0.join("main.ts"),
        "// your code here\nfunction run() {}\n",
    )
    .unwrap();
    let run = |analysis: &str, enabled: bool| {
        let mut args = vec![
            "analyze",
            ".",
            "--analysis",
            analysis,
            "--output",
            "json",
            "--reproducible",
        ];
        if enabled {
            args.extend(["--set", "rules.enable=['reforge.codebase.comment_hygiene']"]);
        }
        let result = success(f.run(&args));
        serde_json::from_str::<serde_json::Value>(&result).unwrap()
    };
    let enabled = run("codebase", true);
    let evidence = enabled["issues"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|issue| issue["evidence"].as_array().unwrap())
        .collect::<Vec<_>>();
    assert!(
        evidence
            .iter()
            .any(|e| e["rule"] == "reforge.codebase.comment_hygiene")
    );
    assert_eq!(
        enabled["coverage"]["codebase"]["rules"]["reforge.codebase.comment_hygiene"]["enabled_source"],
        "enable"
    );
    let disabled = run("codebase", false);
    assert!(disabled["issues"].as_array().unwrap().is_empty());
    let flow = run("dataflow", true);
    assert!(flow["issues"].as_array().unwrap().is_empty());
    assert!(flow["coverage"]["codebase"].is_null());
}

#[test]
fn emitted_utf8_diff_is_accepted_by_git_apply() {
    let f = TestWorkspace::new();
    success(f.run(&[
        "comments",
        "clean",
        "--contains",
        "remove",
        "--output-file",
        "cleanup.patch",
    ]));
    let check = Command::new("git")
        .current_dir(&f.0)
        .args(["apply", "--check", "cleanup.patch"])
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn comment_rule_coverage_counts_only_supported_parsed_files() {
    let workspace = TestWorkspace::new();
    fs::write(workspace.0.join("valid.py"), "def f():\n    pass\n").unwrap();
    fs::write(workspace.0.join("bad.py"), "def : ???\n").unwrap();
    fs::write(workspace.0.join("bad.ts"), "function {\n").unwrap();
    let output: serde_json::Value = serde_json::from_str(&success(workspace.run(&[
        "analyze",
        ".",
        "--analysis",
        "codebase",
        "--output",
        "json",
        "--reproducible",
        "--set",
        "rules.enable=['reforge.codebase.comment_hygiene']",
    ])))
    .unwrap();
    let receipt = &output["coverage"]["codebase"]["rules"]["reforge.codebase.comment_hygiene"];
    assert_eq!(receipt["status"], "partial");
    assert_eq!(receipt["observations"][0]["count"], 2);
    assert_eq!(receipt["limitations"][0]["count"], 2);
}

#[test]
fn comment_rule_supports_suppression_and_combined_execution() {
    let workspace = TestWorkspace::new();
    fs::write(
        workspace.0.join("main.ts"),
        "// your code here\nfunction run() {}\n",
    )
    .unwrap();
    fs::write(
        workspace.0.join("reforge.toml"),
        r#"version = 2
[analysis]
enabled = ["codebase", "dataflow"]
[rules]
enable = ["reforge.codebase.comment_hygiene"]
[[suppressions]]
rule = "reforge.codebase.comment_hygiene"
path = "main.ts"
reason = "Intentional tutorial placeholder"
"#,
    )
    .unwrap();
    let output: serde_json::Value = serde_json::from_str(&success(workspace.run(&[
        "analyze",
        ".",
        "--output",
        "json",
        "--reproducible",
    ])))
    .unwrap();
    assert!(output["issues"].as_array().unwrap().is_empty());
    assert!(output["coverage"]["codebase"].is_object());
    assert!(output["coverage"]["dataflow"].is_object());
    assert_eq!(output["suppression"]["evidence_count"], 1);
    assert_eq!(
        output["suppression"]["by_rule"]["reforge.codebase.comment_hygiene"],
        1
    );
}

#[test]
fn all_previews_and_replays_a_plan_that_removes_documentation() {
    let workspace = TestWorkspace::new();
    let original = fs::read(workspace.0.join("main.ts")).unwrap();
    let preview = workspace.run(&["comments", "clean", "--all"]);
    assert!(String::from_utf8_lossy(&preview.stderr).contains("All-comments mode"));
    let patch = success(preview);
    assert!(patch.contains("-/** remove docs */"));
    assert_eq!(fs::read(workspace.0.join("main.ts")).unwrap(), original);
    success(workspace.run(&[
        "comments",
        "clean",
        "--all",
        "--output",
        "json",
        "--output-file",
        "all.json",
    ]));
    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(workspace.0.join("all.json")).unwrap()).unwrap();
    assert_eq!(plan["version"], 2);
    assert_eq!(plan["removal_mode"], "all");
    success(workspace.run(&["comments", "clean", "--plan", "all.json", "--apply"]));
    assert_eq!(
        fs::read_to_string(workspace.0.join("main.ts")).unwrap(),
        "function run() { return 1; }\n\nfunction documented() {}\n"
    );
}

#[test]
fn all_applies_in_one_command_and_rejects_narrowing_selectors() {
    let workspace = TestWorkspace::new();
    for selector in [
        vec!["--contains", "remove"],
        vec!["--text", "remove template"],
        vec!["--id", "comment-fake"],
        vec!["--kind", "line"],
        vec!["--candidates"],
        vec!["--plan", "all.json"],
    ] {
        let mut args = vec!["comments", "clean", "--all"];
        args.extend(selector);
        assert!(!workspace.run(&args).status.success(), "{args:?}");
    }
    success(workspace.run(&["comments", "clean", "--all", "--apply"]));
    let picked: serde_json::Value = serde_json::from_str(&success(
        workspace.run(&["comments", "pick", "--output", "json"]),
    ))
    .unwrap();
    assert!(picked["comments"].as_array().unwrap().is_empty());
}

#[test]
fn cleans_mixed_language_workspace_with_existing_scope_filters() {
    let workspace = TestWorkspace::new();
    for (name, source) in [
        ("module.py", "# remove\nx = '# literal'\n"),
        (
            "header.hpp",
            "/* remove */\nconst char *x = \"// literal\";\n",
        ),
        ("config.yaml", "# remove\nx: '# literal'\n"),
        ("Gemfile", "# remove\nsource 'https://example.com'\n"),
        ("target/generated.py", "# keep\n"),
        ("ignored/config.yaml", "# keep\n"),
        (".hidden.lua", "-- keep\n"),
    ] {
        let path = workspace.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
    let args = [
        "comments",
        "clean",
        "--text",
        "remove",
        "--ignore-path",
        "ignored",
        "--output",
        "json",
    ];
    let first = success(workspace.run(&args));
    assert_eq!(first, success(workspace.run(&args)));
    let plan: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(plan["files"].as_array().unwrap().len(), 4);
    assert!(plan["skipped"].as_array().unwrap().is_empty());
    fs::write(workspace.0.join("mixed.json"), first).unwrap();
    success(workspace.run(&["comments", "clean", "--plan", "mixed.json", "--apply"]));
    for name in ["module.py", "header.hpp", "config.yaml", "Gemfile"] {
        assert!(
            !fs::read_to_string(workspace.0.join(name))
                .unwrap()
                .contains("remove")
        );
    }
    for name in ["target/generated.py", "ignored/config.yaml", ".hidden.lua"] {
        assert!(
            fs::read_to_string(workspace.0.join(name))
                .unwrap()
                .contains("keep")
        );
    }
}
