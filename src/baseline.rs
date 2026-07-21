use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use serde_json::Value;

use crate::cli::BaselineShow;
use crate::fingerprint::fingerprint_json;
use crate::model::{
    BaselineChange, BaselineChangeOrigin, BaselineChanged, BaselineComparison,
    BaselineDifferenceSet, Finding, Issue, LineageCandidate, LineageEntity,
    SCAN_REPORT_SCHEMA_VERSION, ScanReport,
};

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

pub(crate) fn diff_issues(report: &ScanReport, show: BaselineShow) -> BaselineDiff<'_> {
    let comparison = report
        .baseline_comparison
        .as_ref()
        .expect("baseline display requires an embedded comparison");
    let added = comparison
        .issues
        .added
        .iter()
        .map(|change| change.id.as_str())
        .collect::<BTreeSet<_>>();
    let summary = BaselineDiffSummary {
        new: comparison.issues.added.len(),
        same: comparison.issues.unchanged_count + comparison.issues.changed.len(),
        resolved: comparison.issues.removed.len(),
    };
    let mut issues = Vec::new();
    for issue in &report.issues {
        let status = if added.contains(issue.id.as_str()) {
            BaselineIssueStatus::New
        } else {
            BaselineIssueStatus::Same
        };
        if show == BaselineShow::All || status == BaselineIssueStatus::New {
            issues.push(BaselineIssue { issue, status });
        }
    }
    BaselineDiff {
        summary,
        issues,
        show,
    }
}

pub(crate) fn load_baseline(path: &Path) -> Result<ScanReport> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read baseline {}", path.display()))?;
    validate_baseline_schema(path, &contents)?;
    let report = if is_json(path) {
        serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse JSON baseline {}", path.display()))?
    } else {
        serde_yaml::from_str(&contents)
            .with_context(|| format!("failed to parse YAML baseline {}", path.display()))?
    };
    validate_stable_ids(path, &report)?;
    validate_provenance(path, &report)?;
    Ok(report)
}

pub(crate) fn compare_reports(
    current: &ScanReport,
    previous: &ScanReport,
    baseline_path: Option<&Path>,
) -> Result<BaselineComparison> {
    let dimensions = provenance_change_dimensions(current, previous);
    let origin = change_origin(&dimensions);
    let findings = diff_values(
        &current.findings,
        &previous.findings,
        |finding| finding.id.as_str(),
        finding_signature,
        origin,
    )?;
    let mut issues = diff_values(
        &current.issues,
        &previous.issues,
        |issue| issue.id.as_str(),
        issue_signature,
        origin,
    )?;
    propagate_finding_changes(&mut issues, &findings, &current.issues, &previous.issues)?;
    let lineage_candidates = lineage_candidates(current, previous, &findings, &issues);
    Ok(BaselineComparison {
        baseline_path: baseline_path.map(|path| path.to_string_lossy().to_string()),
        baseline_provenance: previous.provenance.clone(),
        provenance_changed: dimensions
            .iter()
            .any(|dimension| dimension == "engine" || dimension == "configuration"),
        provenance_change_dimensions: dimensions,
        findings,
        issues,
        lineage_candidates,
    })
}

pub(crate) fn gate_count(report: &ScanReport, all: bool) -> usize {
    if all {
        report.findings.len()
    } else {
        report
            .baseline_comparison
            .as_ref()
            .map_or(0, |comparison| comparison.findings.added.len())
    }
}

