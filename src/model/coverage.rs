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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub impact: f64,
    pub intensity: f64,
    pub spread: f64,
    pub change_pressure: f64,
    pub actionability: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            impact: 0.30,
            intensity: 0.30,
            spread: 0.15,
            change_pressure: 0.15,
            actionability: 0.10,
        }
    }
}

impl ScoringWeights {
    pub fn sum(self) -> f64 {
        self.impact + self.intensity + self.spread + self.change_pressure + self.actionability
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectorReliabilityOverride {
    pub detection: f64,
    pub interpretation: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoringPolicy {
    pub policy_id: String,
    pub version: u8,
    pub status: String,
    pub fingerprint: String,
    pub global_weights: ScoringWeights,
    #[serde(default)]
    pub detector_reliability: BTreeMap<FindingKind, DetectorReliabilityOverride>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoringPolicySource {
    Builtin,
    File,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectiveScoringPolicy {
    pub source: ScoringPolicySource,
    pub path: Option<String>,
    pub policy_id: String,
    pub version: u8,
    pub fingerprint: String,
    pub global_weights: ScoringWeights,
    pub detector_reliability: BTreeMap<FindingKind, DetectorReliabilityOverride>,
}

impl EffectiveScoringPolicy {
    pub fn builtin() -> Self {
        let weights = ScoringWeights::default();
        Self {
            source: ScoringPolicySource::Builtin,
            path: None,
            policy_id: "reforge-theoretical-prior".into(),
            version: 1,
            fingerprint: policy_fingerprint(
                "reforge-theoretical-prior",
                1,
                weights,
                &BTreeMap::new(),
            ),
            global_weights: weights,
            detector_reliability: BTreeMap::new(),
        }
    }
}

pub fn policy_fingerprint(
    policy_id: &str,
    version: u8,
    weights: ScoringWeights,
    overrides: &BTreeMap<FindingKind, DetectorReliabilityOverride>,
) -> String {
    let mut canonical = format!(
        "policy-v1\0{policy_id}\0{version}\0{:.12}\0{:.12}\0{:.12}\0{:.12}\0{:.12}",
        weights.impact,
        weights.intensity,
        weights.spread,
        weights.change_pressure,
        weights.actionability
    );
    for (kind, value) in overrides {
        canonical.push_str(&format!(
            "\0{}\0{:.12}\0{:.12}",
            serialized_finding_kind(*kind),
            value.detection,
            value.interpretation
        ));
    }
    format!("sp1-{:016x}", fnv1a64(canonical.as_bytes()))
}
