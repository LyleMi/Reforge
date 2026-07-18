use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnityProjectStatus {
    #[default]
    NotDetected,
    Disabled,
    Observed,
    PartiallyObserved,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnityProjectReport {
    pub status: UnityProjectStatus,
    pub editor_version: Option<String>,
    pub serialization_mode: Option<String>,
    pub analysis_roots: Vec<String>,
    pub stats: UnityProjectStats,
    pub assemblies: Vec<UnityAssemblyNode>,
    pub assembly_edges: Vec<UnityAssemblyEdge>,
    pub problem_references: Vec<UnityReferenceProblem>,
    pub raw_metrics: Vec<UnityRawMetric>,
    pub metric_manifest: Vec<UnityMetricManifestEntry>,
    pub coverage: Vec<UnityCoverageEntry>,
    pub degraded_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnityRawMetric {
    pub name: String,
    pub path: String,
    pub value: usize,
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnityMetricManifestEntry {
    pub name: String,
    pub entity: String,
    pub unit: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnityProjectStats {
    pub assemblies: usize,
    pub scenes: usize,
    pub prefabs: usize,
    pub assets: usize,
    pub meta_files: usize,
    pub guids: usize,
    pub tests: usize,
    pub yaml_assets: usize,
    pub binary_assets: usize,
    pub asset_references: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnityAssemblyNode {
    pub name: String,
    pub path: String,
    pub editor_only: bool,
    pub test_assembly: bool,
    pub predefined: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnityAssemblyEdge {
    pub from: String,
    pub to: String,
    pub reference: String,
    pub resolved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnityReferenceProblem {
    pub source_path: String,
    pub line: usize,
    pub guid: String,
    pub file_id: Option<String>,
    pub category: String,
    pub resolved_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnityCoverageEntry {
    pub area: String,
    pub status: UnityProjectStatus,
    pub reason: Option<String>,
}
