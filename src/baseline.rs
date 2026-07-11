use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::cli::{BaselineMode, BaselineShow};
use crate::model::{Issue, SCAN_REPORT_SCHEMA_VERSION, ScanReport, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaselineIssueStatus {
    New,
    Worse,
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
    pub worse: usize,
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

pub(crate) fn selected_issues<'a>(
    current: &'a [Issue],
    baseline: Option<&ScanReport>,
    mode: BaselineMode,
) -> Vec<&'a Issue> {
    let Some(baseline) = baseline else {
        return current.iter().collect();
    };

    let show = match mode {
        BaselineMode::New => BaselineShow::New,
        BaselineMode::NewOrWorse => BaselineShow::NewOrWorse,
        BaselineMode::All => BaselineShow::All,
    };

    diff_issues(current, baseline, show)
        .issues
        .into_iter()
        .map(|entry| entry.issue)
        .collect()
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
            Some(previous) if is_worse(issue, previous) => BaselineIssueStatus::Worse,
            Some(_) => BaselineIssueStatus::Same,
        };

        match status {
            BaselineIssueStatus::New => summary.new += 1,
            BaselineIssueStatus::Worse => summary.worse += 1,
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

pub(crate) fn gate_failures<'a>(
    selected: impl IntoIterator<Item = &'a Issue>,
    threshold: crate::cli::FailOnSeverity,
) -> Vec<&'a Issue> {
    selected
        .into_iter()
        .filter(|issue| threshold.matches(issue.severity))
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

fn is_worse(current: &Issue, previous: &Issue) -> bool {
    severity_rank(current.severity) > severity_rank(previous.severity)
        || current.priority > previous.priority
}

fn show_matches(show: BaselineShow, status: BaselineIssueStatus) -> bool {
    match show {
        BaselineShow::New => status == BaselineIssueStatus::New,
        BaselineShow::NewOrWorse => {
            matches!(
                status,
                BaselineIssueStatus::New | BaselineIssueStatus::Worse
            )
        }
        BaselineShow::All => true,
    }
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Info => 0,
        Severity::Warning => 1,
        Severity::Critical => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        EvidenceSubject, FindingKind, IssueKey, PriorityFactors, QualityConstruct, RefactorAction,
        SignalMechanism,
    };

    fn issue(line: usize, priority: u8, severity: Severity) -> Issue {
        let subject = EvidenceSubject::File {
            path: format!("src/{line}.rs"),
        };
        Issue {
            id: IssueKey::from_family_and_subject("size", &subject),
            family: "size".into(),
            summary: format!("large file {line}"),
            construct: QualityConstruct::Modifiability,
            mechanism: SignalMechanism::ResponsibilityDispersion,
            action: RefactorAction::DecomposeResponsibility,
            path: format!("src/{line}.rs"),
            line: Some(line),
            primary_finding_id: format!("rf3-{line:032x}").into(),
            finding_ids: vec![format!("rf3-{line:032x}").into()],
            kinds: vec![FindingKind::LargeFile],
            severity,
            priority,
            priority_factors: PriorityFactors::default(),
            subject,
            detection_reliability: 1.0,
            interpretation_reliability: 1.0,
        }
    }

    fn baseline(issues: Vec<Issue>) -> ScanReport {
        ScanReport {
            schema_version: SCAN_REPORT_SCHEMA_VERSION,
            summary: crate::model::ScanSummary {
                scanned_files: 0,
                finding_count: 0,
                issue_count: issues.len(),
                hotspot_count: 0,
                similar_function_group_count: 0,
                duration_ms: 0,
                hotspot_model: crate::cli::HotspotModel::Hybrid,
                churn: crate::model::ChurnSummary {
                    mode: crate::cli::ChurnMode::Off,
                    enabled: false,
                    status: "disabled".to_string(),
                    reason: None,
                    window_days: 180,
                    max_commit_lines: 2_000,
                },
            },
            stats: crate::model::ScanStats::default(),
            metrics_summary: crate::model::MetricsSummary {
                directories: BTreeMap::new(),
                files: BTreeMap::new(),
                functions: BTreeMap::new(),
                types: BTreeMap::new(),
                churn: BTreeMap::new(),
            },
            raw_metrics: crate::model::RawMetrics::default(),
            raw_metric_manifest: Vec::new(),
            dependency_graph: crate::model::DependencyGraphSnapshot::default(),
            hotspots: Vec::new(),
            suppression_summary: crate::model::SuppressionSummary::default(),
            coverage_manifest: Vec::new(),
            coverage_summary: crate::model::CoverageSummary::default(),
            issues,
            detector_manifest: Vec::new(),
            findings: Vec::new(),
        }
    }

    #[test]
    fn baseline_mode_new_selects_only_absent_ids() {
        let old = baseline(vec![issue(1, 50, Severity::Warning)]);
        let current = vec![
            issue(1, 70, Severity::Critical),
            issue(2, 50, Severity::Warning),
        ];

        let selected = selected_issues(&current, Some(&old), BaselineMode::New);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, current[1].id);
    }

    #[test]
    fn baseline_mode_new_or_worse_selects_absent_and_higher_priority_ids() {
        let old = baseline(vec![
            issue(1, 50, Severity::Warning),
            issue(2, 50, Severity::Warning),
        ]);
        let current = vec![
            issue(1, 50, Severity::Warning),
            issue(2, 60, Severity::Warning),
            issue(3, 20, Severity::Info),
        ];

        let selected = selected_issues(&current, Some(&old), BaselineMode::NewOrWorse);

        assert_eq!(
            selected
                .iter()
                .map(|issue| issue.id.clone())
                .collect::<Vec<_>>(),
            vec![current[1].id.clone(), current[2].id.clone()]
        );
    }

    #[test]
    fn diff_classifies_new_worse_same_and_resolved_issues() {
        let old = baseline(vec![
            issue(1, 50, Severity::Warning),
            issue(2, 50, Severity::Warning),
            issue(3, 50, Severity::Warning),
            issue(4, 80, Severity::Critical),
        ]);
        let current = vec![
            issue(1, 50, Severity::Warning),
            issue(2, 60, Severity::Warning),
            issue(3, 50, Severity::Critical),
            issue(5, 20, Severity::Info),
        ];

        let diff = diff_issues(&current, &old, BaselineShow::All);

        assert_eq!(
            diff.summary,
            BaselineDiffSummary {
                new: 1,
                worse: 2,
                same: 1,
                resolved: 1,
            }
        );
        assert_eq!(
            diff.issues
                .iter()
                .map(|entry| entry.status)
                .collect::<Vec<_>>(),
            [
                BaselineIssueStatus::Same,
                BaselineIssueStatus::Worse,
                BaselineIssueStatus::Worse,
                BaselineIssueStatus::New,
            ]
        );
    }

    #[test]
    fn diff_show_new_or_worse_selects_only_actionable_changes() {
        let old = baseline(vec![
            issue(1, 50, Severity::Warning),
            issue(2, 50, Severity::Warning),
        ]);
        let current = vec![
            issue(1, 50, Severity::Warning),
            issue(2, 60, Severity::Warning),
            issue(3, 20, Severity::Info),
        ];

        let diff = diff_issues(&current, &old, BaselineShow::NewOrWorse);

        assert_eq!(
            diff.issues
                .iter()
                .map(|entry| entry.issue.id.clone())
                .collect::<Vec<_>>(),
            vec![current[1].id.clone(), current[2].id.clone()]
        );
    }
}
