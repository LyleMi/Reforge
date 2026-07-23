use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path};

use anyhow::{Context, Result, bail};
use reforge_schema::Report;
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use crate::{ApplicationArtifact, ApprovalArtifact, RunArtifact, SCHEMA_VERSION};

pub(crate) fn snapshot(root: &Path, run_dir: &Path) -> Result<BTreeMap<String, String>> {
    let root = root.canonicalize()?;
    let excluded_run = run_dir.canonicalize().ok();
    let mut result = BTreeMap::new();
    walk_snapshot(&root, &root, excluded_run.as_deref(), &mut result)?;
    Ok(result)
}

fn walk_snapshot(
    root: &Path,
    directory: &Path,
    excluded_run: Option<&Path>,
    output: &mut BTreeMap<String, String>,
) -> Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if excluded_run.is_some_and(|excluded| path.starts_with(excluded)) {
            continue;
        }
        let name = entry.file_name();
        if entry.file_type()?.is_dir() {
            if matches!(
                name.to_str(),
                Some(".git" | "target" | "node_modules" | "dist" | "build")
            ) {
                continue;
            }
            walk_snapshot(root, &path, excluded_run, output)?;
        } else if entry.file_type()?.is_file() {
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            output.insert(relative, file_hash(&path)?);
        }
    }
    Ok(())
}

pub(crate) fn changed_paths(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<String> {
    before
        .keys()
        .chain(after.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect()
}

pub(crate) fn enforce_write_set(changed: &[String], write_set: &[String]) -> Result<()> {
    let outside = changed
        .iter()
        .filter(|path| !write_set.iter().any(|allowed| path_allowed(path, allowed)))
        .cloned()
        .collect::<Vec<_>>();
    if !outside.is_empty() {
        bail!(
            "workspace changes outside approved write set: {}",
            outside.join(", ")
        );
    }
    Ok(())
}

pub(crate) fn path_allowed(path: &str, allowed: &str) -> bool {
    path == allowed
        || path
            .strip_prefix(allowed)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

pub(crate) fn normalize_write_path(path: &str) -> Result<String> {
    let path = Path::new(path);
    if path.is_absolute() {
        bail!("write_set path must be relative: {}", path.display());
    }
    let mut parts = Vec::new();
    for part in path.components() {
        match part {
            Component::Normal(value) => parts.push(value.to_string_lossy()),
            Component::CurDir => {}
            _ => bail!("write_set path escapes workspace: {}", path.display()),
        }
    }
    if parts.is_empty() {
        bail!("write_set may not contain the workspace root");
    }
    Ok(parts.join("/"))
}

pub(crate) fn load_run(run: &Path) -> Result<RunArtifact> {
    let value: serde_json::Value = serde_json::from_slice(
        &std::fs::read(run.join("run.json"))
            .with_context(|| format!("missing workflow run at {}", run.display()))?,
    )?;
    if value
        .get("artifact_schema_version")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|version| version < u64::from(SCHEMA_VERSION))
    {
        bail!(
            "legacy workflow artifact is unsupported; start a new artifact v5 workflow; see docs/upgrading-to-0.2.md"
        );
    }
    let artifact: RunArtifact = serde_json::from_value(value)?;
    validate_artifact_version(artifact.artifact_schema_version)?;
    Ok(artifact)
}

pub(crate) fn validate_application(run: &RunArtifact, run_dir: &Path) -> Result<()> {
    let approval: ApprovalArtifact = read_json(&run_dir.join("approval.json"))?;
    let application: ApplicationArtifact = read_json(&run_dir.join("application.json"))?;
    let current = snapshot(Path::new(&run.workspace_root), run_dir)?;
    let changed = changed_paths(&approval.workspace_snapshot, &current);
    enforce_write_set(&changed, &approval.write_set)?;
    if current != application.workspace_snapshot {
        bail!(
            "workspace changed after mark-applied; run mark-applied again is not allowed after leaving Applied"
        );
    }
    Ok(())
}

pub(crate) fn validate_artifact_version(version: u16) -> Result<()> {
    if version != SCHEMA_VERSION {
        bail!("unsupported workflow artifact schema {version}; expected v5");
    }
    Ok(())
}

pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_reader(
        File::open(path).with_context(|| format!("missing artifact {}", path.display()))?,
    )
    .with_context(|| format!("invalid artifact {}", path.display()))
}

pub(crate) fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let temporary = path.with_extension("tmp");
    let mut file = File::create(&temporary)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

pub(crate) fn copy_report(destination: &Path, report: &Report) -> Result<()> {
    write_json(destination, report)
}

pub(crate) fn safe_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect()
}

pub(crate) fn json_hash(value: &impl Serialize) -> Result<String> {
    Ok(bytes_hash(&serde_json::to_vec(value)?))
}

fn file_hash(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes_hash(&bytes))
}

fn bytes_hash(bytes: &[u8]) -> String {
    format!("sha256-{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_artifact_v4() {
        let error = validate_artifact_version(4).unwrap_err().to_string();
        assert!(error.contains("expected v5"));
    }
}
