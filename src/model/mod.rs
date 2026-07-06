use std::collections::BTreeMap;

use serde::{Serialize, Serializer, ser::SerializeStruct};

use crate::cli::{ChurnMode, HotspotModel};

pub const SCAN_REPORT_SCHEMA_VERSION: u8 = 6;
pub(crate) const SERIALIZED_SIMILAR_LOCATION_LIMIT: usize = 50;
pub(crate) const METRIC_NESTING_DEPTH: &str = "nesting_depth";
pub(crate) const METRIC_PUBLIC_ITEMS: &str = "public_items";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
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
    LargeType,
    LargePublicSurface,
    ImportHeavyFile,
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
    MissingDocumentationSet,
    MissingUserGuide,
    MissingReportSchemaDocs,
    MissingMetricsModelDocs,
    MissingArchitectureDocs,
    StaleCliDocumentation,
    StaleSchemaDocumentation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricDimension {
    Size,
    Complexity,
    Coupling,
    Duplication,
    Drift,
    TestRisk,
    Documentation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RelatedLocation {
    pub path: String,
    pub line: usize,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FindingMetric {
    pub name: String,
    pub value: usize,
    pub threshold: Option<usize>,
    pub unit: String,
    pub excess_ratio: Option<f64>,
    pub dimension: MetricDimension,
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
            dimension: MetricDimension::Size,
            normalized: (threshold > 0).then_some(crate::scoring::normalized_threshold_excess(
                value as f64 / threshold as f64,
            )),
            percentile: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub kind: FindingKind,
    pub severity: Severity,
    pub path: String,
    pub line: Option<usize>,
    pub metrics: Vec<FindingMetric>,
    pub priority: u8,
    pub confidence: f64,
    pub priority_factors: PriorityFactors,
    pub rank_explanation: String,
    pub message: String,
    pub related_locations: Vec<RelatedLocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
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
        let mut state = serializer.serialize_struct("Finding", 11)?;
        state.serialize_field("kind", &self.kind)?;
        state.serialize_field("severity", &self.severity)?;
        state.serialize_field("path", &self.path)?;
        state.serialize_field("line", &self.line)?;
        state.serialize_field("metrics", &self.metrics)?;
        state.serialize_field("priority", &self.priority)?;
        state.serialize_field("confidence", &self.confidence)?;
        state.serialize_field("priority_factors", &self.priority_factors)?;
        state.serialize_field("rank_explanation", &self.rank_explanation)?;
        state.serialize_field("message", &self.message)?;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanSummary {
    pub scanned_files: usize,
    pub finding_count: usize,
    pub hotspot_count: usize,
    pub similar_function_group_count: usize,
    pub duration_ms: u128,
    pub hotspot_model: HotspotModel,
    pub churn: ChurnSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ScanStats {
    pub source_files_scanned: usize,
    pub directories_scanned: usize,
    pub function_candidates: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChurnSummary {
    pub mode: ChurnMode,
    pub enabled: bool,
    pub status: String,
    pub reason: Option<String>,
    pub window_days: usize,
    pub max_commit_lines: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ChurnFileMetric {
    pub commits_touched: usize,
    pub lines_added: usize,
    pub lines_deleted: usize,
    pub authors_count: usize,
    pub recent_weighted_churn: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileRawMetric {
    pub path: String,
    pub loc: usize,
    pub imports: usize,
    pub public_items: usize,
    pub directory_source_files: usize,
    pub is_test: bool,
    pub churn: ChurnFileMetric,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypeRawMetric {
    pub path: String,
    pub name: String,
    pub line: usize,
    pub loc: usize,
    pub member_count: usize,
    pub is_test: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct RawMetrics {
    pub files: Vec<FileRawMetric>,
    pub functions: Vec<FunctionRawMetric>,
    pub types: Vec<TypeRawMetric>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetricPercentiles {
    pub p50: usize,
    pub p75: usize,
    pub p90: usize,
    pub p95: usize,
    pub max: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetricsSummary {
    pub files: BTreeMap<String, MetricPercentiles>,
    pub functions: BTreeMap<String, MetricPercentiles>,
    pub types: BTreeMap<String, MetricPercentiles>,
    pub churn: BTreeMap<String, MetricPercentiles>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HotspotLevel {
    File,
    Function,
    Type,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScanReport {
    pub schema_version: u8,
    pub summary: ScanSummary,
    pub stats: ScanStats,
    pub metrics_summary: MetricsSummary,
    pub raw_metrics: RawMetrics,
    pub hotspots: Vec<Hotspot>,
    pub findings: Vec<Finding>,
}
