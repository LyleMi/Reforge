//! Stable, producer-neutral contracts shared by every Reforge tool.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const REPORT_SCHEMA_VERSION: u16 = 26;
pub const ANALYSIS_CODEBASE: &str = "codebase";
pub const ANALYSIS_DATAFLOW: &str = "dataflow";
pub const ANALYSIS_UNITY: &str = "unity";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    Observed,
    Partial,
    Unsupported,
    NotApplicable,
}

impl CoverageStatus {
    pub fn is_observable(self) -> bool {
        matches!(self, Self::Observed | Self::Partial)
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::Observed => 3,
            Self::Partial => 2,
            Self::NotApplicable => 1,
            Self::Unsupported => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageCoverage {
    pub status: CoverageStatus,
    pub files: usize,
    pub functions: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<CoverageLimitation>,
}

impl Default for LanguageCoverage {
    fn default() -> Self {
        Self {
            status: CoverageStatus::Observed,
            files: 0,
            functions: 0,
            limitations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageObservation {
    pub name: String,
    pub count: usize,
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleExecution {
    pub status: CoverageStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observations: Vec<CoverageObservation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<CoverageLimitation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageLimitation {
    pub code: String,
    pub count: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisCoverage {
    pub status: CoverageStatus,
    pub scanned_files: usize,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub languages: BTreeMap<String, LanguageCoverage>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rules: BTreeMap<String, RuleExecution>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<CoverageLimitation>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Subject {
    Repository,
    Directory { path: String },
    File { path: String },
    Symbol { path: String, symbol: String },
    Group { members: Vec<String> },
}

impl Subject {
    pub fn canonicalized(mut self) -> Self {
        match &mut self {
            Self::Repository => {}
            Self::Directory { path } | Self::File { path } | Self::Symbol { path, .. } => {
                *path = canonical_path(path);
            }
            Self::Group { members } => {
                for member in &mut *members {
                    *member = canonical_member(member);
                }
                members.sort();
                members.dedup();
            }
        }
        self
    }

    pub fn identity(&self) -> String {
        match self.clone().canonicalized() {
            Self::Repository => "repository".into(),
            Self::Directory { path } => format!("directory:{path}"),
            Self::File { path } => format!("file:{path}"),
            Self::Symbol { path, symbol } => format!("symbol:{path}:{symbol}"),
            Self::Group { members } => format!("group:{}", members.join("|")),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Self::Repository => "repository".into(),
            Self::Directory { path } | Self::File { path } => canonical_path(path),
            Self::Symbol { path, symbol } => {
                format!("{} in {}", symbol, canonical_path(path))
            }
            Self::Group { members } => {
                let count = members.len();
                format!("{count} related items")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    pub name: String,
    pub value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    pub unit: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowResolution {
    Exact,
    Partial,
    Unresolved,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowEndpoint {
    pub path: String,
    pub symbol: String,
    pub language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowStep {
    pub path: String,
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    pub operation: String,
    pub resolution: FlowResolution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowWitness {
    pub source: FlowEndpoint,
    pub sink: FlowEndpoint,
    pub ordered_steps: Vec<FlowStep>,
    pub function_hops: usize,
    pub module_hops: usize,
    pub resolution: FlowResolution,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub id: String,
    pub rule: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub measurements: Vec<Measurement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<Location>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub witness: Option<FlowWitness>,
}

impl Evidence {
    pub fn new(rule: impl Into<String>, semantic_anchor: &str, message: impl Into<String>) -> Self {
        let rule = rule.into();
        Self {
            id: evidence_id(&rule, semantic_anchor),
            rule,
            message: message.into(),
            measurements: Vec::new(),
            locations: Vec::new(),
            witness: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Issue {
    pub id: String,
    pub analysis: String,
    pub family: String,
    pub subject: Subject,
    pub title: String,
    pub guidance: String,
    pub evidence: Vec<Evidence>,
}

impl Issue {
    pub fn new<Title, Guidance>(
        analysis: impl Into<String>,
        family: impl Into<String>,
        subject: Subject,
        content: (Title, Guidance),
        mut evidence: Vec<Evidence>,
    ) -> Self
    where
        Title: Into<String>,
        Guidance: Into<String>,
    {
        let analysis = analysis.into();
        let family = family.into();
        let subject = subject.canonicalized();
        let (title, guidance) = content;
        evidence.sort_by(|left, right| left.id.cmp(&right.id));
        Self {
            id: issue_id(&family, &subject),
            analysis,
            family,
            subject,
            title: title.into(),
            guidance: guidance.into(),
            evidence,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineComparison {
    pub new_issue_ids: Vec<String>,
    pub resolved_issue_ids: Vec<String>,
    pub unchanged_issue_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema_version: u16,
    pub producer: Producer,
    pub target: Target,
    pub summary: ReportSummary,
    pub suppression: SuppressionSummary,
    pub coverage: BTreeMap<String, AnalysisCoverage>,
    pub issues: Vec<Issue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_comparison: Option<BaselineComparison>,
}

impl Report {
    pub fn new(
        producer: Producer,
        target: Target,
        suppression: SuppressionSummary,
        coverage: BTreeMap<String, AnalysisCoverage>,
        mut issues: Vec<Issue>,
    ) -> Self {
        issues.sort_by(|left, right| left.id.cmp(&right.id));
        let evidence_count = issues.iter().map(|issue| issue.evidence.len()).sum();
        Self {
            schema_version: REPORT_SCHEMA_VERSION,
            producer,
            target,
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
                "unsupported Reforge report schema {}; expected schema 26; regenerate the report with Reforge 0.2",
                self.schema_version
            );
        }
        validate_namespace("producer name", &self.producer.name)?;
        if self.coverage.is_empty() {
            bail!("report coverage must name at least one analysis");
        }
        validate_coverage(&self.coverage)?;
        validate_issues(&self.issues, &self.coverage)?;
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
        if self.producer != baseline.producer {
            bail!("baseline producer does not match the current report");
        }
        if self.target.workspace_identity != baseline.target.workspace_identity {
            bail!("baseline workspace does not match the current report");
        }
        let current = self.coverage.keys().collect::<BTreeSet<_>>();
        let previous = baseline.coverage.keys().collect::<BTreeSet<_>>();
        if current != previous {
            bail!("baseline analysis set does not match the current report");
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
            .map(|issue| &issue.id)
            .collect::<BTreeSet<_>>();
        let previous = baseline
            .issues
            .iter()
            .map(|issue| &issue.id)
            .collect::<BTreeSet<_>>();
        let degraded = self
            .coverage_downgrades(baseline)
            .into_iter()
            .collect::<BTreeSet<_>>();
        BaselineComparison {
            new_issue_ids: current
                .difference(&previous)
                .map(|id| (*id).clone())
                .collect(),
            resolved_issue_ids: previous
                .difference(&current)
                .filter(|id| {
                    baseline
                        .issues
                        .iter()
                        .find(|issue| issue.id == ***id)
                        .is_none_or(|issue| !degraded.contains(&issue.analysis))
                })
                .map(|id| (*id).clone())
                .collect(),
            unchanged_issue_count: current.intersection(&previous).count(),
        }
    }
}

include!("validation.rs");

#[cfg(test)]
mod tests;
