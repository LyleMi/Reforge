//! Comment inventory and explicitly selected cleanup, independent of report schema 27.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{Config, EffectiveConfig};

mod classify;
mod extract;
mod languages;
pub(crate) use extract::supported as supports_comments;
mod plan;
mod source;
pub use plan::{
    CleanPlan, FileEdit, RemovalMode, TextEdit, apply, diff, prepare, prepare_all, validate,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentKind {
    Line,
    Block,
    Documentation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub path: String,
    pub source_hash: String,
    /// Byte offsets in decoded UTF-8, excluding the original BOM.
    pub start_byte: usize,
    pub end_byte: usize,
    pub line: usize,
    pub end_line: usize,
    pub kind: CommentKind,
    pub text: String,
    pub symbol: Option<String>,
    pub context_start_line: usize,
    pub context: String,
    pub protected: bool,
    pub reasons: Vec<String>,
    /// Review hints, never authorization to delete.
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedFile {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    pub version: u32,
    pub root: PathBuf,
    pub scanned_files: usize,
    pub total_comments: usize,
    pub comments: Vec<Comment>,
    pub skipped: Vec<SkippedFile>,
}

#[derive(Debug, Clone, Default)]
pub struct Selection {
    pub ids: Vec<String>,
    pub contains: Vec<String>,
    /// Exact match against trimmed comment bodies, without delimiters.
    pub text: Vec<String>,
    pub kind: Option<CommentKind>,
    pub candidates_only: bool,
}

impl Selection {
    pub fn is_explicit(&self) -> bool {
        !self.ids.is_empty() || !self.contains.is_empty() || !self.text.is_empty()
    }

    fn matches(&self, comment: &Comment) -> bool {
        (self.ids.is_empty() || self.ids.contains(&comment.id))
            && (self.contains.is_empty()
                || self.contains.iter().any(|text| comment.text.contains(text)))
            && (self.text.is_empty()
                || self
                    .text
                    .iter()
                    .any(|text| extract::body(&comment.text) == text.trim()))
            && self.kind.is_none_or(|kind| kind == comment.kind)
            && (!self.candidates_only || !comment.suggestions.is_empty())
    }

    fn check(&self) -> Result<()> {
        if self
            .contains
            .iter()
            .chain(&self.text)
            .any(|text| text.trim().is_empty())
        {
            bail!("comment text selectors must not be empty");
        }
        Ok(())
    }
}

pub fn pick(root: &Path, config: &Config, selection: &Selection) -> Result<Inventory> {
    selection.check()?;
    if std::fs::symlink_metadata(root)?.file_type().is_symlink() {
        bail!("comment root must not be a symbolic link");
    }
    let target = root.canonicalize()?;
    let base = if target.is_file() {
        target
            .parent()
            .context("source has no parent")?
            .to_path_buf()
    } else {
        target.clone()
    };
    let mut args = EffectiveConfig::defaults_for_path(target.clone());
    args.filters.include_hidden = config.scope.include_hidden;
    args.filters.include_generated = config.scope.include_generated;
    args.filters.no_gitignore = config.scope.no_gitignore;
    args.filters.exclude_tests = config.scope.exclude_tests;
    args.filters.ignore_paths = config.scope.ignore_paths.clone();
    let paths = crate::scan::comment_source_paths(&target, &args)?;
    let mut result = collect_inventory(base, paths)?;
    let known = result
        .comments
        .iter()
        .map(|comment| &comment.id)
        .collect::<BTreeSet<_>>();
    for id in &selection.ids {
        if !known.contains(id) {
            bail!("unknown or stale comment ID: {id}");
        }
    }
    result.comments.retain(|comment| selection.matches(comment));
    Ok(result)
}

fn collect_inventory(base: PathBuf, paths: Vec<PathBuf>) -> Result<Inventory> {
    let mut result = Inventory {
        version: 1,
        root: base.clone(),
        scanned_files: 0,
        total_comments: 0,
        comments: vec![],
        skipped: vec![],
    };
    for path in paths {
        let relative_path = path.strip_prefix(&base)?;
        let Some(parts) = relative_path
            .components()
            .map(|part| part.as_os_str().to_str())
            .collect::<Option<Vec<_>>>()
        else {
            result.skipped.push(SkippedFile {
                path: relative_path.to_string_lossy().into(),
                reason: "non-UTF-8 source path is unsupported".into(),
            });
            continue;
        };
        let relative = parts.join("/");
        let read = (|| {
            if !extract::supported(&path) {
                bail!(
                    "unsupported comment language; see docs/comments.md for supported extensions"
                );
            }
            let bytes = std::fs::read(source::safe_path(&base, &relative)?)?;
            let source = source::Source::decode(&bytes)?;
            extract::comments(&path, &relative, &source.text, &hash(&bytes))
        })();
        match read {
            Ok(comments) => {
                result.scanned_files += 1;
                result.total_comments += comments.len();
                result.comments.extend(comments);
            }
            Err(error) => result.skipped.push(SkippedFile {
                path: relative,
                reason: error.to_string(),
            }),
        }
    }
    Ok(result)
}

pub(super) fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub(crate) fn review_detections(
    sources: &[crate::detectors::similarity::ParsedSourceFile],
) -> Vec<crate::model::DetectedEvidence> {
    use crate::evidence_analysis::DetectedEvidenceInput;
    use crate::model::{DetectedEvidence, Rule};
    sources
        .iter()
        .filter(|source| extract::supported(&source.file.path))
        .flat_map(|source| {
            extract::comments_from_tree(
                &source.tree,
                &source.file.display_path,
                &source.file.source,
                "",
            )
            .into_iter()
            .filter(|comment| !comment.suggestions.is_empty())
            .map(|comment| {
                DetectedEvidence::from(
                    DetectedEvidenceInput::new(
                        Rule::CommentHygiene,
                        comment.path,
                        Some(comment.line),
                        format!(
                            "Review comment: {}; hints do not authorize deletion",
                            comment.suggestions.join(", ")
                        ),
                        vec![],
                    )
                    .with_file_subject(),
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod language_tests;
