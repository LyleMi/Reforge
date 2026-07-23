use super::*;
use crate::execution::ChurnMode;

include!("data_flow.rs");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSummary {
    pub scanned_files: usize,
    pub detected_evidence_count: usize,
    pub similar_function_group_count: usize,
    pub duration_ms: u128,
    pub churn: ChurnSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RunStats {
    pub source_files_discovered: usize,
    pub source_files_analyzed: usize,
    pub directories_scanned: usize,
    pub function_candidates: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuppressionSummary {
    pub suppressed_count: usize,
    pub suppressed_by_kind: BTreeMap<Rule, usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSpec {
    pub kind: Rule,
    pub rule: String,
    pub analysis: String,
    pub family: IssueFamily,
    pub subject: SubjectKind,
    pub observation_source: ObservationSource,
    pub languages: Vec<String>,
    pub measurements: Vec<MetricId>,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueFamily {
    FunctionReadability,
    DocumentationIntegrity,
    ImplementationDuplication,
    DependencyTopology,
    ModuleSurface,
    BoundaryIntegrity,
    ResponsibilityDecomposition,
    DirectoryOrganization,
    LiteralOwnership,
    DataShapeDuplication,
    ErrorHandlingDuplication,
    TestSupport,
    TestCoverage,
    DeadCode,
    DeclaredDebt,
    Naming,
    CompatibilityRetirement,
    DataflowOwnership,
}

impl SuppressionSummary {
    pub fn record(&mut self, detection: &DetectedEvidence) {
        self.suppressed_count += 1;
        *self.suppressed_by_kind.entry(detection.kind).or_insert(0) += 1;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    pub source_revision: Option<String>,
    pub summary: RunSummary,
    pub stats: RunStats,
    pub raw_metrics: RawMetrics,
    pub suppression_summary: SuppressionSummary,
    pub flow_analysis: FlowAnalysisSummary,
    pub parse_failures: Vec<ParseFailure>,
    pub source_failures: Vec<SourceFailure>,
    pub rule_execution: BTreeMap<Rule, reforge_schema::RuleExecution>,
    pub detected_evidence: Vec<DetectedEvidence>,
}
