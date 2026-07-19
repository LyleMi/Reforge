use super::*;
use crate::cli::{ChurnMode, HotspotModel};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanSummary {
    pub scanned_files: usize,
    pub finding_count: usize,
    pub issue_count: usize,
    #[serde(default, skip)]
    pub hotspot_count: usize,
    pub similar_function_group_count: usize,
    pub duration_ms: u128,
    #[serde(default, skip)]
    pub hotspot_model: HotspotModel,
    pub churn: ChurnSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ScanStats {
    pub source_files_scanned: usize,
    pub directories_scanned: usize,
    pub function_candidates: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChurnSummary {
    pub mode: ChurnMode,
    pub enabled: bool,
    pub status: String,
    pub reason: Option<String>,
    pub window_days: usize,
    pub max_commit_lines: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ChurnFileMetric {
    pub commits_touched: usize,
    pub lines_added: usize,
    pub lines_deleted: usize,
    pub authors_count: usize,
    pub recent_weighted_churn: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRawMetric {
    pub path: String,
    pub loc: usize,
    pub imports: usize,
    pub public_items: usize,
    pub is_test: bool,
    pub churn: ChurnFileMetric,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryRawMetric {
    pub path: String,
    pub source_files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionRawMetric {
    pub path: String,
    pub name: String,
    pub line: usize,
    pub loc: usize,
    pub complexity: usize,
    pub nesting_depth: usize,
    pub parameter_count: usize,
    pub is_test: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeRawMetric {
    pub path: String,
    pub name: String,
    pub line: usize,
    pub loc: usize,
    pub member_count: usize,
    pub is_test: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RawMetrics {
    pub directories: Vec<DirectoryRawMetric>,
    pub files: Vec<FileRawMetric>,
    pub functions: Vec<FunctionRawMetric>,
    pub types: Vec<TypeRawMetric>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DependencyGraphSnapshot {
    pub nodes: Vec<DependencyGraphNode>,
    pub edges: Vec<DependencyGraphEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyGraphNode {
    pub path: String,
    pub fan_in: usize,
    pub fan_out: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyGraphEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentEvidenceCoverageStatus {
    Observed,
    Partial,
    Unsupported,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AgentTestReachability {
    pub direct_test_files: Vec<String>,
    pub reachable_test_files: Vec<String>,
    pub reachable_test_file_count: usize,
    pub nearest_test_distance: Option<usize>,
    pub nearest_test_paths: Vec<String>,
    pub paths_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileAgentEvidence {
    pub path: String,
    pub coverage_status: AgentEvidenceCoverageStatus,
    pub context_closure_files: usize,
    pub context_closure_loc: usize,
    pub unresolved_local_dependencies: usize,
    #[serde(flatten)]
    pub test_reachability: AgentTestReachability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceDispersion {
    pub evidence_files: Vec<String>,
    pub evidence_directories: Vec<String>,
    pub evidence_languages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueAgentEvidence {
    pub issue_id: IssueKey,
    pub coverage_status: AgentEvidenceCoverageStatus,
    pub evidence_dispersion: EvidenceDispersion,
    pub context_closure_files: usize,
    pub context_closure_loc: usize,
    pub unresolved_local_dependencies: usize,
    #[serde(flatten)]
    pub test_reachability: AgentTestReachability,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AgentEvidence {
    pub files: Vec<FileAgentEvidence>,
    pub issues: Vec<IssueAgentEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricPercentiles {
    pub p50: usize,
    pub p75: usize,
    pub p90: usize,
    pub p95: usize,
    pub max: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub directories: BTreeMap<String, MetricPercentiles>,
    pub files: BTreeMap<String, MetricPercentiles>,
    pub functions: BTreeMap<String, MetricPercentiles>,
    pub types: BTreeMap<String, MetricPercentiles>,
    pub churn: BTreeMap<String, MetricPercentiles>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SuppressionSummary {
    pub suppressed_count: usize,
    pub suppressed_by_kind: BTreeMap<FindingKind, usize>,
    #[serde(default, skip)]
    pub suppressed_by_severity: BTreeMap<Severity, usize>,
    #[serde(default, skip)]
    pub highest_suppressed_priority: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub id: IssueKey,
    pub family: String,
    pub summary: String,
    pub construct: QualityConstruct,
    pub mechanism: SignalMechanism,
    pub action: RefactorAction,
    pub path: String,
    pub line: Option<usize>,
    pub primary_finding_id: EvidenceId,
    pub finding_ids: Vec<EvidenceId>,
    pub kinds: Vec<FindingKind>,
    #[serde(default, skip)]
    pub priority: u8,
    #[serde(default, skip)]
    pub severity: Severity,
    #[serde(default, skip)]
    pub priority_factors: PriorityFactors,
    pub subject: EvidenceSubject,
    #[serde(default, skip)]
    pub detection_reliability: f64,
    #[serde(default, skip)]
    pub interpretation_reliability: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectorManifestEntry {
    pub kind: FindingKind,
    pub construct: QualityConstruct,
    pub mechanism: SignalMechanism,
    pub action: RefactorAction,
    pub entity_scope: EntityScope,
    pub approach: DetectionApproach,
    pub supported_languages: Vec<String>,
    pub precision_risk: PrecisionRisk,
    pub input_metrics: Vec<MetricId>,
    pub issue_family: String,
    pub evidence_role: EvidenceRole,
    pub constituent_kinds: Vec<FindingKind>,
    #[serde(default, skip)]
    pub default_detection_reliability: f64,
    #[serde(default, skip)]
    pub default_interpretation_reliability: f64,
    #[serde(default, skip)]
    pub impact: f64,
    #[serde(default, skip)]
    pub actionability: f64,
}

impl SuppressionSummary {
    pub fn record(&mut self, finding: &Finding) {
        self.suppressed_count += 1;
        *self.suppressed_by_kind.entry(finding.kind).or_insert(0) += 1;
        *self
            .suppressed_by_severity
            .entry(finding.severity)
            .or_insert(0) += 1;
        self.highest_suppressed_priority = Some(
            self.highest_suppressed_priority
                .unwrap_or(0)
                .max(finding.priority),
        );
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanReport {
    pub schema_version: u8,
    pub summary: ScanSummary,
    pub stats: ScanStats,
    pub metrics_summary: MetricsSummary,
    pub raw_metrics: RawMetrics,
    pub raw_metric_manifest: Vec<RawMetricManifestEntry>,
    pub dependency_graph: DependencyGraphSnapshot,
    pub agent_evidence: AgentEvidence,
    pub unity_project: UnityProjectReport,
    #[serde(default, skip)]
    pub hotspots: Vec<Hotspot>,
    pub suppression_summary: SuppressionSummary,
    pub coverage_manifest: Vec<CoverageManifestEntry>,
    pub coverage_summary: CoverageSummary,
    pub detector_execution: Vec<DetectorExecutionReceipt>,
    pub raw_metric_coverage: Vec<RawMetricCoverage>,
    #[serde(default, skip)]
    pub scoring_policy: EffectiveScoringPolicy,
    pub issues: Vec<Issue>,
    pub detector_manifest: Vec<DetectorManifestEntry>,
    pub findings: Vec<Finding>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotspotLevel {
    File,
    Function,
    Type,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hotspot {
    pub level: HotspotLevel,
    pub path: String,
    pub line: Option<usize>,
    pub name: Option<String>,
    pub priority: u8,
    pub severity: Severity,
    pub static_risk: f64,
    pub churn_risk: f64,
    pub reason: String,
}
