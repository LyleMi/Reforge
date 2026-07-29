use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::io::sha256_file;
use crate::model::{CorpusManifest, Matrix};

const REQUIRED_LANGUAGES: [&str; 5] = ["javascript", "python", "rust", "tsx", "typescript"];

pub(crate) fn load_corpus(path: &Path) -> Result<CorpusManifest> {
    let input = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let manifest = toml::from_str::<CorpusManifest>(&input)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    validate_corpus(&manifest)?;
    Ok(manifest)
}

pub(crate) fn validate_corpus(manifest: &CorpusManifest) -> Result<()> {
    validate_manifest_header(manifest)?;
    validate_collection_metadata(manifest)?;
    let languages = validate_repositories(manifest)?;
    let required = BTreeSet::from(REQUIRED_LANGUAGES);
    if languages != required {
        bail!(
            "calibration corpus languages must be exactly {}; found {}",
            REQUIRED_LANGUAGES.join(", "),
            languages.into_iter().collect::<Vec<_>>().join(", ")
        );
    }
    Ok(())
}

fn validate_manifest_header(manifest: &CorpusManifest) -> Result<()> {
    if manifest.version != 1 {
        bail!(
            "unsupported calibration corpus version {}",
            manifest.version
        );
    }
    if manifest.report_schema != reforge_schema::REPORT_SCHEMA_VERSION {
        bail!(
            "calibration corpus report schema {} does not match schema {}",
            manifest.report_schema,
            reforge_schema::REPORT_SCHEMA_VERSION
        );
    }
    if manifest.frozen_at.trim().is_empty() {
        bail!("calibration corpus frozen-at must not be empty");
    }
    if manifest.repositories.len() != 15 {
        bail!(
            "calibration corpus must contain exactly 15 frozen repositories, found {}",
            manifest.repositories.len()
        );
    }
    Ok(())
}

fn validate_repositories(manifest: &CorpusManifest) -> Result<BTreeSet<&str>> {
    let mut repositories = BTreeSet::new();
    let mut languages = BTreeSet::new();
    for entry in &manifest.repositories {
        validate_repository(entry)?;
        if !repositories.insert(entry.repository.as_str()) {
            bail!("duplicate corpus repository `{}`", entry.repository);
        }
        languages.insert(entry.language.as_str());
    }
    Ok(languages)
}

fn validate_repository(entry: &crate::model::CorpusRepository) -> Result<()> {
    if !valid_repository(&entry.repository) {
        bail!(
            "corpus repository `{}` must use an owner/name identifier",
            entry.repository
        );
    }
    if entry.commit.len() != 40
        || !entry
            .commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!(
            "corpus repository `{}` must pin a lowercase 40-character commit",
            entry.repository
        );
    }
    if entry.license.trim().is_empty() {
        bail!(
            "corpus repository `{}` must declare a license",
            entry.repository
        );
    }
    Ok(())
}

fn validate_collection_metadata(manifest: &CorpusManifest) -> Result<()> {
    let metadata = &manifest.collection;
    for (name, value) in [
        ("clone-command", &metadata.clone_command),
        ("codebase-command", &metadata.codebase_command),
        ("dataflow-command", &metadata.dataflow_command),
        ("combined-command", &metadata.combined_command),
        ("reports", &metadata.reports),
    ] {
        if value.trim().is_empty() {
            bail!("calibration corpus collection.{name} must not be empty");
        }
    }
    Ok(())
}

fn valid_repository(value: &str) -> bool {
    value
        .split_once('/')
        .is_some_and(|(owner, name)| !owner.is_empty() && !name.is_empty() && !name.contains('/'))
        && !value.chars().any(char::is_whitespace)
}

pub(crate) fn corpus_matrix(manifest: &CorpusManifest) -> Matrix {
    Matrix {
        include: manifest.repositories.clone(),
    }
}

pub(crate) fn corpus_digest(path: &Path) -> Result<String> {
    sha256_file(path)
}