fn diff_values<T, I, S>(
    current: &[T],
    previous: &[T],
    id: I,
    signature: S,
    origin: BaselineChangeOrigin,
) -> Result<BaselineDifferenceSet>
where
    T: Serialize,
    I: Fn(&T) -> &str,
    S: Fn(&T) -> Result<Value>,
{
    let current_by_id = current
        .iter()
        .map(|item| (id(item), item))
        .collect::<BTreeMap<_, _>>();
    let previous_by_id = previous
        .iter()
        .map(|item| (id(item), item))
        .collect::<BTreeMap<_, _>>();
    let mut result = BaselineDifferenceSet::default();

    for (item_id, item) in &current_by_id {
        let Some(old) = previous_by_id.get(item_id) else {
            result.added.push(BaselineChange {
                id: (*item_id).to_string(),
                origin,
                value: serde_json::to_value(item)?,
            });
            continue;
        };
        let before = signature(old)?;
        let after = signature(item)?;
        if before == after {
            result.unchanged_count += 1;
        } else {
            result.changed.push(BaselineChanged {
                id: (*item_id).to_string(),
                origin,
                changed_fields: changed_fields(&before, &after),
                before: serde_json::to_value(old)?,
                after: serde_json::to_value(item)?,
            });
        }
    }
    for (item_id, item) in previous_by_id {
        if !current_by_id.contains_key(item_id) {
            result.removed.push(BaselineChange {
                id: item_id.to_string(),
                origin,
                value: serde_json::to_value(item)?,
            });
        }
    }
    Ok(result)
}

