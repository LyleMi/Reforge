use std::path::Path;
use std::process::Command;

use anyhow::Result;

use crate::detectors;
use crate::fingerprint::{fingerprint, fingerprint_json};
use crate::model::{ConfigurationProvenance, EngineProvenance, ReportProvenance, SourceProvenance};

pub(crate) fn build_revision() -> Option<&'static str> {
    option_env!("REFORGE_BUILD_REVISION").filter(|value| !value.is_empty())
}

pub(crate) fn engine_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub(crate) fn collect(
    root: &Path,
    effective_config: serde_json::Value,
) -> Result<ReportProvenance> {
    let source = source_provenance(root);
    let configuration_hash = fingerprint_json(&effective_config);
    let policy = serde_json::json!({
        "policy_abi": "reforge-detector-policy-v1",
        "engine_build_revision": build_revision(),
        "detector_manifest": detectors::manifest::detector_manifest(),
        "raw_metric_manifest": detectors::manifest::raw_metric_manifest(),
    });
    Ok(ReportProvenance {
        engine: EngineProvenance {
            version: engine_version().to_string(),
            build_revision: build_revision().map(str::to_string),
        },
        source,
        configuration: ConfigurationProvenance {
            effective: effective_config,
            hash: configuration_hash,
        },
        detector_policy_hash: fingerprint(&policy)?,
    })
}

fn source_provenance(root: &Path) -> SourceProvenance {
    let revision = git_output(root, &["rev-parse", "HEAD"]);
    let dirty = revision.as_ref().map(|_| {
        Command::new("git")
            .args(["status", "--porcelain", "--untracked-files=normal"])
            .current_dir(root)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .is_none_or(|output| !output.stdout.is_empty())
    });
    SourceProvenance {
        git_revision: revision,
        dirty,
    }
}

fn git_output(root: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_non_git_and_configuration_hash_changes() -> Result<()> {
        let root = temporary_root("non-git")?;
        let first = collect(&root, serde_json::json!({"threshold": 1}))?;
        let second = collect(&root, serde_json::json!({"threshold": 2}))?;
        assert_eq!(first.source.git_revision, None);
        assert_eq!(first.source.dirty, None);
        assert_ne!(first.configuration.hash, second.configuration.hash);
        assert_eq!(first.detector_policy_hash, second.detector_policy_hash);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn records_clean_and_dirty_target_git_state() -> Result<()> {
        let root = temporary_root("git")?;
        if !run_git(&root, &["init", "--quiet"]) {
            std::fs::remove_dir_all(root)?;
            return Ok(());
        }
        run_git(&root, &["config", "user.email", "reforge@example.invalid"]);
        run_git(&root, &["config", "user.name", "Reforge Test"]);
        std::fs::write(root.join("source.rs"), "fn clean() {}\n")?;
        assert!(run_git(&root, &["add", "source.rs"]));
        assert!(run_git(&root, &["commit", "--quiet", "-m", "test"]));

        let clean = collect(&root, serde_json::json!({}))?;
        assert!(clean.source.git_revision.is_some());
        assert_eq!(clean.source.dirty, Some(false));
        std::fs::write(root.join("source.rs"), "fn dirty() {}\n")?;
        let dirty = collect(&root, serde_json::json!({}))?;
        assert_eq!(dirty.source.git_revision, clean.source.git_revision);
        assert_eq!(dirty.source.dirty, Some(true));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    fn temporary_root(name: &str) -> Result<std::path::PathBuf> {
        let root = std::env::temp_dir().join(format!(
            "reforge-provenance-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::create_dir_all(&root)?;
        Ok(root)
    }

    fn run_git(root: &Path, args: &[&str]) -> bool {
        Command::new("git")
            .args(args)
            .current_dir(root)
            .status()
            .is_ok_and(|status| status.success())
    }
}
