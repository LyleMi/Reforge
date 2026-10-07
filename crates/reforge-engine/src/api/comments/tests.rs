use super::*;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) struct TestWorkspace(pub(super) PathBuf);
impl TestWorkspace {
    pub(super) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "reforge-comments-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    pub(super) fn write(&self, path: &str, bytes: impl AsRef<[u8]>) {
        fs::create_dir_all(self.0.join(path).parent().unwrap()).unwrap();
        fs::write(self.0.join(path), bytes).unwrap();
    }
    pub(super) fn pick(&self) -> Inventory {
        pick(&self.0, &Config::defaults(), &Selection::default()).unwrap()
    }
    pub(super) fn plan(&self, text: &str) -> CleanPlan {
        prepare(
            &self.0,
            &Config::defaults(),
            &Selection {
                contains: vec![text.into()],
                ..Default::default()
            },
        )
        .unwrap()
    }
}
impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn extracts_syntax_comments_with_context_and_protections() {
    let f = TestWorkspace::new();
    f.write("lib.rs", "/// Public documentation\nfn sample() {\n    let url = \"https://example.com//fake\";\n    // plain explanation\n    // continued explanation\n    let _ = url; /* removable */\n}\n");
    let inventory = f.pick();
    assert_eq!(inventory.comments.len(), 3);
    assert!(inventory.comments[0].protected);
    assert_eq!(inventory.comments[1].symbol.as_deref(), Some("sample"));
    assert_eq!(inventory.comments[1].line, 4);
    assert_eq!(inventory.comments[1].end_line, 5);
    assert!(
        inventory.comments[1]
            .context
            .contains("https://example.com")
    );
    assert_eq!(inventory.comments[2].kind, CommentKind::Block);
}

#[test]
fn protections_cannot_be_overridden_by_text_or_imported_plan() {
    let f = TestWorkspace::new();
    f.write("main.ts", "// remove Copyright owner\n\n// remove @ts-ignore\nconst x = 1;\n// remove TODO unfinished\n\n// remove because order matters\n\n/** remove docs */\nfunction f() {}\n// remove ordinary\n");
    let plan = f.plan("remove");
    assert_eq!(plan.protected_comments, 5);
    assert_eq!(plan.files[0].comment_ids.len(), 1);
    apply(&plan).unwrap();
    let text = fs::read_to_string(f.0.join("main.ts")).unwrap();
    assert!(text.contains("@ts-ignore"));
    assert!(!text.contains("ordinary"));
}