fn finding_signature(finding: &Finding) -> Result<Value> {
    let metrics = finding
        .metrics
        .iter()
        .map(|metric| {
            serde_json::json!({
                "name": metric.name,
                "value": metric.value,
                "threshold": metric.threshold,
                "unit": metric.unit,
                "excess_ratio_e12": semantic_float(metric.excess_ratio),
                "normalized_e12": semantic_float(metric.normalized),
                "percentile_e12": semantic_float(metric.percentile),
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "kind": finding.kind,
        "path": finding.path,
        "line": finding.line,
        "metrics": metrics,
        "classification": {
            "construct": finding.construct,
            "mechanism": finding.mechanism,
        },
        "message": finding.message,
        "related_locations": finding.related_locations,
        "flow_witness": finding.flow_witness,
    }))
}

fn semantic_float(value: Option<f64>) -> Option<i64> {
    value.map(|value| (value * 1_000_000_000_000.0).round() as i64)
}

fn issue_signature(issue: &Issue) -> Result<Value> {
    Ok(serde_json::json!({
        "family": issue.family,
        "summary": issue.summary,
        "classification": {
            "construct": issue.construct,
            "mechanism": issue.mechanism,
            "action": issue.action,
        },
        "path": issue.path,
        "line": issue.line,
        "primary_finding_id": issue.primary_finding_id,
        "finding_ids": issue.finding_ids,
        "kinds": issue.kinds,
        "subject": issue.subject,
    }))
}

fn changed_fields(before: &Value, after: &Value) -> Vec<String> {
    let keys = before
        .as_object()
        .into_iter()
        .flat_map(|object| object.keys())
        .chain(
            after
                .as_object()
                .into_iter()
                .flat_map(|object| object.keys()),
        )
        .collect::<BTreeSet<_>>();
    keys.into_iter()
        .filter(|key| before.get(*key) != after.get(*key))
        .cloned()
        .collect()
}

fn provenance_change_dimensions(current: &ScanReport, previous: &ScanReport) -> Vec<String> {
    let mut dimensions = Vec::new();
    if current.provenance.engine != previous.provenance.engine
        || current.provenance.detector_policy_hash != previous.provenance.detector_policy_hash
    {
        dimensions.push("engine".into());
    }
    if current.provenance.configuration.hash != previous.provenance.configuration.hash {
        dimensions.push("configuration".into());
    }
    if current.provenance.source != previous.provenance.source {
        dimensions.push(
            if current.provenance.source.git_revision.is_some()
                && previous.provenance.source.git_revision.is_some()
            {
                "source"
            } else {
                "unknown"
            }
            .into(),
        );
    }
    dimensions
}

fn change_origin(dimensions: &[String]) -> BaselineChangeOrigin {
    match dimensions {
        [dimension] if dimension == "engine" => BaselineChangeOrigin::Engine,
        [dimension] if dimension == "configuration" => BaselineChangeOrigin::Configuration,
        [dimension] if dimension == "source" => BaselineChangeOrigin::Source,
        [dimension] if dimension == "unknown" => BaselineChangeOrigin::Unknown,
        [] => BaselineChangeOrigin::Unknown,
        _ => BaselineChangeOrigin::Mixed,
    }
}

fn propagate_finding_changes(
    issues: &mut BaselineDifferenceSet,
    findings: &BaselineDifferenceSet,
    current: &[Issue],
    previous: &[Issue],
) -> Result<()> {
    let changed_ids = findings
        .changed
        .iter()
        .map(|change| change.id.as_str())
        .collect::<BTreeSet<_>>();
    let already_changed = issues
        .changed
        .iter()
        .map(|change| change.id.clone())
        .collect::<BTreeSet<_>>();
    let previous_by_id = previous
        .iter()
        .map(|issue| (issue.id.as_str(), issue))
        .collect::<BTreeMap<_, _>>();
    for issue in current.iter().filter(|issue| {
        !already_changed.contains(issue.id.as_str())
            && issue
                .finding_ids
                .iter()
                .any(|id| changed_ids.contains(id.as_str()))
    }) {
        let Some(before) = previous_by_id.get(issue.id.as_str()) else {
            continue;
        };
        issues.unchanged_count = issues.unchanged_count.saturating_sub(1);
        issues.changed.push(BaselineChanged {
            id: issue.id.to_string(),
            origin: findings
                .changed
                .iter()
                .find(|change| issue.finding_ids.iter().any(|id| id.as_str() == change.id))
                .map_or(BaselineChangeOrigin::Unknown, |change| change.origin),
            changed_fields: vec!["supporting_findings".into()],
            before: serde_json::to_value(before)?,
            after: serde_json::to_value(issue)?,
        });
    }
    issues.changed.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(())
}

fn lineage_candidates(
    current: &ScanReport,
    previous: &ScanReport,
    finding_diff: &BaselineDifferenceSet,
    issue_diff: &BaselineDifferenceSet,
) -> Vec<LineageCandidate> {
    let removed_finding_ids = change_ids(&finding_diff.removed);
    let added_finding_ids = change_ids(&finding_diff.added);
    let mut finding_candidates = Vec::new();
    for old in previous
        .findings
        .iter()
        .filter(|item| removed_finding_ids.contains(item.id.as_str()))
    {
        for new in current
            .findings
            .iter()
            .filter(|item| added_finding_ids.contains(item.id.as_str()) && item.kind == old.kind)
        {
            if let Some((confidence, reasons)) = finding_lineage_score(old, new) {
                finding_candidates.push(candidate(
                    LineageEntity::Finding,
                    old.id.as_str(),
                    new.id.as_str(),
                    confidence,
                    reasons,
                ));
            }
        }
    }

    let removed_issue_ids = change_ids(&issue_diff.removed);
    let added_issue_ids = change_ids(&issue_diff.added);
    let finding_scores = lineage_score_map(&finding_candidates);
    let mut issue_candidates = Vec::new();
    for old in previous
        .issues
        .iter()
        .filter(|item| removed_issue_ids.contains(item.id.as_str()))
    {
        for new in current
            .issues
            .iter()
            .filter(|item| added_issue_ids.contains(item.id.as_str()) && item.family == old.family)
        {
            let scores = old
                .finding_ids
                .iter()
                .flat_map(|old_id| {
                    new.finding_ids.iter().filter_map(|new_id| {
                        finding_scores
                            .get(&(old_id.as_str(), new_id.as_str()))
                            .copied()
                    })
                })
                .collect::<Vec<_>>();
            if let Some(confidence) = scores.iter().max().copied() {
                issue_candidates.push(candidate(
                    LineageEntity::Issue,
                    old.id.as_str(),
                    new.id.as_str(),
                    confidence,
                    vec![
                        format!("{} supporting finding match(es)", scores.len()),
                        "same issue family".into(),
                    ],
                ));
            }
        }
    }
    finding_candidates.extend(issue_candidates);
    finding_candidates.sort_by(|left, right| left.id.cmp(&right.id));
    finding_candidates
}

fn change_ids(changes: &[BaselineChange]) -> BTreeSet<&str> {
    changes.iter().map(|item| item.id.as_str()).collect()
}

fn lineage_score_map(candidates: &[LineageCandidate]) -> BTreeMap<(&str, &str), u8> {
    candidates
        .iter()
        .map(|item| {
            (
                (item.previous_id.as_str(), item.current_id.as_str()),
                item.confidence_percent,
            )
        })
        .collect()
}

fn finding_lineage_score(old: &Finding, new: &Finding) -> Option<(u8, Vec<String>)> {
    let old_paths = finding_paths(old);
    let new_paths = finding_paths(new);
    let path_overlap = !old_paths.is_disjoint(&new_paths);
    let old_symbols = finding_symbols(old);
    let new_symbols = finding_symbols(new);
    let symbol_overlap = !old_symbols.is_disjoint(&new_symbols);
    if !path_overlap && !symbol_overlap {
        return None;
    }
    let mut score = 0u8;
    let mut reasons = Vec::new();
    if path_overlap {
        score += 35;
        reasons.push("overlapping paths".into());
    }
    if symbol_overlap {
        score += 35;
        reasons.push("overlapping symbols".into());
    }
    let old_metrics = old
        .metrics
        .iter()
        .map(|metric| metric.name)
        .collect::<BTreeSet<_>>();
    let new_metrics = new
        .metrics
        .iter()
        .map(|metric| metric.name)
        .collect::<BTreeSet<_>>();
    if !old_metrics.is_disjoint(&new_metrics) {
        score += 20;
        reasons.push("overlapping metric names".into());
    }
    if Path::new(&old.path).file_name() == Path::new(&new.path).file_name() {
        score += 10;
        reasons.push("same primary basename".into());
    }
    (score >= 60).then_some((score, reasons))
}

fn finding_paths(finding: &Finding) -> BTreeSet<String> {
    std::iter::once(finding.path.replace('\\', "/"))
        .chain(
            finding
                .related_locations
                .iter()
                .map(|location| location.path.replace('\\', "/")),
        )
        .collect()
}

fn finding_symbols(finding: &Finding) -> BTreeSet<String> {
    let mut symbols = finding
        .related_locations
        .iter()
        .filter_map(|location| location.name.clone())
        .collect::<BTreeSet<_>>();
    if let Some(witness) = &finding.flow_witness {
        symbols.insert(witness.source.name.clone());
        symbols.insert(witness.sink.name.clone());
    }
    symbols
}

fn candidate(
    entity: LineageEntity,
    previous_id: &str,
    current_id: &str,
    confidence_percent: u8,
    reasons: Vec<String>,
) -> LineageCandidate {
    let key = serde_json::json!({"algorithm": "lineage-v1", "entity": entity, "previous": previous_id, "current": current_id});
    let digest = fingerprint_json(&key);
    LineageCandidate {
        id: format!("rl1-{}", &digest[7..23]),
        entity,
        previous_id: previous_id.into(),
        current_id: current_id.into(),
        confidence_percent,
        reasons,
    }
}

fn validate_baseline_schema(path: &Path, contents: &str) -> Result<()> {
    let version = if is_json(path) {
        serde_json::from_str::<Value>(contents)
            .ok()
            .and_then(|value| value.get("schema_version").and_then(Value::as_u64))
    } else {
        serde_yaml::from_str::<serde_yaml::Value>(contents)
            .ok()
            .and_then(|value| {
                value
                    .get("schema_version")
                    .and_then(serde_yaml::Value::as_u64)
            })
    };
    ensure!(
        version == Some(u64::from(SCAN_REPORT_SCHEMA_VERSION)),
        "baseline {} uses unsupported schema version {}; schema 23 baselines are required and schema 22 or earlier reports are not compatible",
        path.display(),
        version.map_or_else(|| "unknown".into(), |value| value.to_string())
    );
    Ok(())
}

fn validate_stable_ids(path: &Path, report: &ScanReport) -> Result<()> {
    if report
        .issues
        .iter()
        .all(|issue| valid_stable_id(issue.id.as_str(), "ri3-"))
        && report
            .findings
            .iter()
            .all(|finding| valid_stable_id(finding.id.as_str(), "rf3-"))
    {
        return Ok(());
    }
    bail!(
        "baseline {} contains invalid Stable IDs; schema 23 requires ri3-* issues and rf3-* findings",
        path.display()
    )
}

fn valid_stable_id(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|digest| {
        digest.len() == 16 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn validate_provenance(path: &Path, report: &ScanReport) -> Result<()> {
    ensure!(
        !report.provenance.engine.version.trim().is_empty()
            && report.provenance.configuration.effective.is_object()
            && valid_sha256(&report.provenance.configuration.hash)
            && valid_sha256(&report.provenance.detector_policy_hash),
        "baseline {} contains incomplete or invalid schema 23 provenance",
        path.display()
    );
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256-").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn is_json(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{ChurnMode, ScanArgs};
    use crate::scan::NoopProgress;

    #[test]
    fn rejects_schema_22_baselines_before_deserialization() {
        let error =
            validate_baseline_schema(Path::new("baseline.json"), r#"{"schema_version":22}"#)
                .unwrap_err();
        assert!(error.to_string().contains("schema 23"));
        assert!(error.to_string().contains("schema 22 or earlier"));
    }

    #[test]
    fn same_id_payload_change_propagates_to_the_issue_diff() -> Result<()> {
        let (root, previous) = test_report("changed")?;
        let mut current = previous.clone();
        current.findings[0]
            .message
            .push_str(" with changed semantics");

        let comparison = compare_reports(&current, &previous, None)?;

        assert_eq!(comparison.findings.changed.len(), 1);
        assert!(
            comparison.findings.changed[0]
                .changed_fields
                .contains(&"message".into())
        );
        assert_eq!(comparison.issues.changed.len(), 1);
        assert_eq!(
            comparison.issues.changed[0].changed_fields,
            ["supporting_findings"]
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn lineage_candidate_is_deterministic_for_a_moved_function() -> Result<()> {
        let (root, previous) = test_report("lineage")?;
        let mut current = previous.clone();
        current.findings[0].line = current.findings[0].line.map(|line| line + 1);
        current.findings[0].refresh_id();
        current.issues = crate::evidence_analysis::cluster_findings(&mut current.findings);

        let first = compare_reports(&current, &previous, None)?;
        let second = compare_reports(&current, &previous, None)?;

        assert_eq!(first.lineage_candidates, second.lineage_candidates);
        assert!(first.lineage_candidates.iter().any(|candidate| {
            candidate.entity == LineageEntity::Finding && candidate.confidence_percent >= 60
        }));
        assert!(first.lineage_candidates.iter().any(|candidate| {
            candidate.entity == LineageEntity::Issue && candidate.id.starts_with("rl1-")
        }));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    fn test_report(name: &str) -> Result<(std::path::PathBuf, ScanReport)> {
        let root = std::env::temp_dir().join(format!(
            "reforge-baseline-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("src"))?;
        std::fs::write(
            root.join("src/lib.rs"),
            "fn example() {\n let a = 1;\n let b = 2;\n let c = 3;\n let d = 4;\n let _ = a + b + c + d;\n}\n",
        )?;
        let mut args = ScanArgs::defaults_for_path(root.clone());
        args.churn = Some(ChurnMode::Off);
        args.max_function_lines = 1;
        args.finding_controls.only = Some("long_function".into());
        let mut progress = NoopProgress;
        let report = crate::scan::scan_report(&args, &mut progress)?;
        ensure!(
            !report.findings.is_empty(),
            "test scan should emit a long function"
        );
        Ok((root, report))
    }
}
