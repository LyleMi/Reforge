use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use super::source::{self, Source, Staged};
use super::{Comment, Config, Inventory, Selection, SkippedFile, extract, hash, pick};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextEdit {
    pub start_byte: usize,
    pub end_byte: usize,
    pub original: String,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEdit {
    pub path: String,
    pub source_hash: String,
    pub comment_ids: Vec<String>,
    pub edits: Vec<TextEdit>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemovalMode {
    #[default]
    Selected,
    All,
}

impl RemovalMode {
    fn is_selected(&self) -> bool {
        *self == Self::Selected
    }

    fn removes(self, comment: &Comment) -> bool {
        self == Self::All || !comment.protected
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CleanPlan {
    pub version: u32,
    /// Version 1 plans omit this field and preserve protected comments.
    #[serde(default, skip_serializing_if = "RemovalMode::is_selected")]
    pub removal_mode: RemovalMode,
    pub root: PathBuf,
    pub selected_comments: usize,
    pub protected_comments: usize,
    pub files: Vec<FileEdit>,
    pub skipped: Vec<SkippedFile>,
}

pub fn prepare(root: &Path, config: &Config, selection: &Selection) -> Result<CleanPlan> {
    if !selection.is_explicit() {
        bail!(
            "clean requires --all, --id, --text or --contains; review hints alone do not select deletions"
        );
    }
    let inventory = pick(root, config, selection)?;
    build_plan(inventory, RemovalMode::Selected)
}

/// Select every comment in scope, including documentation and tool directives.
pub fn prepare_all(root: &Path, config: &Config) -> Result<CleanPlan> {
    build_plan(pick(root, config, &Selection::default())?, RemovalMode::All)
}

fn build_plan(inventory: Inventory, removal_mode: RemovalMode) -> Result<CleanPlan> {
    let mut plan = CleanPlan {
        version: if removal_mode == RemovalMode::All {
            2
        } else {
            1
        },
        removal_mode,
        root: inventory.root,
        selected_comments: inventory.comments.len(),
        protected_comments: inventory
            .comments
            .iter()
            .filter(|comment| !removal_mode.removes(comment))
            .count(),
        files: vec![],
        skipped: inventory.skipped,
    };
    let mut by_file = BTreeMap::<String, Vec<Comment>>::new();
    for comment in inventory
        .comments
        .into_iter()
        .filter(|comment| removal_mode.removes(comment))
    {
        by_file
            .entry(comment.path.clone())
            .or_default()
            .push(comment);
    }
    for (path, comments) in by_file {
        let full = source::safe_path(&plan.root, &path)?;
        let bytes = fs::read(&full)?;
        let source = Source::decode(&bytes)?;
        if comments
            .iter()
            .any(|comment| comment.source_hash != hash(&bytes))
        {
            bail!("source changed while preparing plan: {path}");
        }
        let file = file_edit(&path, &source.text, &hash(&bytes), &comments);
        checked_replacement(&full, &source.text, &file)?;
        plan.files.push(file);
    }
    Ok(plan)
}

fn file_edit(path: &str, source: &str, source_hash: &str, comments: &[Comment]) -> FileEdit {
    FileEdit {
        path: path.into(),
        source_hash: source_hash.into(),
        comment_ids: comments.iter().map(|comment| comment.id.clone()).collect(),
        edits: comments
            .iter()
            .map(|comment| edit(source, comment))
            .collect(),
    }
}

fn edit(source: &str, comment: &Comment) -> TextEdit {
    let line_start = source[..comment.start_byte]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let line_end = if source[..comment.end_byte].ends_with('\n') {
        comment.end_byte
    } else {
        source[comment.end_byte..]
            .find('\n')
            .map_or(source.len(), |index| comment.end_byte + index + 1)
    };
    let (start, end, replacement) = if source[line_start..comment.start_byte].trim().is_empty()
        && source[comment.end_byte..line_end].trim().is_empty()
    {
        (line_start, line_end, String::new())
    } else {
        // Preserve line terminators and separate tokens for inline/block comments.
        let newlines: String = source[comment.start_byte..comment.end_byte]
            .chars()
            .filter(|c| matches!(c, '\r' | '\n' | '\u{2028}' | '\u{2029}'))
            .collect();
        (
            comment.start_byte,
            comment.end_byte,
            if newlines.is_empty() {
                " ".into()
            } else {
                newlines
            },
        )
    };
    TextEdit {
        start_byte: start,
        end_byte: end,
        original: source[start..end].into(),
        replacement,
    }
}

fn checked_replacement(path: &Path, source: &str, file: &FileEdit) -> Result<String> {
    let before = extract::parse(path, source)?;
    let mut result = source.to_owned();
    let mut previous = source.len();
    for edit in file.edits.iter().rev() {
        if edit.start_byte > edit.end_byte
            || edit.end_byte > previous
            || source.get(edit.start_byte..edit.end_byte) != Some(edit.original.as_str())
        {
            bail!("invalid or overlapping edits for {}", file.path);
        }
        result.replace_range(edit.start_byte..edit.end_byte, &edit.replacement);
        previous = edit.start_byte;
    }
    let after = extract::parse(path, &result)?;
    if extract::syntax_signature(&before, source) != extract::syntax_signature(&after, &result) {
        bail!(
            "comment removal changes non-comment syntax in {}",
            file.path
        );
    }
    Ok(result)
}

struct Prepared {
    path: PathBuf,
    before: Vec<u8>,
    after: Vec<u8>,
    original_text: String,
    replacement_text: String,
}

fn preflight(plan: &CleanPlan) -> Result<Vec<Prepared>> {
    if !matches!(
        (plan.version, plan.removal_mode),
        (1, RemovalMode::Selected) | (2, RemovalMode::All)
    ) {
        bail!("unsupported comment plan version {}", plan.version);
    }
    if !plan.root.is_absolute() || plan.root.canonicalize()? != plan.root || !plan.root.is_dir() {
        bail!("plan root must be a canonical absolute directory");
    }
    let mut seen = BTreeSet::new();
    let mut prepared = Vec::new();
    for file in &plan.files {
        if !seen.insert(&file.path) {
            bail!("duplicate file in comment plan: {}", file.path);
        }
        prepared.push(preflight_file(&plan.root, file, plan.removal_mode)?);
    }
    Ok(prepared)
}

fn preflight_file(root: &Path, file: &FileEdit, removal_mode: RemovalMode) -> Result<Prepared> {
    let path = source::safe_path(root, &file.path)?;
    let before = fs::read(&path)?;
    if hash(&before) != file.source_hash {
        bail!("source changed since selection: {}", file.path);
    }
    let source = Source::decode(&before)?;
    let comments = extract::comments(&path, &file.path, &source.text, &file.source_hash)?;
    if removal_mode == RemovalMode::All && comments.len() != file.comment_ids.len() {
        bail!(
            "all-comments plan must select every comment in {}",
            file.path
        );
    }
    let selected = comments
        .into_iter()
        .filter(|comment| file.comment_ids.contains(&comment.id))
        .collect::<Vec<_>>();
    if selected.is_empty()
        || selected.len() != file.comment_ids.len()
        || selected
            .iter()
            .any(|comment| !removal_mode.removes(comment))
    {
        bail!(
            "invalid, stale or protected comment selection in {}",
            file.path
        );
    }
    // Recompute edits instead of trusting replacement text from imported plans.
    if file_edit(&file.path, &source.text, &file.source_hash, &selected) != *file {
        bail!(
            "plan edits differ from selected comment removals: {}",
            file.path
        );
    }
    let replacement_text = checked_replacement(&path, &source.text, file)?;
    Ok(Prepared {
        path,
        before,
        after: source.encode(&replacement_text),
        original_text: source.text,
        replacement_text,
    })
}

pub fn validate(plan: &CleanPlan) -> Result<()> {
    preflight(plan).map(|_| ())
}

/// Validate every file and stage replacements before committing any changes.
/// Recover earlier writes on an ordinary commit failure. This is not a
/// crash-atomic transaction spanning multiple files.
pub fn apply(plan: &CleanPlan) -> Result<usize> {
    let prepared = preflight(plan)?;
    let mut stages = Vec::new();
    for file in &prepared {
        stages.push(Staged::new(
            &file.path,
            &file.after,
            source::editable(&file.path)?,
        )?);
    }
    commit_all(&plan.root, &prepared, &stages)?;
    Ok(prepared.len())
}

fn commit_all(root: &Path, prepared: &[Prepared], stages: &[Staged]) -> Result<()> {
    for (index, (file, staged)) in prepared.iter().zip(stages).enumerate() {
        if let Err(error) = commit_one(root, file, staged) {
            let failures = rollback(root, &prepared[..index]);
            if !failures.is_empty() {
                bail!("{error}; rollback incomplete: {}", failures.join("; "));
            }
            return Err(error.context("comment apply failed; prior writes restored"));
        }
    }
    Ok(())
}

fn checked_path(root: &Path, file: &Prepared) -> Result<PathBuf> {
    let relative = file
        .path
        .strip_prefix(root)?
        .to_str()
        .context("non-UTF-8 source path")?
        .replace('\\', "/");
    source::safe_path(root, &relative)
}

fn commit_one(root: &Path, file: &Prepared, staged: &Staged) -> Result<()> {
    let path = checked_path(root, file)?;
    source::editable(&path)?;
    if fs::read(&path)? != file.before {
        bail!("source changed before apply: {}", path.display());
    }
    staged.commit(&path)
}

fn rollback(root: &Path, committed: &[Prepared]) -> Vec<String> {
    committed
        .iter()
        .rev()
        .filter_map(|file| {
            restore_one(root, file)
                .err()
                .map(|error| format!("{}: {error}", file.path.display()))
        })
        .collect()
}

fn restore_one(root: &Path, file: &Prepared) -> Result<()> {
    let path = checked_path(root, file)?;
    if fs::read(&path)? != file.after {
        bail!("source changed after apply");
    }
    Staged::new(&path, &file.before, source::editable(&path)?)?.commit(&path)
}

/// Human-review unified diff over decoded text. Apply the plan to preserve
/// UTF-16/BOM encodings instead of applying this text with an external patcher.
pub fn diff(plan: &CleanPlan) -> Result<String> {
    let files = preflight(plan)?;
    let mut output = String::new();
    for (change, file) in plan.files.iter().zip(files) {
        unified_diff(
            &change.path,
            &file.original_text,
            &file.replacement_text,
            &mut output,
        );
    }
    Ok(output)
}

fn unified_diff(path: &str, before: &str, after: &str, output: &mut String) {
    if before == after {
        return;
    }
    let old = before.split_inclusive('\n').collect::<Vec<_>>();
    let new = after.split_inclusive('\n').collect::<Vec<_>>();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let start = prefix.saturating_sub(3);
    let old_end = (old.len() - suffix + 3).min(old.len());
    let new_end = (new.len() - suffix + 3).min(new.len());
    let old_line = if old_end == start { start } else { start + 1 };
    let new_line = if new_end == start { start } else { start + 1 };
    output.push_str(&format!(
        "--- {}\n+++ {}\n@@ -{old_line},{} +{new_line},{} @@\n",
        quote_path(&format!("a/{path}")),
        quote_path(&format!("b/{path}")),
        old_end - start,
        new_end - start
    ));
    for line in &old[start..prefix] {
        diff_line(output, ' ', line);
    }
    for line in &old[prefix..old.len() - suffix] {
        diff_line(output, '-', line);
    }
    for line in &new[prefix..new.len() - suffix] {
        diff_line(output, '+', line);
    }
    for line in &old[old.len() - suffix..old_end] {
        diff_line(output, ' ', line);
    }
}

fn quote_path(path: &str) -> String {
    if path
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '"' | '\\'))
    {
        serde_json::to_string(path).expect("string serialization")
    } else {
        path.into()
    }
}

fn diff_line(output: &mut String, prefix: char, line: &str) {
    output.push(prefix);
    output.push_str(line);
    if !line.ends_with('\n') {
        output.push_str("\n\\ No newline at end of file\n");
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::TestWorkspace;
    use super::*;

    #[test]
    fn later_commit_failure_restores_earlier_files() {
        let workspace = TestWorkspace::new();
        workspace.write("a.rs", "// remove\nfn a() {}\n");
        workspace.write("b.rs", "// remove\nfn b() {}\n");
        let plan = workspace.plan("remove");
        let prepared = preflight(&plan).unwrap();
        let stages = prepared
            .iter()
            .map(|file| {
                Staged::new(
                    &file.path,
                    &file.after,
                    source::editable(&file.path).unwrap(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        stages[1]
            .commit(&workspace.0.join("displaced.tmp"))
            .unwrap();
        let error = commit_all(&plan.root, &prepared, &stages).unwrap_err();
        assert!(error.to_string().contains("prior writes restored"));
        for file in &prepared {
            assert_eq!(fs::read(&file.path).unwrap(), file.before);
        }
    }

    #[test]
    fn rollback_refuses_to_overwrite_concurrent_changes() {
        let workspace = TestWorkspace::new();
        workspace.write("a.rs", "// remove\nfn a() {}\n");
        let plan = workspace.plan("remove");
        let prepared = preflight(&plan).unwrap();
        workspace.write("a.rs", "fn user_edit() {}\n");
        let failures = rollback(&plan.root, &prepared);
        assert_eq!(failures.len(), 1);
        assert_eq!(
            fs::read_to_string(workspace.0.join("a.rs")).unwrap(),
            "fn user_edit() {}\n"
        );
    }
}
