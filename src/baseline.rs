use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::cli::BaselineShow;
use crate::model::{Finding, Issue, SCAN_REPORT_SCHEMA_VERSION, ScanReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaselineIssueStatus {
    New,
    Same,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BaselineIssue<'a> {
    pub issue: &'a Issue,
    pub status: BaselineIssueStatus,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BaselineDiffSummary {
    pub new: usize,
    pub same: usize,
    pub resolved: usize,
}

#[derive(Debug)]
pub(crate) struct BaselineDiff<'a> {
    pub summary: BaselineDiffSummary,
    pub issues: Vec<BaselineIssue<'a>>,
    pub show: BaselineShow,
}

pub(crate) fn load_baseline(path: &Path) -> Result<ScanReport> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read baseline {}", path.display()))?;
    validate_baseline_schema(path, &contents)?;

    let report = if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse JSON baseline {}", path.display()))?
    } else {
        serde_yaml::from_str(&contents)
            .with_context(|| format!("failed to parse YAML baseline {}", path.display()))?
    };
    validate_issue_ids(path, &report)?;
    Ok(report)
}

pub(crate) fn diff_issues<'a>(
    current: &'a [Issue],
    baseline: &ScanReport,
    show: BaselineShow,
) -> BaselineDiff<'a> {
    let previous = baseline
        .issues
        .iter()
        .map(|issue| (issue.id.as_str(), issue))
        .collect::<BTreeMap<_, _>>();
    let current_ids = current
        .iter()
        .map(|issue| issue.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    let mut summary = BaselineDiffSummary {
        resolved: baseline
            .issues
            .iter()
            .filter(|issue| !current_ids.contains(issue.id.as_str()))
            .count(),
        ..BaselineDiffSummary::default()
    };
    let mut issues = Vec::new();

    for issue in current {
        let status = match previous.get(issue.id.as_str()) {
            None => BaselineIssueStatus::New,
            Some(_) => BaselineIssueStatus::Same,
        };

        match status {
            BaselineIssueStatus::New => summary.new += 1,
            BaselineIssueStatus::Same => summary.same += 1,
        }

        if show_matches(show, status) {
            issues.push(BaselineIssue { issue, status });
        }
    }

    BaselineDiff {
        summary,
        issues,
        show,
    }
}

pub(crate) fn new_unsuppressed_findings<'a>(
    current: &'a [Finding],
    baseline: Option<&ScanReport>,
) -> Vec<&'a Finding> {
    let Some(baseline) = baseline else {
        return Vec::new();
    };
    let previous = baseline
        .findings
        .iter()
        .map(|finding| finding.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    current
        .iter()
        .filter(|finding| !previous.contains(finding.id.as_str()))
        .collect()
}

fn validate_baseline_schema(path: &Path, contents: &str) -> Result<()> {
    let version = if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        serde_json::from_str::<serde_json::Value>(contents)
            .ok()
            .and_then(|value| {
                value
                    .get("schema_version")
                    .and_then(|version| version.as_u64())
            })
    } else {
        serde_yaml::from_str::<serde_yaml::Value>(contents)
            .ok()
            .and_then(|value| {
                value
                    .get("schema_version")
                    .and_then(|version| version.as_u64())
            })
    };

    if version == Some(u64::from(SCAN_REPORT_SCHEMA_VERSION)) {
        return Ok(());
    }

    bail!(
        "baseline {} uses schema version {}; regenerate the baseline with Reforge schema {} so issues include stable IDs",
        path.display(),
        version
            .map(|version| version.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        SCAN_REPORT_SCHEMA_VERSION
    )
}

fn validate_issue_ids(path: &Path, report: &ScanReport) -> Result<()> {
    if report
        .issues
        .iter()
        .all(|issue| issue.id.starts_with("ri3-"))
    {
        return Ok(());
    }

    bail!(
        "baseline {} contains issues without stable IDs; regenerate the baseline with Reforge schema {}",
        path.display(),
        SCAN_REPORT_SCHEMA_VERSION
    )
}

fn show_matches(show: BaselineShow, status: BaselineIssueStatus) -> bool {
    match show {
        BaselineShow::New => status == BaselineIssueStatus::New,
        BaselineShow::All => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_schema_20_baselines_before_deserialization() {
        let error = validate_baseline_schema(
            Path::new("baseline.json"),
            r#"{"schema_version":20,"issues":[],"findings":[]}"#,
        )
        .expect_err("schema 20 must not be accepted as a baseline");

        assert!(error.to_string().contains("schema version 20"));
        assert!(error.to_string().contains("schema 22"));
    }
}
