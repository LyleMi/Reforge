use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::execution::{ChurnMode, EffectiveConfig};
use crate::model::{ChurnFileMetric, ChurnSummary, RawMetrics};

use super::display_path;

pub(super) fn collect_churn_metrics(
    root: &Path,
    args: &EffectiveConfig,
    raw_metrics: &mut RawMetrics,
) -> Result<ChurnSummary> {
    let mode = args.churn.expect("effective args should set churn mode");
    let window_days = args
        .churn_window_days
        .expect("effective args should set churn window");
    let max_commit_lines = args
        .churn_max_commit_lines
        .expect("effective args should set churn max commit lines");

    if mode == ChurnMode::Off {
        return Ok(churn_summary(ChurnSummaryInput {
            mode,
            enabled: false,
            status: "disabled",
            reason: Some("churn collection disabled by configuration".to_string()),
            window_days,
            max_commit_lines,
        }));
    }

    match collect_git_churn(root, window_days, max_commit_lines) {
        Ok(churn_by_path) => {
            for file_metric in &mut raw_metrics.files {
                if let Some(churn) = churn_by_path.get(&file_metric.path) {
                    file_metric.churn = churn.clone();
                }
            }

            Ok(churn_summary(ChurnSummaryInput {
                mode,
                enabled: true,
                status: "enabled",
                reason: None,
                window_days,
                max_commit_lines,
            }))
        }
        Err(error) if mode == ChurnMode::Auto => Ok(churn_summary(ChurnSummaryInput {
            mode,
            enabled: false,
            status: "unavailable",
            reason: Some(error.to_string()),
            window_days,
            max_commit_lines,
        })),
        Err(error) => Err(error),
    }
}

struct ChurnSummaryInput {
    mode: ChurnMode,
    enabled: bool,
    status: &'static str,
    reason: Option<String>,
    window_days: usize,
    max_commit_lines: usize,
}

fn churn_summary(input: ChurnSummaryInput) -> ChurnSummary {
    ChurnSummary {
        mode: input.mode,
        enabled: input.enabled,
        status: input.status.to_string(),
        reason: input.reason,
        window_days: input.window_days,
        max_commit_lines: input.max_commit_lines,
    }
}

fn collect_git_churn(
    root: &Path,
    window_days: usize,
    max_commit_lines: usize,
) -> Result<BTreeMap<String, ChurnFileMetric>> {
    let command_root = if root.is_file() {
        root.parent().unwrap_or(root)
    } else {
        root
    };
    let git_root_output = Command::new("git")
        .arg("-C")
        .arg(command_root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("failed to run git rev-parse")?;
    if !git_root_output.status.success() {
        anyhow::bail!("scan root is not inside a git repository");
    }

    let git_root_text = String::from_utf8_lossy(&git_root_output.stdout);
    let git_root = PathBuf::from(git_root_text.trim());
    let scan_relative = root
        .strip_prefix(&git_root)
        .ok()
        .map(path_to_git_slash)
        .unwrap_or_default();
    let since = format!("{window_days} days ago");
    let log_output = Command::new("git")
        .arg("-C")
        .arg(&git_root)
        .args([
            "log",
            "--no-merges",
            &format!("--since={since}"),
            "--numstat",
            "--format=commit:%H%x09%an",
        ])
        .output()
        .context("failed to run git log")?;
    if !log_output.status.success() {
        anyhow::bail!("failed to collect git churn");
    }

    let churn_by_relative_path = parse_git_numstat_churn(
        &String::from_utf8_lossy(&log_output.stdout),
        &scan_relative,
        max_commit_lines,
    );
    Ok(churn_by_relative_path
        .into_iter()
        .map(|(path, churn)| (display_path(&git_root.join(path)), churn))
        .collect())
}

#[derive(Debug, Clone)]
struct PendingCommitChurn {
    author: String,
    files: Vec<(String, usize, usize)>,
    total_lines: usize,
}

pub(super) fn parse_git_numstat_churn(
    output: &str,
    scan_relative: &str,
    max_commit_lines: usize,
) -> BTreeMap<String, ChurnFileMetric> {
    let mut churn_by_path = BTreeMap::<String, ChurnFileMetric>::new();
    let mut authors_by_path = BTreeMap::<String, BTreeSet<String>>::new();
    let mut pending: Option<PendingCommitChurn> = None;

    for line in output.lines() {
        if let Some(header) = line.strip_prefix("commit:") {
            flush_pending_commit(
                &mut churn_by_path,
                &mut authors_by_path,
                pending.take(),
                max_commit_lines,
            );
            let author = header
                .split_once('\t')
                .map(|(_, author)| author)
                .unwrap_or_default()
                .to_string();
            pending = Some(PendingCommitChurn {
                author,
                files: Vec::new(),
                total_lines: 0,
            });
            continue;
        }

        let Some(commit) = pending.as_mut() else {
            continue;
        };
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() < 3 || fields[0] == "-" || fields[1] == "-" {
            continue;
        }
        let Ok(added) = fields[0].parse::<usize>() else {
            continue;
        };
        let Ok(deleted) = fields[1].parse::<usize>() else {
            continue;
        };
        let path = normalize_git_numstat_path(fields[2]);
        if !path_in_scan_root(&path, scan_relative) {
            continue;
        }
        commit.total_lines += added + deleted;
        commit.files.push((path, added, deleted));
    }

    flush_pending_commit(
        &mut churn_by_path,
        &mut authors_by_path,
        pending,
        max_commit_lines,
    );
    for (path, authors) in authors_by_path {
        if let Some(metric) = churn_by_path.get_mut(&path) {
            metric.authors_count = authors.len();
        }
    }
    churn_by_path
}

fn flush_pending_commit(
    churn_by_path: &mut BTreeMap<String, ChurnFileMetric>,
    authors_by_path: &mut BTreeMap<String, BTreeSet<String>>,
    pending: Option<PendingCommitChurn>,
    max_commit_lines: usize,
) {
    let Some(pending) = pending else {
        return;
    };
    if pending.total_lines > max_commit_lines {
        return;
    }

    for (path, added, deleted) in pending.files {
        let metric = churn_by_path.entry(path.clone()).or_default();
        metric.commits_touched += 1;
        metric.lines_added += added;
        metric.lines_deleted += deleted;
        metric.recent_weighted_churn += added + deleted;
        if !pending.author.is_empty() {
            authors_by_path
                .entry(path)
                .or_default()
                .insert(pending.author.clone());
        }
    }
}

fn normalize_git_numstat_path(path: &str) -> String {
    let path = path
        .rsplit_once(" => ")
        .map(|(_, new_path)| new_path)
        .unwrap_or(path);
    path.trim_matches(['{', '}']).replace('\\', "/")
}

fn path_in_scan_root(path: &str, scan_relative: &str) -> bool {
    scan_relative.is_empty()
        || path == scan_relative
        || path
            .strip_prefix(scan_relative)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn path_to_git_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
