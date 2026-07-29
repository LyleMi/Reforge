use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_reforge-calibrate")
}

fn corpus_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../calibration/corpus.toml")
        .canonicalize()
        .unwrap()
}

fn temp_manifest(transform: impl FnOnce(String) -> String) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "reforge-calibrate-cli-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("corpus.toml");
    let source = std::fs::read_to_string(corpus_path()).unwrap();
    std::fs::write(&path, transform(source)).unwrap();
    (root, path)
}

fn validate(path: &Path) -> bool {
    Command::new(binary())
        .args(["corpus", "validate", "--manifest"])
        .arg(path)
        .status()
        .unwrap()
        .success()
}

#[test]
fn corpus_validation_rejects_bad_version_duplicate_repository_and_unpinned_commit() {
    for transform in [
        |source: String| source.replacen("version = 1", "version = 2", 1),
        |source: String| source.replacen("ast-grep/ast-grep", "BurntSushi/ripgrep", 1),
        |source: String| {
            source.replacen(
                "dffd776a737dc19a48b758dd6a621de113794121",
                "not-a-fixed-commit",
                1,
            )
        },
    ] {
        let (root, manifest) = temp_manifest(transform);
        assert!(!validate(&manifest));
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn corpus_validation_rejects_missing_language_coverage() {
    let (root, manifest) = temp_manifest(|source| {
        source.replacen("language = \"tsx\"", "language = \"typescript\"", 1)
    });
    assert!(!validate(&manifest));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn preview_only_registry_passes_without_promotion_evidence() {
    let output = Command::new(binary())
        .args(["verify-promotion", "--corpus"])
        .arg(corpus_path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["promotion_candidates"], 0);
}
