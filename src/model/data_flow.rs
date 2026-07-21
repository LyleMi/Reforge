#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowAnalysisStatus {
    Disabled,
    Observed,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowCapabilityStatus {
    Supported,
    Partial,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowCapability {
    pub language: String,
    pub local_def_use: FlowCapabilityStatus,
    pub direct_calls: FlowCapabilityStatus,
    pub fields: FlowCapabilityStatus,
    pub dynamic_dispatch: FlowCapabilityStatus,
    pub library_models: FlowCapabilityStatus,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowAnalysisSummary {
    pub status: FlowAnalysisStatus,
    pub functions_analyzed: usize,
    pub exact_edges: usize,
    pub unresolved_edges: usize,
    pub truncated_paths: usize,
    pub capabilities: Vec<FlowCapability>,
}

impl Default for FlowAnalysisSummary {
    fn default() -> Self {
        Self {
            status: FlowAnalysisStatus::Disabled,
            functions_analyzed: 0,
            exact_edges: 0,
            unresolved_edges: 0,
            truncated_paths: 0,
            capabilities: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowNodeKind {
    Parameter,
    Local,
    Argument,
    Return,
    CallResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowEdgeKind {
    Assignment,
    ArgumentToParameter,
    ReturnToResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowResolution {
    Exact,
    Unresolved,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowLocation {
    pub id: String,
    pub kind: FlowNodeKind,
    pub path: String,
    pub line: usize,
    pub function: String,
    pub module: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowWitnessStep {
    pub kind: FlowEdgeKind,
    pub resolution: FlowResolution,
    pub from: String,
    pub to: String,
    pub path: String,
    pub line: usize,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowWitness {
    pub policy: String,
    pub source: FlowLocation,
    pub ordered_steps: Vec<FlowWitnessStep>,
    pub sink: FlowLocation,
    pub module_hops: usize,
    pub call_edges: usize,
    pub path_steps: usize,
    pub truncated: bool,
    pub conforming_path: Option<Vec<RelatedLocation>>,
}
