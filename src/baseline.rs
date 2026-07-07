use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::cli::BaselineMode;
use crate::model::{Finding, SCAN_REPORT_SCHEMA_VERSION, ScanReport, Severity};

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
    validate_finding_ids(path, &report)?;
    Ok(report)
}

pub(crate) fn selected_findings<'a>(
    current: &'a [Finding],
    baseline: Option<&ScanReport>,
    mode: BaselineMode,
) -> Vec<&'a Finding> {
    let Some(baseline) = baseline else {
        return current.iter().collect();
    };
    if mode == BaselineMode::All {
        return current.iter().collect();
    }

    let previous = baseline
        .findings
        .iter()
        .map(|finding| (finding.id.as_str(), finding))
        .collect::<BTreeMap<_, _>>();

    current
        .iter()
        .filter(|finding| match previous.get(finding.id.as_str()) {
            None => true,
            Some(previous) => mode == BaselineMode::NewOrWorse && is_worse(finding, previous),
        })
        .collect()
}

pub(crate) fn gate_failures<'a>(
    selected: impl IntoIterator<Item = &'a Finding>,
    threshold: crate::cli::FailOnSeverity,
) -> Vec<&'a Finding> {
    selected
        .into_iter()
        .filter(|finding| threshold.matches(finding.severity))
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
        "baseline {} uses schema version {}; regenerate the baseline with Reforge schema {} so findings include stable IDs",
        path.display(),
        version
            .map(|version| version.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        SCAN_REPORT_SCHEMA_VERSION
    )
}

fn validate_finding_ids(path: &Path, report: &ScanReport) -> Result<()> {
    if report
        .findings
        .iter()
        .all(|finding| finding.id.starts_with("rf1-"))
    {
        return Ok(());
    }

    bail!(
        "baseline {} contains findings without stable IDs; regenerate the baseline with Reforge schema {}",
        path.display(),
        SCAN_REPORT_SCHEMA_VERSION
    )
}

fn is_worse(current: &Finding, previous: &Finding) -> bool {
    severity_rank(current.severity) > severity_rank(previous.severity)
        || current.priority > previous.priority
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
    use crate::model::{FindingKind, PriorityFactors};

    fn finding(id: &str, priority: u8, severity: Severity) -> Finding {
        Finding {
            id: id.to_string(),
            kind: FindingKind::LargeFile,
            severity,
            path: "src/a.rs".to_string(),
            line: Some(1),
            metrics: Vec::new(),
            priority,
            confidence: 1.0,
            priority_factors: PriorityFactors {
                impact: 0.0,
                intensity: 0.0,
                spread: 0.0,
                change_pressure: 0.0,
                actionability: 0.0,
                confidence: 1.0,
            },
            rank_explanation: String::new(),
            message: String::new(),
            related_locations: Vec::new(),
        }
    }

    fn baseline(findings: Vec<Finding>) -> ScanReport {
        ScanReport {
            schema_version: SCAN_REPORT_SCHEMA_VERSION,
            summary: crate::model::ScanSummary {
                scanned_files: 0,
                finding_count: findings.len(),
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
                files: BTreeMap::new(),
                functions: BTreeMap::new(),
                types: BTreeMap::new(),
                churn: BTreeMap::new(),
            },
            raw_metrics: crate::model::RawMetrics::default(),
            hotspots: Vec::new(),
            findings,
        }
    }

    #[test]
    fn baseline_mode_new_selects_only_absent_ids() {
        let old = baseline(vec![finding("rf1-old", 50, Severity::Warning)]);
        let current = vec![
            finding("rf1-old", 70, Severity::Critical),
            finding("rf1-new", 50, Severity::Warning),
        ];

        let selected = selected_findings(&current, Some(&old), BaselineMode::New);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, "rf1-new");
    }

    #[test]
    fn baseline_mode_new_or_worse_selects_absent_and_higher_priority_ids() {
        let old = baseline(vec![
            finding("rf1-same", 50, Severity::Warning),
            finding("rf1-worse", 50, Severity::Warning),
        ]);
        let current = vec![
            finding("rf1-same", 50, Severity::Warning),
            finding("rf1-worse", 60, Severity::Warning),
            finding("rf1-new", 20, Severity::Info),
        ];

        let selected = selected_findings(&current, Some(&old), BaselineMode::NewOrWorse);

        assert_eq!(
            selected
                .iter()
                .map(|finding| finding.id.as_str())
                .collect::<Vec<_>>(),
            ["rf1-worse", "rf1-new"]
        );
    }
}
