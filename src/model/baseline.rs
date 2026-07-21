use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineProvenance {
    pub version: String,
    pub build_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceProvenance {
    pub git_revision: Option<String>,
    pub dirty: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationProvenance {
    pub effective: serde_json::Value,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportProvenance {
    pub engine: EngineProvenance,
    pub source: SourceProvenance,
    pub configuration: ConfigurationProvenance,
    pub detector_policy_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineChangeOrigin {
    Engine,
    Configuration,
    Source,
    Mixed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineChange {
    pub id: String,
    pub origin: BaselineChangeOrigin,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineChanged {
    pub id: String,
    pub origin: BaselineChangeOrigin,
    pub changed_fields: Vec<String>,
    pub before: serde_json::Value,
    pub after: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineDifferenceSet {
    pub added: Vec<BaselineChange>,
    pub removed: Vec<BaselineChange>,
    pub changed: Vec<BaselineChanged>,
    pub unchanged_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageEntity {
    Finding,
    Issue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineageCandidate {
    pub id: String,
    pub entity: LineageEntity,
    pub previous_id: String,
    pub current_id: String,
    pub confidence_percent: u8,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineComparison {
    pub baseline_path: Option<String>,
    pub baseline_provenance: ReportProvenance,
    pub provenance_changed: bool,
    pub provenance_change_dimensions: Vec<String>,
    pub findings: BaselineDifferenceSet,
    pub issues: BaselineDifferenceSet,
    pub lineage_candidates: Vec<LineageCandidate>,
}