#[test]
fn selectors_intersect_and_stale_ids_are_errors() {
    let f = TestWorkspace::new();
    f.write(
        "a.rs",
        "fn a() { /* remove one */ }\nfn b() { /* keep two */ }\n",
    );
    let selection = Selection {
        text: vec!["remove one".into()],
        kind: Some(CommentKind::Block),
        ..Default::default()
    };
    let inventory = pick(&f.0, &Config::defaults(), &selection).unwrap();
    assert_eq!(inventory.comments.len(), 1);
    let id = inventory.comments[0].id.clone();
    assert!(prepare(&f.0, &Config::defaults(), &Selection::default()).is_err());
    assert!(
        pick(
            &f.0,
            &Config::defaults(),
            &Selection {
                contains: vec!["".into()],
                ..Default::default()
            }
        )
        .is_err()
    );
    f.write("a.rs", "fn a() {}\n");
    assert!(
        pick(
            &f.0,
            &Config::defaults(),
            &Selection {
                ids: vec![id],
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn respects_scope_and_reports_unsupported_and_invalid_sources() {
    let f = TestWorkspace::new();
    for name in [
        "a.rs",
        "target/a.rs",
        "node_modules/a.js",
        ".hidden/a.ts",
        "ignored/a.ts",
        "skip.rs",
        "tests/a.rs",
    ] {
        f.write(name, "// removable\n");
    }
    f.write(".gitignore", "skip.rs\n");
    f.write("bad.ts", "function { ???");
    f.write("invalid.rs", [0x80]);
    f.write("unsupported.vue", "# comment\n");
    let mut config = Config::defaults();
    config.apply_scope_overrides(false, false, false, true, &["ignored".into()]);
    let inventory = pick(&f.0, &config, &Selection::default()).unwrap();
    assert_eq!(inventory.scanned_files, 1);
    assert_eq!(inventory.comments[0].path, "a.rs");
    assert_eq!(inventory.skipped.len(), 3);
}

#[test]
fn plan_is_deterministic_and_preview_never_writes() {
    let f = TestWorkspace::new();
    let original = "fn a() { /* remove */ let x = 1; }\n";
    f.write("a.rs", original);
    let first = f.plan("remove");
    let second = f.plan("remove");
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );
    let patch = diff(&first).unwrap();
    assert!(patch.contains("--- a/a.rs"));
    assert_eq!(fs::read_to_string(f.0.join("a.rs")).unwrap(), original);
    assert_eq!(apply(&first).unwrap(), 1);
    assert!(apply(&first).is_err());
}

#[test]
fn stale_or_tampered_plan_never_partially_applies() {
    let f = TestWorkspace::new();
    f.write("a.rs", "// remove\nfn a() {}\n");
    f.write("b.rs", "// remove\nfn b() {}\n");
    let mut plan = f.plan("remove");
    plan.files[0].edits[0].replacement = "fn malicious() {}\n".into();
    assert!(apply(&plan).is_err());
    plan = f.plan("remove");
    f.write("b.rs", "// changed\nfn b() {}\n");
    assert!(apply(&plan).is_err());
    assert!(
        fs::read_to_string(f.0.join("a.rs"))
            .unwrap()
            .contains("remove")
    );
    plan.files[0].path = "../escape.rs".into();
    assert!(apply(&plan).is_err());
}

#[test]
fn preserves_utf_encodings_crlf_and_missing_final_newline() {
    for encoding in [
        source::Encoding::Utf8,
        source::Encoding::Utf8Bom,
        source::Encoding::Utf16Le,
        source::Encoding::Utf16Be,
    ] {
        let f = TestWorkspace::new();
        let source = source::Source {
            text: String::new(),
            encoding,
        };
        let original = "// remove 中文\r\nfn main() { let 名称 = 1; }";
        f.write("a.rs", source.encode(original));
        apply(&f.plan("remove")).unwrap();
        assert_eq!(
            fs::read(f.0.join("a.rs")).unwrap(),
            source.encode("fn main() { let 名称 = 1; }")
        );
    }
}

#[test]
fn keeps_inline_tokens_and_javascript_line_terminators() {
    let f = TestWorkspace::new();
    f.write(
        "a.ts",
        "function f() { return /* remove */ 1; }\nconst x = 1/* remove\nsecond line */+2;\n",
    );
    let plan = f.plan("remove");
    apply(&plan).unwrap();
    let text = fs::read_to_string(f.0.join("a.ts")).unwrap();
    assert!(text.contains("return   1"));
    assert!(text.contains("1\n+2"));
}

#[test]
fn supports_tsx_nested_rust_comments_and_comment_only_files() {
    let f = TestWorkspace::new();
    f.write("a.tsx", "const view = <div>{/* remove */}hello</div>;\n");
    f.write("a.rs", "/* remove /* nested */ tail */\n");
    let plan = f.plan("remove");
    apply(&plan).unwrap();
    assert_eq!(fs::read(f.0.join("a.rs")).unwrap(), b"");
    assert!(
        fs::read_to_string(f.0.join("a.tsx"))
            .unwrap()
            .contains("{ }")
    );
}

#[test]
fn hints_are_review_only_and_cannot_authorize_cleanup() {
    let f = TestWorkspace::new();
    f.write("a.ts", "// const old = 1;\n\n// your code here\n\n// repeated description\n\n// repeated description\n");
    let inventory = pick(
        &f.0,
        &Config::defaults(),
        &Selection {
            candidates_only: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(inventory.comments.len(), 4);
    assert!(
        inventory.comments[0]
            .suggestions
            .contains(&"possible_commented_code".into())
    );
    assert!(
        prepare(
            &f.0,
            &Config::defaults(),
            &Selection {
                candidates_only: true,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn rejects_symlinks_and_hardlinks_and_preserves_permissions() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let f = TestWorkspace::new();
    f.write("a.rs", "// remove\nfn a() {}\n");
    let plan = f.plan("remove");
    fs::hard_link(f.0.join("a.rs"), f.0.join("hard.rs")).unwrap();
    assert!(apply(&plan).is_err());
    fs::remove_file(f.0.join("hard.rs")).unwrap();
    fs::set_permissions(f.0.join("a.rs"), fs::Permissions::from_mode(0o750)).unwrap();
    apply(&plan).unwrap();
    assert_eq!(
        fs::metadata(f.0.join("a.rs")).unwrap().permissions().mode() & 0o777,
        0o750
    );
    symlink(f.0.join("a.rs"), f.0.join("link.rs")).unwrap();
    assert!(
        pick(
            &f.0.join("link.rs"),
            &Config::defaults(),
            &Selection::default()
        )
        .is_err()
    );
}

#[test]
fn rejects_asi_tree_changes_without_writing() {
    let f = TestWorkspace::new();
    let original = "function f() { return /* remove\nline */ 1; }\n";
    f.write("a.ts", original);
    let result = prepare(
        &f.0,
        &Config::defaults(),
        &Selection {
            contains: vec!["remove".into()],
            ..Default::default()
        },
    );
    assert!(result.is_err());
    assert_eq!(fs::read_to_string(f.0.join("a.ts")).unwrap(), original);
}

#[test]
fn single_file_scope_exclusions_and_unsupported_receipts_are_respected() {
    let workspace = TestWorkspace::new();
    workspace.write("a.rs", "// removable\n");
    workspace.write("a.vue", "# comment\n");
    let mut config = Config::defaults();
    config.apply_scope_overrides(false, false, false, false, &["a.rs".into()]);
    assert!(
        pick(&workspace.0.join("a.rs"), &config, &Selection::default())
            .unwrap()
            .comments
            .is_empty()
    );
    let unsupported = pick(&workspace.0.join("a.vue"), &config, &Selection::default()).unwrap();
    assert_eq!(unsupported.skipped.len(), 1);
}

#[test]
fn regex_and_template_text_are_not_comments_but_interpolation_comments_are() {
    let workspace = TestWorkspace::new();
    workspace.write(
        "a.js",
        r#"const url = /https?:\/\/example/;
const template = `// not a comment ${1 /* actual */ + 2}`;
const raw = "/* also not */";
"#,
    );
    let inventory = workspace.pick();
    assert_eq!(inventory.comments.len(), 1);
    assert_eq!(inventory.comments[0].text, "/* actual */");
    apply(&workspace.plan("actual")).unwrap();
}

#[test]
fn readonly_second_file_stops_all_writes_and_cleans_staging() {
    let workspace = TestWorkspace::new();
    workspace.write("a.rs", "// remove\nfn a() {}\n");
    workspace.write("b.rs", "// remove\nfn b() {}\n");
    let plan = workspace.plan("remove");
    let path = workspace.0.join("b.rs");
    let original_permissions = fs::metadata(&path).unwrap().permissions();
    let mut readonly = original_permissions.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let result = apply(&plan);
    fs::set_permissions(&path, original_permissions).unwrap();
    assert!(result.is_err());
    assert!(
        fs::read_to_string(workspace.0.join("a.rs"))
            .unwrap()
            .contains("remove")
    );
    assert_eq!(fs::read_dir(&workspace.0).unwrap().count(), 2);
}

#[test]
fn all_mode_removes_protected_comments_and_preserves_code_and_encoding() {
    let workspace = TestWorkspace::new();
    let encoded = source::Source {
        text: String::new(),
        encoding: source::Encoding::Utf16Le,
    };
    workspace.write("a.rs", encoded.encode("//! crate docs\r\n/// function docs\r\nfn main() { /* SAFETY: explanation */ let text = \"// keep string\"; }\r\n"));
    workspace.write("a.ts", "// Copyright owner\n// TODO task\n// @ts-ignore\nconst value = 1; /* ordinary */\n/** API docs */\nfunction run() {}\n");
    let plan = prepare_all(&workspace.0, &Config::defaults()).unwrap();
    assert_eq!(plan.version, 2);
    assert_eq!(plan.removal_mode, RemovalMode::All);
    assert_eq!(plan.protected_comments, 0);
    let imported: CleanPlan = serde_json::from_slice(&serde_json::to_vec(&plan).unwrap()).unwrap();
    assert!(diff(&imported).unwrap().contains("-//! crate docs"));
    assert_eq!(apply(&imported).unwrap(), 2);
    assert!(workspace.pick().comments.is_empty());
    assert_eq!(
        fs::read(workspace.0.join("a.rs")).unwrap(),
        encoded.encode("fn main() {   let text = \"// keep string\"; }\r\n")
    );
    assert!(apply(&imported).is_err());
}

#[test]
fn all_mode_respects_scope_and_reports_skipped_sources() {
    let workspace = TestWorkspace::new();
    workspace.write("a.rs", "// ordinary\nfn main() {}\n");
    workspace.write("ignored.rs", "// keep\n");
    workspace.write("target/generated.rs", "// keep\n");
    workspace.write("a.vue", "# unsupported\n");
    let mut config = Config::defaults();
    config.apply_scope_overrides(false, false, false, false, &["ignored.rs".into()]);
    let plan = prepare_all(&workspace.0, &config).unwrap();
    assert_eq!(plan.files.len(), 1);
    assert_eq!(plan.skipped.len(), 1);
    apply(&plan).unwrap();
    assert_eq!(
        fs::read_to_string(workspace.0.join("ignored.rs")).unwrap(),
        "// keep\n"
    );
    assert_eq!(
        fs::read_to_string(workspace.0.join("target/generated.rs")).unwrap(),
        "// keep\n"
    );
    let again = prepare_all(&workspace.0, &config).unwrap();
    assert_eq!(apply(&again).unwrap(), 0);
}

#[test]
fn legacy_plans_keep_protection_and_all_mode_requires_its_version() {
    let workspace = TestWorkspace::new();
    workspace.write("a.rs", "/// docs\nfn main() {}\n// remove ordinary\n");
    let legacy = workspace.plan("remove");
    let json = serde_json::to_value(&legacy).unwrap();
    assert_eq!(json["version"], 1);
    assert!(json.get("removal_mode").is_none());
    let decoded: CleanPlan = serde_json::from_value(json).unwrap();
    assert_eq!(decoded.removal_mode, RemovalMode::Selected);
    validate(&decoded).unwrap();
    let all = prepare_all(&workspace.0, &Config::defaults()).unwrap();
    let mut tampered = all.clone();
    tampered.version = 1;
    assert!(apply(&tampered).is_err());
    tampered = all.clone();
    tampered.version = 1;
    tampered.removal_mode = RemovalMode::Selected;
    assert!(apply(&tampered).is_err());
    tampered = all;
    tampered.files[0].edits[0].replacement = "fn injected() {}\n".into();
    assert!(apply(&tampered).is_err());
    assert_eq!(workspace.pick().comments.len(), 2);
}
