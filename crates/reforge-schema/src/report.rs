use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::{
    AnalysisCoverage, AnalysisProvenance, BaselineComparison, BaselineEntry, BaselineState, Issue,
    REPORT_SCHEMA_VERSION, ReportProvenance, RuleProvenance, comparison_entry,
    coverage_is_downgraded, stable_id, validate_coverage, validate_issues, validate_namespace,
    validate_provenance, validate_rule_provenance,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Producer {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub root: String,
    pub workspace_identity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_revision: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportSummary {
    pub issue_count: usize,
    pub evidence_count: usize,
    pub scanned_files: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuppressionSummary {
    pub evidence_count: usize,
    pub by_rule: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReportInput {
    pub producer: Producer,
    pub target: Target,
    pub provenance: ReportProvenance,
    pub suppression: SuppressionSummary,
    pub coverage: BTreeMap<String, AnalysisCoverage>,
    pub issues: Vec<Issue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema_version: u16,
    pub producer: Producer,
    pub target: Target,
    pub provenance: ReportProvenance,
    pub summary: ReportSummary,
    pub suppression: SuppressionSummary,
    pub coverage: BTreeMap<String, AnalysisCoverage>,
    pub issues: Vec<Issue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_comparison: Option<BaselineComparison>,
}

impl Report {
    pub fn new(input: ReportInput) -> Self {
        let ReportInput {
            producer,
            target,
            provenance,
            suppression,
            coverage,
            mut issues,
        } = input;
        issues.sort_by(|left, right| left.id.cmp(&right.id));
        let evidence_count = issues.iter().map(|issue| issue.evidence.len()).sum();
        Self {
            schema_version: REPORT_SCHEMA_VERSION,
            producer,
            target,
            provenance,
            summary: ReportSummary {
                issue_count: issues.len(),
                evidence_count,
                scanned_files: coverage
                    .values()
                    .map(|analysis| analysis.scanned_files)
                    .max()
                    .unwrap_or_default(),
            },
            suppression,
            coverage,
            issues,
            baseline_comparison: None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != REPORT_SCHEMA_VERSION {
            bail!(
                "unsupported Reforge report schema {}; expected schema 27; see docs/upgrading-to-0.2.md and regenerate the report with Reforge 0.2",
                self.schema_version
            );
        }
        validate_namespace("producer name", &self.producer.name)?;
        validate_provenance(&self.provenance, &self.coverage)?;
        if self.coverage.is_empty() {
            bail!("report coverage must name at least one analysis");
        }
        validate_coverage(&self.coverage)?;
        validate_issues(&self.issues, &self.coverage)?;
        for rule in self
            .issues
            .iter()
            .flat_map(|issue| issue.evidence.iter().map(|evidence| evidence.rule.as_str()))
        {
            validate_rule_provenance(&self.provenance, rule)?;
        }
        if self.summary.issue_count != self.issues.len()
            || self.summary.evidence_count
                != self
                    .issues
                    .iter()
                    .map(|issue| issue.evidence.len())
                    .sum::<usize>()
            || self.summary.scanned_files
                != self
                    .coverage
                    .values()
                    .map(|analysis| analysis.scanned_files)
                    .max()
                    .unwrap_or_default()
        {
            bail!("report summary does not match coverage and issue contents");
        }
        Ok(())
    }

    pub fn validate_baseline(&self, baseline: &Self) -> Result<()> {
        if self.producer.name != baseline.producer.name {
            bail!("baseline producer does not match the current report");
        }
        if self.provenance.identity_scheme != baseline.provenance.identity_scheme {
            bail!("baseline identity scheme does not match the current report");
        }
        if self.target.workspace_identity != baseline.target.workspace_identity {
            bail!("baseline workspace does not match the current report");
        }
        Ok(())
    }

    pub fn coverage_downgrades(&self, baseline: &Self) -> Vec<String> {
        baseline
            .coverage
            .iter()
            .filter_map(|(analysis, previous)| {
                self.coverage
                    .get(analysis)
                    .filter(|current| coverage_is_downgraded(current, previous))
                    .map(|_| analysis.clone())
            })
            .collect()
    }

    pub fn compare_to(&self, baseline: &Self) -> BaselineComparison {
        let current = self
            .issues
            .iter()
            .map(|issue| (issue.id.as_str(), issue))
            .collect::<BTreeMap<_, _>>();
        let previous = baseline
            .issues
            .iter()
            .map(|issue| (issue.id.as_str(), issue))
            .collect::<BTreeMap<_, _>>();
        let ids = current
            .keys()
            .chain(previous.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        let issues = ids
            .into_iter()
            .map(|id| {
                let entry = match (current.get(id), previous.get(id)) {
                    (Some(after), Some(before)) => BaselineEntry {
                        state: if after.content_fingerprint == before.content_fingerprint {
                            BaselineState::Unchanged
                        } else {
                            BaselineState::Updated
                        },
                        reason: None,
                    },
                    (Some(issue), None) => {
                        comparison_entry(self, baseline, issue, BaselineState::New)
                    }
                    (None, Some(issue)) => {
                        comparison_entry(self, baseline, issue, BaselineState::Absent)
                    }
                    (None, None) => unreachable!(),
                };
                (id.to_owned(), entry)
            })
            .collect();
        BaselineComparison { issues }
    }
}

pub fn default_provenance(
    coverage: &BTreeMap<String, AnalysisCoverage>,
    issues: &[Issue],
) -> ReportProvenance {
    let analyses = coverage
        .keys()
        .map(|analysis| {
            (
                analysis.clone(),
                AnalysisProvenance {
                    config_digest: stable_id("cfg7", &[analysis]),
                    policy_digest: stable_id("pol7", &[analysis]),
                },
            )
        })
        .collect();
    let rules = coverage
        .values()
        .flat_map(|analysis| analysis.rules.keys().cloned())
        .chain(
            issues
                .iter()
                .flat_map(|issue| issue.evidence.iter().map(|evidence| evidence.rule.clone())),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|rule| {
            (
                rule.clone(),
                RuleProvenance {
                    semantic_version: "0.0.0".into(),
                    evaluation_digest: stable_id("eval7", &[&rule]),
                },
            )
        })
        .collect();
    ReportProvenance {
        identity_scheme: crate::IDENTITY_SCHEME.into(),
        scope_digest: stable_id("scope7", &["default"]),
        analyses,
        rules,
    }
}
