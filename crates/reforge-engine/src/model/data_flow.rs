#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowAnalysisSummary {
    pub functions_analyzed: usize,
    pub exact_edges: usize,
    pub unresolved_edges: usize,
    pub truncated_paths: usize,
    pub policy_configured: bool,
    pub protected_sources_evaluated: usize,
    pub relay_sources_evaluated: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<FlowProgram>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowNodeKind {
    Parameter,
    Local,
    Literal,
    Argument,
    Return,
    CallResult,
    Field,
    GlobalState,
    Capture,
    Source,
    Sink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowEdgeKind {
    Assignment,
    ArgumentToParameter,
    ReturnToResult,
    FieldRead,
    FieldWrite,
    Mutation,
    Construction,
    Transformation,
    Capture,
    Branch,
    Merge,
    SourceBinding,
    SinkBinding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowResolution {
    Exact,
    Partial,
    Unresolved,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowLocation {
    pub id: String,
    pub kind: FlowNodeKind,
    pub language: String,
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
    pub function_hops: usize,
    pub call_edges: usize,
    pub path_steps: usize,
    pub truncated: bool,
    pub resolution: FlowResolution,
    pub limitations: Vec<String>,
    pub conforming_path: Option<Vec<RelatedLocation>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowProgramEdge {
    pub id: String,
    pub kind: FlowEdgeKind,
    pub resolution: FlowResolution,
    pub language: String,
    pub from: String,
    pub to: String,
    pub path: String,
    pub line: usize,
    pub symbol: String,
    pub module: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowUnresolvedRecord {
    pub resolution: FlowResolution,
    pub reason: String,
    pub count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowProgram {
    pub modules: Vec<String>,
    pub functions: Vec<String>,
    pub nodes: Vec<FlowLocation>,
    pub edges: Vec<FlowProgramEdge>,
    pub sources: Vec<String>,
    pub sinks: Vec<String>,
    pub mutations: Vec<String>,
    pub transformations: Vec<String>,
    pub unresolved: Vec<FlowUnresolvedRecord>,
}
