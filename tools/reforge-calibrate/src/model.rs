use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateSite {
    pub(crate) rule: String,
    pub(crate) language: String,
    pub(crate) repository: String,
    pub(crate) candidate: bool,
    pub(crate) quiet: bool,
    pub(crate) fixture_expected: Option<bool>,
    pub(crate) unsupported_expected: bool,
    pub(crate) summary: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReviewPacket {
    pub(crate) packet_schema_version: u16,
    pub(crate) packet_digest: String,
    pub(crate) sites: Vec<PacketSite>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PacketSite {
    pub(crate) id: String,
    pub(crate) rule: String,
    pub(crate) language: String,
    pub(crate) repository_bucket: String,
    pub(crate) candidate: bool,
    pub(crate) quiet: bool,
    pub(crate) fixture_expected: Option<bool>,
    pub(crate) unsupported_expected: bool,
    pub(crate) summary: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Reviewer {
    pub(crate) reviewer_type: String,
    pub(crate) model: String,
    pub(crate) version: String,
    pub(crate) prompt_digest: String,
    pub(crate) timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabelFile {
    pub(crate) label_schema_version: u16,
    pub(crate) packet_digest: String,
    pub(crate) reviewer: Reviewer,
    pub(crate) labels: Vec<SiteLabel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SiteLabel {
    pub(crate) site_id: String,
    pub(crate) judgments: BTreeMap<String, bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CalibrationSummary {
    pub(crate) summary_schema_version: u16,
    pub(crate) validation_basis: String,
    pub(crate) packet_digest: String,
    pub(crate) corpus_digest: String,
    pub(crate) report_digest: String,
    pub(crate) reviewers: Vec<Reviewer>,
    pub(crate) groups: Vec<GroupSummary>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GroupSummary {
    pub(crate) rule: String,
    pub(crate) language: String,
    pub(crate) candidate_sites: usize,
    pub(crate) candidate_repositories: usize,
    pub(crate) max_repository_share: f64,
    pub(crate) quiet_sites: usize,
    pub(crate) quiet_repositories: usize,
    pub(crate) fixture_recall: Rate,
    pub(crate) unsupported_coverage: Rate,
    pub(crate) dimensions: BTreeMap<String, DimensionSummary>,
    pub(crate) eligible_for_stable_advisory: bool,
    pub(crate) failures: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DimensionSummary {
    pub(crate) positive_rate: Rate,
    pub(crate) raw_agreement: f64,
    pub(crate) cohens_kappa: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Rate {
    pub(crate) successes: usize,
    pub(crate) total: usize,
    pub(crate) wilson_95_lower: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct CorpusManifest {
    pub(crate) version: u16,
    pub(crate) frozen_at: String,
    pub(crate) report_schema: u16,
    pub(crate) collection: CollectionMetadata,
    pub(crate) repositories: Vec<CorpusRepository>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct CollectionMetadata {
    pub(crate) clone_command: String,
    pub(crate) codebase_command: String,
    pub(crate) dataflow_command: String,
    pub(crate) combined_command: String,
    pub(crate) reports: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CorpusRepository {
    pub(crate) repository: String,
    pub(crate) language: String,
    pub(crate) commit: String,
    pub(crate) license: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct Matrix {
    pub(crate) include: Vec<CorpusRepository>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReportAudit {
    pub(crate) audit_schema_version: u16,
    pub(crate) report_schema: u16,
    pub(crate) corpus_digest: String,
    pub(crate) repository: String,
    pub(crate) revision: String,
    pub(crate) workspace_identity: String,
    pub(crate) report_digest: String,
    pub(crate) artifacts: BTreeMap<String, String>,
    pub(crate) coverage_status: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PromotionVerification {
    pub(crate) verification_schema_version: u16,
    pub(crate) corpus_digest: String,
    pub(crate) audited_repositories: usize,
    pub(crate) promotion_candidates: usize,
}
