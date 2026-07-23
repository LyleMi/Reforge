use super::*;

macro_rules! define_rules {
    ($($variant:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum Rule {
            $($variant),+
        }

        impl Rule {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
        }
    };
}

define_rules!(
    LargeFile,
    LargeDirectory,
    DebtMarker,
    SimilarFunctions,
    LongFunction,
    ComplexFunction,
    DeepNesting,
    ManyParameters,
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
    AdapterFlowBypass,
    ExcessiveRelay,
    FlowFanOut,
    StaleCompatibilityPath,
    MissingUserGuide,
    MissingReportSchemaDocs,
    MissingMetricsModelDocs,
    MissingArchitectureDocs,
    StaleCliDocumentation,
    StaleSchemaDocumentation,
    DependencyCycle,
    DependencyHub,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    Repository,
    Directory,
    File,
    Symbol,
    Group,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationSource {
    Repositories,
    Directories,
    Files,
    Functions,
    Types,
    FunctionPairs,
    DependencyNodes,
    DataflowSources,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedLocation {
    pub path: String,
    pub line: usize,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectedMeasurement {
    pub name: MetricId,
    pub value: usize,
    pub threshold: Option<usize>,
    pub unit: String,
}

impl DetectedMeasurement {
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
        }
    }

    pub fn measurement(name: MetricId, value: usize, unit: impl Into<String>) -> Self {
        Self {
            name,
            value,
            threshold: None,
            unit: unit.into(),
        }
    }
}
