use super::evidence::normalize_identity_path;
use super::*;

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
    UnityAssemblyCycle,
    UnityAssemblyHub,
    UnityUnresolvedAssemblyReference,
    UnityRuntimeEditorDependency,
    UnityDuplicateGuid,
    UnityMissingMeta,
    UnityOrphanMeta,
    UnityBrokenAssetReference,
    UnityMissingScript,
    UnityNonTextSerialization,
    UnitySceneBuildDrift,
    UnityLargeScene,
    UnityLargePrefab,
    UnitySerializedFieldBloat,
    UnityLifecycleOverload,
    UnityExpensiveFrameCall,
    UnityEditorApiInRuntime,
    UnityUnbalancedEventSubscription,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    #[default]
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum EvidenceSubject {
    Repository,
    Directory { path: String },
    File { path: String },
    Function { path: String, line: usize },
    Type { path: String, line: usize },
    Group { locations: Vec<String> },
}

impl EvidenceSubject {
    pub fn identity(&self) -> String {
        match self {
            Self::Repository => "repository".into(),
            Self::Directory { path } => format!("directory:{}", normalize_identity_path(path)),
            Self::File { path } => format!("file:{}", normalize_identity_path(path)),
            Self::Function { path, line } => {
                format!("function:{}:{line}", normalize_identity_path(path))
            }
            Self::Type { path, line } => format!("type:{}:{line}", normalize_identity_path(path)),
            Self::Group { locations } => {
                let mut locations = locations
                    .iter()
                    .map(|value| normalize_identity_path(value))
                    .collect::<Vec<_>>();
                locations.sort();
                locations.dedup();
                format!("group:{}", locations.join("|"))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRole {
    Atomic,
    Alternative,
    CompositeSummary,
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
    pub name: MetricId,
    pub entity_scope: EntityScope,
    pub unit: String,
    pub scale: MetricScale,
    pub direction: MetricDirection,
    pub description: String,
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
    pub name: MetricId,
    pub value: usize,
    pub threshold: Option<usize>,
    pub unit: String,
    pub excess_ratio: Option<f64>,
    pub normalized: Option<f64>,
    pub percentile: Option<f64>,
}

impl FindingMetric {
    pub fn threshold(
        name: MetricId,
        value: usize,
        threshold: usize,
        unit: impl Into<String>,
    ) -> Self {
        Self {
            name,
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

    pub fn measurement(name: MetricId, value: usize, unit: impl Into<String>) -> Self {
        Self {
            name,
            value,
            threshold: None,
            unit: unit.into(),
            excess_ratio: None,
            normalized: None,
            percentile: None,
        }
    }
}
