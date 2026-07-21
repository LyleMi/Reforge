use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageExpectation {
    Required,
    Planned,
    IntentionallyOutOfScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    Observed,
    PartiallyObserved,
    Unsupported,
    NoEntities,
    Planned,
    IntentionallyOutOfScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageManifestEntry {
    pub mechanism: SignalMechanism,
    pub entity_scope: EntityScope,
    pub expectation: CoverageExpectation,
    pub status: CoverageStatus,
    pub reason: String,
    pub detectors: Vec<FindingKind>,
    pub completed_detectors: Vec<FindingKind>,
    pub entity_count: usize,
    pub unobservable_reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectorExecutionStatus {
    Completed,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorExecutionReceipt {
    pub kind: FindingKind,
    pub status: DetectorExecutionStatus,
    pub analyzed_entities: usize,
    pub candidate_groups: usize,
    pub unobservable_count: usize,
    pub unobservable_reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawMetricCoverageStatus {
    Observed,
    PartiallyObserved,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawMetricCoverage {
    pub metric: MetricId,
    pub status: RawMetricCoverageStatus,
    pub entity_count: usize,
    pub reason: String,
    pub unobservable_reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseFailureReason {
    SyntaxError,
    ParserFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseFailure {
    pub path: String,
    pub language: String,
    pub reason: ParseFailureReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CoverageSummary {
    pub detected_languages: Vec<String>,
    pub applicable_detectors: Vec<FindingKind>,
    pub analyzed_entities: BTreeMap<EntityScope, usize>,
    pub parse_failures: Vec<ParseFailure>,
    pub unresolved_dependency_edges: usize,
    pub unobservable_reasons: Vec<String>,
}
