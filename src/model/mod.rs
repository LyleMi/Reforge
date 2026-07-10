use std::collections::BTreeMap;

use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};

use crate::cli::{ChurnMode, HotspotModel};

pub const SCAN_REPORT_SCHEMA_VERSION: u8 = 15;
pub(crate) const SERIALIZED_SIMILAR_LOCATION_LIMIT: usize = 50;
pub(crate) const METRIC_NESTING_DEPTH: &str = "nesting_depth";
pub(crate) const METRIC_PUBLIC_ITEMS: &str = "public_items";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    LargeFile,
    LargeDirectory,
    DebtMarker,
    SimilarFunctions,
    LongFunction,
    ComplexFunction,
    DeepNesting,
    ManyParameters,
    ReadabilityRisk,
    LargeType,
    LargePublicSurface,
    ImportHeavyFile,
    FunctionProliferation,
    UnusedFunction,
    RepeatedLiteral,
    RepeatedErrorPattern,
    TestDuplication,
    HappyPathOnlyTests,
    FileNamingDrift,
    DirectoryDrift,
    DataClump,
    ParallelImplementation,
    ShadowedAbstraction,
    DuplicateTypeShape,
    ConfigKeyDrift,
    FixtureFactoryDrift,
    GenericBucketDrift,
    AdapterBoundaryBypass,
    StaleCompatibilityPath,
    MissingDocumentationSet,
    MissingUserGuide,
    MissingReportSchemaDocs,
    MissingMetricsModelDocs,
    MissingArchitectureDocs,
    StaleCliDocumentation,
    StaleSchemaDocumentation,
    DependencyCycle,
    DependencyHub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityConstruct {
    Modularity,
    Reusability,
    Analysability,
    Modifiability,
    Testability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalMechanism {
    CognitiveLoad,
    DependencyPropagation,
    ResponsibilityDispersion,
    DuplicationDivergence,
    ChangePressure,
    VerificationDifficulty,
    KnowledgeDrift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefactorAction {
    SimplifyFunction,
    ReduceDependencyCoupling,
    DecomposeResponsibility,
    ConsolidateDuplication,
    ConsolidateTestSupport,
    StrengthenTestCoverage,
    RemoveDeadCode,
    ResolveDeclaredDebt,
    StandardizeNaming,
    RetireCompatibility,
    RestoreDocumentation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityScope {
    Repository,
    Directory,
    File,
    Function,
    Type,
    FindingGroup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricScale {
    Boolean,
    Count,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricDirection {
    HigherIsMorePressure,
    ContextOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawMetricManifestEntry {
    pub name: String,
    pub entity_scope: EntityScope,
    pub unit: String,
    pub scale: MetricScale,
    pub direction: MetricDirection,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectorRelationKind {
    FacetOf,
    AlternativeEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorRelation {
    pub kind: FindingKind,
    pub relation: DetectorRelationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionApproach {
    Threshold,
    ParsedAnalysis,
    GraphAnalysis,
    Heuristic,
    RepositoryAudit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrecisionRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedLocation {
    pub path: String,
    pub line: usize,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingMetric {
    pub name: String,
    pub value: usize,
    pub threshold: Option<usize>,
    pub unit: String,
    pub excess_ratio: Option<f64>,
    pub normalized: Option<f64>,
    pub percentile: Option<f64>,
}

impl FindingMetric {
    pub fn threshold(
        name: impl Into<String>,
        value: usize,
        threshold: usize,
        unit: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            value,
            threshold: Some(threshold),
            unit: unit.into(),
            excess_ratio: (threshold > 0).then_some(value as f64 / threshold as f64),
            normalized: (threshold > 0).then_some(crate::scoring::normalized_threshold_excess(
                value as f64 / threshold as f64,
            )),
            percentile: None,
        }
    }

    pub fn measurement(name: impl Into<String>, value: usize, unit: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value,
            threshold: None,
            unit: unit.into(),
            excess_ratio: None,
            normalized: None,
            percentile: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Finding {
    pub id: String,
    pub kind: FindingKind,
    pub severity: Severity,
    pub path: String,
    pub line: Option<usize>,
    pub metrics: Vec<FindingMetric>,
    pub construct: QualityConstruct,
    pub mechanism: SignalMechanism,
    pub issue_cluster_id: Option<String>,
    pub priority: u8,
    pub confidence: f64,
    pub priority_factors: PriorityFactors,
    pub rank_explanation: String,
    pub message: String,
    pub related_locations: Vec<RelatedLocation>,
}

impl Finding {
    pub fn refresh_id(&mut self) {
        self.id = stable_finding_id(self);
    }

    pub fn recommendation(&self) -> &'static str {
        recommendation_for_kind(self.kind)
    }
}

pub fn recommendation_for_kind(kind: FindingKind) -> &'static str {
    KIND_RECOMMENDATIONS
        .iter()
        .find_map(|(candidate, recommendation)| (*candidate == kind).then_some(*recommendation))
        .unwrap_or(
            "Review the finding and choose the smallest refactor that reduces the reported risk.",
        )
}

const KIND_RECOMMENDATIONS: &[(FindingKind, &str)] = &[
    (
        FindingKind::LargeFile,
        "Split the file around cohesive responsibilities and move shared helpers behind clear module boundaries.",
    ),
    (
        FindingKind::LargeDirectory,
        "Group related files into focused subdirectories with explicit ownership boundaries.",
    ),
    (
        FindingKind::DebtMarker,
        "Resolve the marked debt or replace the marker with an owner, rationale, and tracking reference.",
    ),
    (
        FindingKind::SimilarFunctions,
        "Extract the shared behavior into a common helper or deliberately separate the variants if they should evolve independently.",
    ),
    (
        FindingKind::LongFunction,
        "Extract named steps until the function has one clear orchestration path.",
    ),
    (
        FindingKind::ComplexFunction,
        "Simplify branching with guard clauses, smaller decision helpers, or a clearer state model.",
    ),
    (
        FindingKind::DeepNesting,
        "Flatten control flow with early returns and extracted helpers for nested branches.",
    ),
    (
        FindingKind::ManyParameters,
        "Introduce a small parameter object or split the function by responsibility.",
    ),
    (
        FindingKind::ReadabilityRisk,
        "Extract named steps or narrower collaborators around the combined size, branching, nesting, or parameter pressure.",
    ),
    (
        FindingKind::LargeType,
        "Separate independent responsibilities into smaller types or move behavior to collaborators.",
    ),
    (
        FindingKind::LargePublicSurface,
        "Reduce public API exposure to the stable operations callers actually need.",
    ),
    (
        FindingKind::ImportHeavyFile,
        "Review dependencies and split orchestration, domain logic, and adapters into narrower modules.",
    ),
    (
        FindingKind::FunctionProliferation,
        "Consolidate tiny related functions into cohesive units or move them near their owning abstraction.",
    ),
    (
        FindingKind::UnusedFunction,
        "Delete the unused function or add the missing call path if it is intentionally exposed.",
    ),
    (
        FindingKind::RepeatedLiteral,
        "Replace repeated literals with a named constant or domain concept where the value has shared meaning.",
    ),
    (
        FindingKind::RepeatedErrorPattern,
        "Centralize repeated error handling in a helper, result mapper, or shared policy.",
    ),
    (
        FindingKind::TestDuplication,
        "Extract common test setup into fixtures while keeping each assertion path explicit.",
    ),
    (
        FindingKind::HappyPathOnlyTests,
        "Add focused failure, boundary, and malformed-input cases around the same behavior.",
    ),
    (
        FindingKind::FileNamingDrift,
        "Normalize file naming within the directory or split mixed conventions by layer.",
    ),
    (
        FindingKind::DirectoryDrift,
        "Reorganize mixed concepts into directories that match domain or layer ownership.",
    ),
    (
        FindingKind::DataClump,
        "Introduce a named value object for fields that repeatedly travel together.",
    ),
    (
        FindingKind::ParallelImplementation,
        "Merge parallel implementations behind one abstraction or document why both variants must remain.",
    ),
    (
        FindingKind::ShadowedAbstraction,
        "Route callers through the existing abstraction instead of maintaining a local duplicate.",
    ),
    (
        FindingKind::DuplicateTypeShape,
        "Consolidate duplicate type shapes or introduce a shared DTO/model with explicit conversion points.",
    ),
    (
        FindingKind::ConfigKeyDrift,
        "Centralize related configuration keys and keep aliases documented at the boundary.",
    ),
    (
        FindingKind::FixtureFactoryDrift,
        "Consolidate fixture factories so test data defaults come from one named source.",
    ),
    (
        FindingKind::GenericBucketDrift,
        "Move generic bucket contents into modules named for the concept they own.",
    ),
    (
        FindingKind::AdapterBoundaryBypass,
        "Route boundary access through the adapter instead of reaching across layers directly.",
    ),
    (
        FindingKind::StaleCompatibilityPath,
        "Remove the compatibility path if callers have migrated or add an explicit sunset plan.",
    ),
    (
        FindingKind::MissingDocumentationSet,
        "Add the missing documentation files or update the documentation index to match supported docs.",
    ),
    (
        FindingKind::MissingUserGuide,
        "Document the user-facing workflow, including commands, options, and expected output.",
    ),
    (
        FindingKind::MissingReportSchemaDocs,
        "Update the report schema reference to include current serialized fields and compatibility notes.",
    ),
    (
        FindingKind::MissingMetricsModelDocs,
        "Document how raw metrics, percentiles, hotspots, and priority factors are computed.",
    ),
    (
        FindingKind::MissingArchitectureDocs,
        "Add architecture notes that explain module boundaries and detector/reporting flow.",
    ),
    (
        FindingKind::StaleCliDocumentation,
        "Update CLI documentation so listed flags and defaults match the parser.",
    ),
    (
        FindingKind::StaleSchemaDocumentation,
        "Update schema documentation for the current report fields and finding kinds.",
    ),
    (
        FindingKind::DependencyCycle,
        "Break the cycle by moving shared contracts to a lower-level module or inverting one dependency.",
    ),
    (
        FindingKind::DependencyHub,
        "Review the hub for mixed responsibilities and split fan-in/fan-out behind narrower interfaces.",
    ),
];

pub fn stable_finding_id(finding: &Finding) -> String {
    let mut input = String::new();
    input.push_str("rf1\0");
    input.push_str(&serialized_finding_kind(finding.kind));
    input.push('\0');
    input.push_str(&normalize_identity_path(&finding.path));
    input.push('\0');
    input.push_str(&finding.line.unwrap_or(0).to_string());

    let mut metric_names = finding
        .metrics
        .iter()
        .map(|metric| metric.name.as_str())
        .collect::<Vec<_>>();
    metric_names.sort_unstable();
    metric_names.dedup();
    for name in metric_names {
        input.push('\0');
        input.push_str(name);
    }

    let mut related = finding
        .related_locations
        .iter()
        .map(|location| {
            format!(
                "{}:{}:{}",
                normalize_identity_path(&location.path),
                location.line,
                location.name.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>();
    related.sort_unstable();
    for location in related {
        input.push('\0');
        input.push_str(&location);
    }

    format!("rf1-{:016x}", fnv1a64(input.as_bytes()))
}

pub fn serialized_finding_kind(kind: FindingKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(ToString::to_string))
        .unwrap_or_else(|| format!("{kind:?}"))
}

fn normalize_identity_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriorityFactors {
    pub impact: f64,
    pub intensity: f64,
    pub spread: f64,
    pub change_pressure: f64,
    pub actionability: f64,
    pub confidence: f64,
}

impl Serialize for Finding {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Finding", 16)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("kind", &self.kind)?;
        state.serialize_field("severity", &self.severity)?;
        state.serialize_field("path", &self.path)?;
        state.serialize_field("line", &self.line)?;
        state.serialize_field("metrics", &self.metrics)?;
        state.serialize_field("construct", &self.construct)?;
        state.serialize_field("mechanism", &self.mechanism)?;
        state.serialize_field("issue_cluster_id", &self.issue_cluster_id)?;
        state.serialize_field("priority", &self.priority)?;
        state.serialize_field("confidence", &self.confidence)?;
        state.serialize_field("priority_factors", &self.priority_factors)?;
        state.serialize_field("rank_explanation", &self.rank_explanation)?;
        state.serialize_field("message", &self.message)?;
        state.serialize_field("recommendation", &self.recommendation())?;
        state.serialize_field("related_locations", serialized_related_locations(self))?;
        state.end()
    }
}

fn serialized_related_locations(finding: &Finding) -> &[RelatedLocation] {
    if finding.kind == FindingKind::SimilarFunctions
        && finding.related_locations.len() > SERIALIZED_SIMILAR_LOCATION_LIMIT
    {
        &finding.related_locations[..SERIALIZED_SIMILAR_LOCATION_LIMIT]
    } else {
        &finding.related_locations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanSummary {
    pub scanned_files: usize,
    pub finding_count: usize,
    pub issue_count: usize,
    pub hotspot_count: usize,
    pub similar_function_group_count: usize,
    pub duration_ms: u128,
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
    pub directory_source_files: usize,
    pub is_test: bool,
    pub churn: ChurnFileMetric,
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
    pub files: BTreeMap<String, MetricPercentiles>,
    pub functions: BTreeMap<String, MetricPercentiles>,
    pub types: BTreeMap<String, MetricPercentiles>,
    pub churn: BTreeMap<String, MetricPercentiles>,
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

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SuppressionSummary {
    pub suppressed_count: usize,
    pub suppressed_by_kind: BTreeMap<FindingKind, usize>,
    pub suppressed_by_severity: BTreeMap<Severity, usize>,
    pub highest_suppressed_priority: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueCluster {
    pub id: String,
    pub construct: QualityConstruct,
    pub mechanism: SignalMechanism,
    pub action: RefactorAction,
    pub path: String,
    pub line: Option<usize>,
    pub primary_finding_id: String,
    pub finding_ids: Vec<String>,
    pub kinds: Vec<FindingKind>,
    pub priority: u8,
    pub severity: Severity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorManifestEntry {
    pub kind: FindingKind,
    pub construct: QualityConstruct,
    pub mechanism: SignalMechanism,
    pub action: RefactorAction,
    pub entity_scope: EntityScope,
    pub approach: DetectionApproach,
    pub supported_languages: Vec<String>,
    pub precision_risk: PrecisionRisk,
    pub parent_kind: Option<FindingKind>,
    pub relations: Vec<DetectorRelation>,
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
    pub hotspots: Vec<Hotspot>,
    pub suppression_summary: SuppressionSummary,
    pub issue_clusters: Vec<IssueCluster>,
    pub detector_manifest: Vec<DetectorManifestEntry>,
    pub findings: Vec<Finding>,
}
