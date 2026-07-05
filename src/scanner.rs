use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};
use walkdir::{DirEntry, WalkDir};

use crate::agent_drift::{AgentDriftOptions, scan_agent_drift};
use crate::cli::{ChurnMode, HotspotModel, ScanArgs};
use crate::similar_functions::{
    ParsedSourceFile, SimilarFunctionOptions, SimilarFunctionProgress, SourceFile,
    parse_source_file, scan_parsed_similar_functions_report_with_progress,
};
use crate::structural::{
    StructureOptions, collect_raw_structure_metrics, is_supported_structure_source,
};

const DEFAULT_EXCLUDED_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    "out",
    "target",
    "coverage",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".vite",
];
const PERCENT_SCALE: usize = 100;
pub const SCAN_REPORT_SCHEMA_VERSION: u8 = 4;
const SERIALIZED_SIMILAR_LOCATION_LIMIT: usize = 50;
const PERCENTILE_MIN_SAMPLE: usize = 5;
const DEFAULT_MAX_FILE_LINES: usize = 800;
const DEFAULT_MAX_DIR_FILES: usize = 40;
const DEFAULT_MIN_SIMILAR_FUNCTIONS: usize = 3;
const DEFAULT_MIN_FUNCTION_TOKENS: usize = 80;
const DEFAULT_FUNCTION_SIMILARITY: f64 = 0.85;
const DEFAULT_MAX_FUNCTION_LINES: usize = 80;
const DEFAULT_MAX_FUNCTION_COMPLEXITY: usize = 15;
const DEFAULT_MAX_NESTING_DEPTH: usize = 4;
const DEFAULT_MAX_FUNCTION_PARAMETERS: usize = 5;
const DEFAULT_MAX_TYPE_LINES: usize = 250;
const DEFAULT_MAX_TYPE_MEMBERS: usize = 30;
const DEFAULT_MAX_IMPORTS: usize = 35;
const DEFAULT_MAX_PUBLIC_ITEMS: usize = 30;
const DEFAULT_MIN_REPEATED_LITERAL_OCCURRENCES: usize = 4;
const DEFAULT_MIN_DATA_CLUMP_OCCURRENCES: usize = 3;
const DEFAULT_CHURN_WINDOW_DAYS: usize = 180;
const DEFAULT_CHURN_MAX_COMMIT_LINES: usize = 2_000;

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
            normalized: (threshold > 0)
                .then_some(normalized_threshold_excess(value as f64 / threshold as f64)),
        }
    }

    fn with_kind_context(mut self, kind: FindingKind) -> Self {
        self.dimension = metric_dimension(kind, &self.name);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub kind: FindingKind,
    pub severity: Severity,
    pub path: String,
    pub line: Option<usize>,
    pub metrics: Vec<FindingMetric>,
    pub score: u8,
    pub confidence: f64,
    pub score_breakdown: ScoreBreakdown,
    pub rank_reason: String,
    pub message: String,
    pub related_locations: Vec<RelatedLocation>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScoreBreakdown {
    pub impact: f64,
    pub intensity: f64,
    pub spread: f64,
    pub confidence: f64,
    pub actionability: f64,
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
        state.serialize_field("score", &self.score)?;
        state.serialize_field("confidence", &self.confidence)?;
        state.serialize_field("score_breakdown", &self.score_breakdown)?;
        state.serialize_field("rank_reason", &self.rank_reason)?;
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

pub fn finding(
    kind: FindingKind,
    path: impl Into<String>,
    line: Option<usize>,
    message: impl Into<String>,
    metrics: Vec<FindingMetric>,
    related_locations: Vec<RelatedLocation>,
) -> Finding {
    scored_finding(
        kind,
        path,
        line,
        message,
        metrics,
        default_confidence(kind),
        related_locations,
    )
}

pub fn scored_finding(
    kind: FindingKind,
    path: impl Into<String>,
    line: Option<usize>,
    message: impl Into<String>,
    metrics: Vec<FindingMetric>,
    confidence: f64,
    related_locations: Vec<RelatedLocation>,
) -> Finding {
    let path = path.into();
    let metrics = metrics
        .into_iter()
        .map(|metric| metric.with_kind_context(kind))
        .collect::<Vec<_>>();
    let score_breakdown = score_breakdown(kind, &metrics, confidence, &related_locations);
    let score = priority_score_from_breakdown(&score_breakdown);
    let rank_reason = rank_reason(kind, &score_breakdown, &related_locations);
    Finding {
        kind,
        severity: severity_for_score(score),
        path,
        line,
        metrics,
        score,
        confidence,
        score_breakdown,
        rank_reason,
        message: message.into(),
        related_locations,
    }
}

pub fn severity_for_score(score: u8) -> Severity {
    match score {
        0..=34 => Severity::Info,
        35..=69 => Severity::Warning,
        70..=u8::MAX => Severity::Critical,
    }
}

#[allow(dead_code)]
pub fn priority_score(
    kind: FindingKind,
    metrics: &[FindingMetric],
    confidence: f64,
    related_locations: &[RelatedLocation],
) -> u8 {
    let breakdown = score_breakdown(kind, metrics, confidence, related_locations);
    priority_score_from_breakdown(&breakdown)
}

fn score_breakdown(
    kind: FindingKind,
    metrics: &[FindingMetric],
    confidence: f64,
    related_locations: &[RelatedLocation],
) -> ScoreBreakdown {
    ScoreBreakdown {
        impact: impact_score(kind),
        intensity: intensity_score(metrics),
        spread: spread_score(related_locations),
        confidence: confidence.clamp(0.0, 1.0),
        actionability: actionability_score(kind),
    }
}

fn priority_score_from_breakdown(breakdown: &ScoreBreakdown) -> u8 {
    let weighted = (breakdown.impact * 0.50)
        + (breakdown.intensity * 0.30)
        + (breakdown.spread * 0.10)
        + (breakdown.actionability * 0.10);
    (weighted * breakdown.confidence).round().clamp(0.0, 100.0) as u8
}

fn intensity_score(metrics: &[FindingMetric]) -> f64 {
    let strongest = metrics
        .iter()
        .filter_map(|metric| metric.normalized)
        .max_by(f64::total_cmp)
        .unwrap_or(0.20);
    (strongest * 100.0).clamp(0.0, 100.0)
}

fn spread_score(related_locations: &[RelatedLocation]) -> f64 {
    let unique_related_files = related_locations
        .iter()
        .map(|location| location.path.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    if unique_related_files <= 1 {
        return 0.0;
    }

    let file_spread = (unique_related_files as f64).ln_1p() / 8.0_f64.ln_1p();
    (35.0 + file_spread * 50.0).min(85.0)
}

pub fn default_confidence(kind: FindingKind) -> f64 {
    match kind {
        FindingKind::SimilarFunctions => 0.85,
        FindingKind::RepeatedErrorPattern
        | FindingKind::TestDuplication
        | FindingKind::DataClump
        | FindingKind::ConfigKeyDrift
        | FindingKind::FixtureFactoryDrift => 0.85,
        FindingKind::RepeatedLiteral => 0.75,
        FindingKind::DuplicateTypeShape | FindingKind::AdapterBoundaryBypass => 0.80,
        FindingKind::GenericBucketDrift => 0.70,
        FindingKind::HappyPathOnlyTests => 0.60,
        FindingKind::FileNamingDrift
        | FindingKind::DirectoryDrift
        | FindingKind::ParallelImplementation
        | FindingKind::ShadowedAbstraction => 0.65,
        FindingKind::DebtMarker
        | FindingKind::LargeFile
        | FindingKind::LargeDirectory
        | FindingKind::LongFunction
        | FindingKind::ComplexFunction
        | FindingKind::DeepNesting
        | FindingKind::ManyParameters
        | FindingKind::LargeType
        | FindingKind::LargePublicSurface
        | FindingKind::ImportHeavyFile => 1.0,
    }
}

fn impact_score(kind: FindingKind) -> f64 {
    match kind {
        FindingKind::DebtMarker => 25.0,
        FindingKind::RepeatedLiteral | FindingKind::FileNamingDrift => 40.0,
        FindingKind::HappyPathOnlyTests | FindingKind::ShadowedAbstraction => 45.0,
        FindingKind::ConfigKeyDrift | FindingKind::FixtureFactoryDrift => 50.0,
        FindingKind::LargeFile | FindingKind::LargeDirectory => 65.0,
        FindingKind::LongFunction
        | FindingKind::DeepNesting
        | FindingKind::ManyParameters
        | FindingKind::LargeType => 70.0,
        FindingKind::LargePublicSurface | FindingKind::ImportHeavyFile => 60.0,
        FindingKind::RepeatedErrorPattern
        | FindingKind::TestDuplication
        | FindingKind::DirectoryDrift
        | FindingKind::DataClump
        | FindingKind::DuplicateTypeShape
        | FindingKind::GenericBucketDrift => 65.0,
        FindingKind::ComplexFunction => 90.0,
        FindingKind::SimilarFunctions
        | FindingKind::ParallelImplementation
        | FindingKind::AdapterBoundaryBypass => 80.0,
    }
}

fn actionability_score(kind: FindingKind) -> f64 {
    match kind {
        FindingKind::RepeatedLiteral | FindingKind::HappyPathOnlyTests => 45.0,
        FindingKind::DebtMarker | FindingKind::FileNamingDrift | FindingKind::DirectoryDrift => {
            60.0
        }
        FindingKind::ShadowedAbstraction
        | FindingKind::ConfigKeyDrift
        | FindingKind::FixtureFactoryDrift
        | FindingKind::GenericBucketDrift => 65.0,
        FindingKind::RepeatedErrorPattern | FindingKind::TestDuplication => 70.0,
        FindingKind::LargeDirectory
        | FindingKind::ImportHeavyFile
        | FindingKind::LargePublicSurface
        | FindingKind::DataClump => 75.0,
        FindingKind::LargeFile
        | FindingKind::LongFunction
        | FindingKind::ComplexFunction
        | FindingKind::DeepNesting
        | FindingKind::ManyParameters
        | FindingKind::LargeType
        | FindingKind::SimilarFunctions
        | FindingKind::ParallelImplementation
        | FindingKind::DuplicateTypeShape
        | FindingKind::AdapterBoundaryBypass => 85.0,
    }
}

fn metric_dimension(kind: FindingKind, metric_name: &str) -> MetricDimension {
    match kind {
        FindingKind::LargeFile
        | FindingKind::LargeDirectory
        | FindingKind::LongFunction
        | FindingKind::LargeType => MetricDimension::Size,
        FindingKind::ComplexFunction | FindingKind::DeepNesting | FindingKind::ManyParameters => {
            MetricDimension::Complexity
        }
        FindingKind::LargePublicSurface
        | FindingKind::ImportHeavyFile
        | FindingKind::AdapterBoundaryBypass => MetricDimension::Coupling,
        FindingKind::SimilarFunctions
        | FindingKind::RepeatedLiteral
        | FindingKind::RepeatedErrorPattern
        | FindingKind::DataClump
        | FindingKind::DuplicateTypeShape => MetricDimension::Duplication,
        FindingKind::TestDuplication | FindingKind::HappyPathOnlyTests => MetricDimension::TestRisk,
        FindingKind::FileNamingDrift
        | FindingKind::DirectoryDrift
        | FindingKind::ParallelImplementation
        | FindingKind::ShadowedAbstraction
        | FindingKind::ConfigKeyDrift
        | FindingKind::FixtureFactoryDrift
        | FindingKind::GenericBucketDrift => MetricDimension::Drift,
        FindingKind::DebtMarker => match metric_name {
            "imports" | "public_items" => MetricDimension::Coupling,
            "function_complexity" | "nesting_depth" | "function_parameters" => {
                MetricDimension::Complexity
            }
            _ => MetricDimension::Size,
        },
    }
}

fn normalized_threshold_excess(ratio: f64) -> f64 {
    let ratio = ratio.max(0.0);
    if ratio <= 1.0 {
        return 0.35;
    }

    let log_component = ratio.ln_1p() / 5.0_f64.ln_1p();
    (0.35 + (log_component * 0.65)).clamp(0.35, 1.0)
}

fn rank_reason(
    kind: FindingKind,
    breakdown: &ScoreBreakdown,
    related_locations: &[RelatedLocation],
) -> String {
    let mut reasons = Vec::new();
    reasons.push(match primary_dimension(kind) {
        MetricDimension::Size => "high size",
        MetricDimension::Complexity => "high complexity",
        MetricDimension::Coupling => "coupling pressure",
        MetricDimension::Duplication => "duplication signal",
        MetricDimension::Drift => "drift signal",
        MetricDimension::TestRisk => "test-risk signal",
    });

    if unique_related_file_count(related_locations) > 1 {
        reasons.push("cross-file spread");
    }

    if breakdown.confidence >= 0.85 {
        reasons.push("high confidence");
    } else if breakdown.confidence >= 0.65 {
        reasons.push("medium confidence");
    } else {
        reasons.push("low confidence");
    }

    if breakdown.actionability < 60.0 {
        reasons.push("lower actionability");
    }

    reasons.join(", ")
}

fn primary_dimension(kind: FindingKind) -> MetricDimension {
    metric_dimension(kind, "")
}

fn unique_related_file_count(related_locations: &[RelatedLocation]) -> usize {
    related_locations
        .iter()
        .map(|location| location.path.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

fn finalize_scoring(findings: &mut [Finding], raw_metrics: &RawMetrics) {
    let percentile_values = percentile_metric_values(raw_metrics);

    for finding in findings {
        for metric in &mut finding.metrics {
            metric.dimension = metric_dimension(finding.kind, &metric.name);
            let threshold_normalized = metric
                .excess_ratio
                .map(normalized_threshold_excess)
                .unwrap_or(0.20);
            metric.normalized = Some(normalized_metric_value(
                metric,
                threshold_normalized,
                &percentile_values,
            ));
        }

        finding.score_breakdown = score_breakdown(
            finding.kind,
            &finding.metrics,
            finding.confidence,
            &finding.related_locations,
        );
        finding.score = priority_score_from_breakdown(&finding.score_breakdown);
        finding.severity = severity_for_score(finding.score);
        finding.rank_reason = rank_reason(
            finding.kind,
            &finding.score_breakdown,
            &finding.related_locations,
        );
    }
}

fn percentile_metric_values(raw_metrics: &RawMetrics) -> BTreeMap<String, Vec<usize>> {
    let mut values = BTreeMap::<String, Vec<usize>>::new();

    for file in &raw_metrics.files {
        values
            .entry("file_lines".to_string())
            .or_default()
            .push(file.loc);
        values
            .entry("imports".to_string())
            .or_default()
            .push(file.imports);
        values
            .entry("public_items".to_string())
            .or_default()
            .push(file.public_items);
        values
            .entry("directory_files".to_string())
            .or_default()
            .push(file.directory_source_files);
    }

    for function in &raw_metrics.functions {
        values
            .entry("function_lines".to_string())
            .or_default()
            .push(function.loc);
        values
            .entry("function_complexity".to_string())
            .or_default()
            .push(function.complexity);
        values
            .entry("nesting_depth".to_string())
            .or_default()
            .push(function.nesting_depth);
        values
            .entry("function_parameters".to_string())
            .or_default()
            .push(function.parameter_count);
    }

    for type_metric in &raw_metrics.types {
        values
            .entry("type_lines".to_string())
            .or_default()
            .push(type_metric.loc);
        values
            .entry("type_members".to_string())
            .or_default()
            .push(type_metric.member_count);
    }

    for values in values.values_mut() {
        values.sort_unstable();
    }

    values
}

fn normalized_metric_value(
    metric: &FindingMetric,
    threshold_normalized: f64,
    percentile_values: &BTreeMap<String, Vec<usize>>,
) -> f64 {
    if !matches!(
        metric.dimension,
        MetricDimension::Size | MetricDimension::Complexity
    ) {
        return threshold_normalized;
    }

    let Some(values) = percentile_values.get(&metric.name) else {
        return threshold_normalized;
    };

    if values.len() < PERCENTILE_MIN_SAMPLE {
        return threshold_normalized;
    }

    let rank = values.partition_point(|value| *value <= metric.value);
    let percentile = rank as f64 / values.len() as f64;
    ((threshold_normalized * 0.65) + (percentile * 0.35)).clamp(0.0, 1.0)
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
    pub score: u8,
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

#[derive(Debug, Default)]
struct SourceScan {
    findings: Vec<Finding>,
    parsed_sources: Vec<ParsedSourceFile>,
    structure_sources: Vec<SourceFile>,
    raw_metrics: RawMetrics,
    stats: ScanStats,
}

#[derive(Debug, Default)]
struct SourceScanPlan {
    source_files: Vec<PathBuf>,
    directory_source_files: BTreeMap<PathBuf, usize>,
    directories_scanned: usize,
}

#[derive(Debug, Clone, Copy)]
struct FileScanOptions {
    max_file_lines: usize,
    directory_source_files: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProgressEvent<'a> {
    completed: usize,
    total: usize,
    detail: &'a str,
}

impl ProgressEvent<'_> {
    fn percent(self) -> Option<usize> {
        if self.total == 0 {
            return None;
        }

        Some(self.completed.saturating_mul(PERCENT_SCALE) / self.total)
    }

    fn detail_suffix(self) -> String {
        if self.detail.is_empty() {
            String::new()
        } else {
            format!(" {}", self.detail)
        }
    }

    fn is_complete(self) -> bool {
        self.completed == self.total
    }
}

pub(crate) trait ProgressSink {
    fn report(&mut self, message: &str);

    fn report_scan_progress(&mut self, event: ProgressEvent<'_>) {
        let Some(percent) = event.percent() else {
            return;
        };
        self.report(&format!(
            "[{percent:>3}%] Scanning source files ({}/{}) {}",
            event.completed, event.total, event.detail
        ));
    }

    fn report_analysis_progress(&mut self, event: ProgressEvent<'_>, phase: &str) {
        let Some(percent) = event.percent() else {
            return;
        };
        let detail = event.detail_suffix();
        self.report(&format!(
            "[{percent:>3}%] Analyzing similar functions: {phase} ({}/{}){detail}",
            event.completed, event.total
        ));
    }

    fn wants_detailed_progress(&self) -> bool {
        false
    }

    fn finish(&mut self) {}
}

pub struct NoopProgress;

impl ProgressSink for NoopProgress {
    fn report(&mut self, _message: &str) {}
}

pub struct StderrProgress {
    dynamic: bool,
    last_dynamic_len: usize,
    last_bucket: Option<usize>,
}

impl StderrProgress {
    pub fn new(dynamic: bool) -> Self {
        Self {
            dynamic,
            last_dynamic_len: 0,
            last_bucket: None,
        }
    }

    fn finish_dynamic_line(&mut self) {
        if self.last_dynamic_len > 0 {
            let _ = writeln!(std::io::stderr());
            self.last_dynamic_len = 0;
        }
    }

    fn report_percent_progress(&mut self, message: &str, event: ProgressEvent<'_>) {
        if self.dynamic {
            let padding = self.last_dynamic_len.saturating_sub(message.len());
            let mut stderr = std::io::stderr().lock();
            let _ = write!(stderr, "\r{message}{}", " ".repeat(padding));
            let _ = stderr.flush();
            self.last_dynamic_len = message.len();
        } else {
            let Some(percent) = event.percent() else {
                return;
            };
            let bucket = percent / 10;
            if self.last_bucket != Some(bucket) || event.is_complete() {
                self.report(message);
                self.last_bucket = Some(bucket);
            }
        }
    }
}

impl ProgressSink for StderrProgress {
    fn report(&mut self, message: &str) {
        self.finish_dynamic_line();
        let _ = writeln!(std::io::stderr(), "{message}");
    }

    fn report_scan_progress(&mut self, event: ProgressEvent<'_>) {
        let Some(percent) = event.percent() else {
            return;
        };
        let message = format!(
            "[{percent:>3}%] Scanning source files ({}/{}) {}",
            event.completed, event.total, event.detail
        );

        self.report_percent_progress(&message, event);
    }

    fn report_analysis_progress(&mut self, event: ProgressEvent<'_>, phase: &str) {
        let Some(percent) = event.percent() else {
            return;
        };
        let detail = event.detail_suffix();
        let message = format!(
            "[{percent:>3}%] Analyzing similar functions: {phase} ({}/{}){detail}",
            event.completed, event.total
        );

        self.report_percent_progress(&message, event);
    }

    fn wants_detailed_progress(&self) -> bool {
        true
    }

    fn finish(&mut self) {
        self.finish_dynamic_line();
    }
}

#[cfg(test)]
pub struct WriterProgress<W: Write> {
    writer: W,
}

#[cfg(test)]
impl<W: Write> WriterProgress<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

#[cfg(test)]
impl<W: Write> ProgressSink for WriterProgress<W> {
    fn report(&mut self, message: &str) {
        let _ = writeln!(self.writer, "{message}");
    }

    fn wants_detailed_progress(&self) -> bool {
        true
    }
}

#[allow(dead_code)]
pub(crate) fn scan_path(args: &ScanArgs) -> Result<Vec<Finding>> {
    let mut progress = NoopProgress;
    Ok(scan_report(args, &mut progress)?.findings)
}

pub(crate) fn scan_report(args: &ScanArgs, progress: &mut dyn ProgressSink) -> Result<ScanReport> {
    let started_at = Instant::now();
    let root = resolve_scan_root(args)?;
    let effective_args = effective_scan_args(args, &root)?;
    let mut scan = SourceScan::default();
    let source_plan = collect_source_scan_plan(&root, &effective_args)?;

    let total_source_files = progress
        .wants_detailed_progress()
        .then_some(source_plan.source_files.len());

    report_scan_start(progress, &root, total_source_files);
    scan_sources(
        source_plan,
        &effective_args,
        total_source_files,
        progress,
        &mut scan,
    )?;
    let similar_function_group_count = {
        let mut signals = ScanSignalContext {
            args: &effective_args,
            progress,
            scan: &mut scan,
        };
        signals.scan_structural_signals()?;
        signals.scan_agent_drift_signals();
        signals.scan_similarity_signals()?
    };
    merge_structure_raw_metrics(&mut scan.raw_metrics, &scan.parsed_sources);
    let churn_summary = collect_churn_metrics(&root, &effective_args, &mut scan.raw_metrics)?;
    let metrics_summary = summarize_raw_metrics(&scan.raw_metrics);
    finalize_scoring(&mut scan.findings, &scan.raw_metrics);
    let hotspots = rank_hotspots(
        &scan.raw_metrics,
        &metrics_summary,
        effective_args
            .hotspot_model
            .expect("effective args should set hotspot model"),
    );
    apply_hotspot_scores_to_findings(&mut scan.findings, &hotspots);

    let summary = ScanSummary {
        scanned_files: scan.stats.source_files_scanned,
        finding_count: scan.findings.len(),
        hotspot_count: hotspots.len(),
        similar_function_group_count,
        duration_ms: started_at.elapsed().as_millis(),
        hotspot_model: effective_args
            .hotspot_model
            .expect("effective args should set hotspot model"),
        churn: churn_summary,
    };

    progress.report(&format!(
        "Finished scan: {} files, {} findings",
        summary.scanned_files, summary.finding_count
    ));
    progress.finish();

    Ok(ScanReport {
        schema_version: SCAN_REPORT_SCHEMA_VERSION,
        summary,
        stats: scan.stats,
        metrics_summary,
        raw_metrics: scan.raw_metrics,
        hotspots,
        findings: scan.findings,
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct ReforgeConfig {
    max_file_lines: Option<usize>,
    max_dir_files: Option<usize>,
    min_similar_functions: Option<usize>,
    min_function_tokens: Option<usize>,
    function_similarity: Option<f64>,
    max_function_lines: Option<usize>,
    max_function_complexity: Option<usize>,
    max_nesting_depth: Option<usize>,
    max_function_parameters: Option<usize>,
    max_type_lines: Option<usize>,
    max_type_members: Option<usize>,
    max_imports: Option<usize>,
    max_public_items: Option<usize>,
    min_repeated_literal_occurrences: Option<usize>,
    min_data_clump_occurrences: Option<usize>,
    churn: Option<ChurnMode>,
    hotspot_model: Option<HotspotModel>,
    churn_window_days: Option<usize>,
    churn_max_commit_lines: Option<usize>,
    ignore_paths: Vec<String>,
}

fn effective_scan_args(args: &ScanArgs, root: &Path) -> Result<ScanArgs> {
    let mut effective = args.clone();
    let config = load_config(args, root)?;

    if let Some(config) = config {
        apply_config_defaults(&mut effective, &config);
    }

    effective.churn = Some(
        args.churn
            .unwrap_or(effective.churn.unwrap_or(ChurnMode::Auto)),
    );
    effective.hotspot_model = Some(
        args.hotspot_model
            .unwrap_or(effective.hotspot_model.unwrap_or(HotspotModel::Hybrid)),
    );
    effective.churn_window_days = Some(
        args.churn_window_days.unwrap_or(
            effective
                .churn_window_days
                .unwrap_or(DEFAULT_CHURN_WINDOW_DAYS),
        ),
    );
    effective.churn_max_commit_lines = Some(
        args.churn_max_commit_lines.unwrap_or(
            effective
                .churn_max_commit_lines
                .unwrap_or(DEFAULT_CHURN_MAX_COMMIT_LINES),
        ),
    );

    Ok(effective)
}

fn load_config(args: &ScanArgs, root: &Path) -> Result<Option<ReforgeConfig>> {
    let config_path = if let Some(path) = &args.config {
        Some(path.clone())
    } else {
        discover_config_path(root)
    };

    let Some(path) = config_path else {
        return Ok(None);
    };

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;
    let config = toml::from_str(&contents)
        .with_context(|| format!("failed to parse config file {}", path.display()))?;
    Ok(Some(config))
}

fn discover_config_path(root: &Path) -> Option<PathBuf> {
    let mut current = if root.is_file() {
        root.parent()?.to_path_buf()
    } else {
        root.to_path_buf()
    };

    loop {
        let candidate = current.join("reforge.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn apply_config_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    apply_usize_default(
        &mut args.max_file_lines,
        DEFAULT_MAX_FILE_LINES,
        config.max_file_lines,
    );
    apply_usize_default(
        &mut args.max_dir_files,
        DEFAULT_MAX_DIR_FILES,
        config.max_dir_files,
    );
    apply_usize_default(
        &mut args.min_similar_functions,
        DEFAULT_MIN_SIMILAR_FUNCTIONS,
        config.min_similar_functions,
    );
    apply_usize_default(
        &mut args.min_function_tokens,
        DEFAULT_MIN_FUNCTION_TOKENS,
        config.min_function_tokens,
    );
    if (args.function_similarity - DEFAULT_FUNCTION_SIMILARITY).abs() < f64::EPSILON
        && let Some(value) = config.function_similarity
    {
        args.function_similarity = value;
    }
    apply_usize_default(
        &mut args.max_function_lines,
        DEFAULT_MAX_FUNCTION_LINES,
        config.max_function_lines,
    );
    apply_usize_default(
        &mut args.max_function_complexity,
        DEFAULT_MAX_FUNCTION_COMPLEXITY,
        config.max_function_complexity,
    );
    apply_usize_default(
        &mut args.max_nesting_depth,
        DEFAULT_MAX_NESTING_DEPTH,
        config.max_nesting_depth,
    );
    apply_usize_default(
        &mut args.max_function_parameters,
        DEFAULT_MAX_FUNCTION_PARAMETERS,
        config.max_function_parameters,
    );
    apply_usize_default(
        &mut args.max_type_lines,
        DEFAULT_MAX_TYPE_LINES,
        config.max_type_lines,
    );
    apply_usize_default(
        &mut args.max_type_members,
        DEFAULT_MAX_TYPE_MEMBERS,
        config.max_type_members,
    );
    apply_usize_default(
        &mut args.max_imports,
        DEFAULT_MAX_IMPORTS,
        config.max_imports,
    );
    apply_usize_default(
        &mut args.max_public_items,
        DEFAULT_MAX_PUBLIC_ITEMS,
        config.max_public_items,
    );
    apply_usize_default(
        &mut args.min_repeated_literal_occurrences,
        DEFAULT_MIN_REPEATED_LITERAL_OCCURRENCES,
        config.min_repeated_literal_occurrences,
    );
    apply_usize_default(
        &mut args.min_data_clump_occurrences,
        DEFAULT_MIN_DATA_CLUMP_OCCURRENCES,
        config.min_data_clump_occurrences,
    );

    args.churn = args.churn.or(config.churn);
    args.hotspot_model = args.hotspot_model.or(config.hotspot_model);
    args.churn_window_days = args.churn_window_days.or(config.churn_window_days);
    args.churn_max_commit_lines = args
        .churn_max_commit_lines
        .or(config.churn_max_commit_lines);
    if args.ignore_paths.is_empty() {
        args.ignore_paths = config.ignore_paths.clone();
    }
}

fn apply_usize_default(target: &mut usize, default: usize, configured: Option<usize>) {
    if *target == default
        && let Some(value) = configured
    {
        *target = value;
    }
}

fn merge_structure_raw_metrics(raw_metrics: &mut RawMetrics, parsed_sources: &[ParsedSourceFile]) {
    let structure_metrics = collect_raw_structure_metrics(parsed_sources);
    let by_path = structure_metrics
        .into_iter()
        .map(|metric| (metric.path.clone(), metric))
        .collect::<BTreeMap<_, _>>();

    for file_metric in &mut raw_metrics.files {
        if let Some(structure_metric) = by_path.get(&file_metric.path) {
            file_metric.imports = structure_metric.imports;
            file_metric.public_items = structure_metric.public_items;
            file_metric.is_test = structure_metric.is_test;
        }
    }

    raw_metrics.functions = by_path
        .values()
        .flat_map(|metric| metric.functions.clone())
        .map(|function| FunctionRawMetric {
            path: function.path,
            name: function.name,
            line: function.line,
            loc: function.loc,
            complexity: function.complexity,
            nesting_depth: function.nesting_depth,
            parameter_count: function.parameter_count,
            is_test: function.is_test,
        })
        .collect();
    raw_metrics.types = by_path
        .values()
        .flat_map(|metric| metric.types.clone())
        .map(|type_metric| TypeRawMetric {
            path: type_metric.path,
            name: type_metric.name,
            line: type_metric.line,
            loc: type_metric.loc,
            member_count: type_metric.member_count,
            is_test: type_metric.is_test,
        })
        .collect();
}

fn collect_churn_metrics(
    root: &Path,
    args: &ScanArgs,
    raw_metrics: &mut RawMetrics,
) -> Result<ChurnSummary> {
    let mode = args.churn.expect("effective args should set churn mode");
    let window_days = args
        .churn_window_days
        .expect("effective args should set churn window");
    let max_commit_lines = args
        .churn_max_commit_lines
        .expect("effective args should set churn max commit lines");

    if mode == ChurnMode::Off {
        return Ok(churn_summary(
            mode,
            false,
            "disabled",
            Some("churn collection disabled by configuration".to_string()),
            window_days,
            max_commit_lines,
        ));
    }

    match collect_git_churn(root, window_days, max_commit_lines) {
        Ok(churn_by_path) => {
            for file_metric in &mut raw_metrics.files {
                if let Some(churn) = churn_by_path.get(&file_metric.path) {
                    file_metric.churn = churn.clone();
                }
            }

            Ok(churn_summary(
                mode,
                true,
                "enabled",
                None,
                window_days,
                max_commit_lines,
            ))
        }
        Err(error) if mode == ChurnMode::Auto => Ok(churn_summary(
            mode,
            false,
            "unavailable",
            Some(error.to_string()),
            window_days,
            max_commit_lines,
        )),
        Err(error) => Err(error),
    }
}

fn churn_summary(
    mode: ChurnMode,
    enabled: bool,
    status: &str,
    reason: Option<String>,
    window_days: usize,
    max_commit_lines: usize,
) -> ChurnSummary {
    ChurnSummary {
        mode,
        enabled,
        status: status.to_string(),
        reason,
        window_days,
        max_commit_lines,
    }
}

fn collect_git_churn(
    root: &Path,
    window_days: usize,
    max_commit_lines: usize,
) -> Result<BTreeMap<String, ChurnFileMetric>> {
    let command_root = if root.is_file() {
        root.parent().unwrap_or(root)
    } else {
        root
    };
    let git_root_output = Command::new("git")
        .arg("-C")
        .arg(command_root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("failed to run git rev-parse")?;
    if !git_root_output.status.success() {
        anyhow::bail!("scan root is not inside a git repository");
    }

    let git_root_text = String::from_utf8_lossy(&git_root_output.stdout);
    let git_root = PathBuf::from(git_root_text.trim());
    let scan_relative = root
        .strip_prefix(&git_root)
        .ok()
        .map(path_to_git_slash)
        .unwrap_or_default();
    let since = format!("{window_days} days ago");
    let log_output = Command::new("git")
        .arg("-C")
        .arg(&git_root)
        .args([
            "log",
            "--no-merges",
            &format!("--since={since}"),
            "--numstat",
            "--format=commit:%H%x09%an",
        ])
        .output()
        .context("failed to run git log")?;
    if !log_output.status.success() {
        anyhow::bail!("failed to collect git churn");
    }

    let churn_by_relative_path = parse_git_numstat_churn(
        &String::from_utf8_lossy(&log_output.stdout),
        &scan_relative,
        max_commit_lines,
    );
    Ok(churn_by_relative_path
        .into_iter()
        .map(|(path, churn)| (display_path(&git_root.join(path)), churn))
        .collect())
}

#[derive(Debug, Clone)]
struct PendingCommitChurn {
    author: String,
    files: Vec<(String, usize, usize)>,
    total_lines: usize,
}

fn parse_git_numstat_churn(
    output: &str,
    scan_relative: &str,
    max_commit_lines: usize,
) -> BTreeMap<String, ChurnFileMetric> {
    let mut churn_by_path = BTreeMap::<String, ChurnFileMetric>::new();
    let mut authors_by_path = BTreeMap::<String, BTreeSet<String>>::new();
    let mut pending: Option<PendingCommitChurn> = None;

    for line in output.lines() {
        if let Some(header) = line.strip_prefix("commit:") {
            flush_pending_commit(
                &mut churn_by_path,
                &mut authors_by_path,
                pending.take(),
                max_commit_lines,
            );
            let author = header
                .split_once('\t')
                .map(|(_, author)| author)
                .unwrap_or_default()
                .to_string();
            pending = Some(PendingCommitChurn {
                author,
                files: Vec::new(),
                total_lines: 0,
            });
            continue;
        }

        let Some(commit) = pending.as_mut() else {
            continue;
        };
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() < 3 || fields[0] == "-" || fields[1] == "-" {
            continue;
        }
        let Ok(added) = fields[0].parse::<usize>() else {
            continue;
        };
        let Ok(deleted) = fields[1].parse::<usize>() else {
            continue;
        };
        let path = normalize_git_numstat_path(fields[2]);
        if !path_in_scan_root(&path, scan_relative) {
            continue;
        }
        commit.total_lines += added + deleted;
        commit.files.push((path, added, deleted));
    }

    flush_pending_commit(
        &mut churn_by_path,
        &mut authors_by_path,
        pending,
        max_commit_lines,
    );
    for (path, authors) in authors_by_path {
        if let Some(metric) = churn_by_path.get_mut(&path) {
            metric.authors_count = authors.len();
        }
    }
    churn_by_path
}

fn flush_pending_commit(
    churn_by_path: &mut BTreeMap<String, ChurnFileMetric>,
    authors_by_path: &mut BTreeMap<String, BTreeSet<String>>,
    pending: Option<PendingCommitChurn>,
    max_commit_lines: usize,
) {
    let Some(pending) = pending else {
        return;
    };
    if pending.total_lines > max_commit_lines {
        return;
    }

    for (path, added, deleted) in pending.files {
        let metric = churn_by_path.entry(path.clone()).or_default();
        metric.commits_touched += 1;
        metric.lines_added += added;
        metric.lines_deleted += deleted;
        metric.recent_weighted_churn += added + deleted;
        if !pending.author.is_empty() {
            authors_by_path
                .entry(path)
                .or_default()
                .insert(pending.author.clone());
        }
    }
}

fn normalize_git_numstat_path(path: &str) -> String {
    let path = path
        .rsplit_once(" => ")
        .map(|(_, new_path)| new_path)
        .unwrap_or(path);
    path.trim_matches(['{', '}']).replace('\\', "/")
}

fn path_in_scan_root(path: &str, scan_relative: &str) -> bool {
    scan_relative.is_empty()
        || path == scan_relative
        || path
            .strip_prefix(scan_relative)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn path_to_git_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn summarize_raw_metrics(raw_metrics: &RawMetrics) -> MetricsSummary {
    MetricsSummary {
        files: percentile_map([
            (
                "loc",
                raw_metrics.files.iter().map(|metric| metric.loc).collect(),
            ),
            (
                "imports",
                raw_metrics
                    .files
                    .iter()
                    .map(|metric| metric.imports)
                    .collect(),
            ),
            (
                "public_items",
                raw_metrics
                    .files
                    .iter()
                    .map(|metric| metric.public_items)
                    .collect(),
            ),
            (
                "directory_source_files",
                raw_metrics
                    .files
                    .iter()
                    .map(|metric| metric.directory_source_files)
                    .collect(),
            ),
        ]),
        functions: percentile_map([
            (
                "loc",
                raw_metrics
                    .functions
                    .iter()
                    .map(|metric| metric.loc)
                    .collect(),
            ),
            (
                "complexity",
                raw_metrics
                    .functions
                    .iter()
                    .map(|metric| metric.complexity)
                    .collect(),
            ),
            (
                "nesting_depth",
                raw_metrics
                    .functions
                    .iter()
                    .map(|metric| metric.nesting_depth)
                    .collect(),
            ),
            (
                "parameter_count",
                raw_metrics
                    .functions
                    .iter()
                    .map(|metric| metric.parameter_count)
                    .collect(),
            ),
        ]),
        types: percentile_map([
            (
                "loc",
                raw_metrics.types.iter().map(|metric| metric.loc).collect(),
            ),
            (
                "member_count",
                raw_metrics
                    .types
                    .iter()
                    .map(|metric| metric.member_count)
                    .collect(),
            ),
        ]),
        churn: percentile_map([
            (
                "commits_touched",
                raw_metrics
                    .files
                    .iter()
                    .map(|metric| metric.churn.commits_touched)
                    .collect(),
            ),
            (
                "lines_added",
                raw_metrics
                    .files
                    .iter()
                    .map(|metric| metric.churn.lines_added)
                    .collect(),
            ),
            (
                "lines_deleted",
                raw_metrics
                    .files
                    .iter()
                    .map(|metric| metric.churn.lines_deleted)
                    .collect(),
            ),
            (
                "authors_count",
                raw_metrics
                    .files
                    .iter()
                    .map(|metric| metric.churn.authors_count)
                    .collect(),
            ),
            (
                "recent_weighted_churn",
                raw_metrics
                    .files
                    .iter()
                    .map(|metric| metric.churn.recent_weighted_churn)
                    .collect(),
            ),
        ]),
    }
}

fn percentile_map<const N: usize>(
    inputs: [(&'static str, Vec<usize>); N],
) -> BTreeMap<String, MetricPercentiles> {
    inputs
        .into_iter()
        .filter_map(|(name, values)| {
            (!values.is_empty()).then(|| (name.to_string(), percentiles(values)))
        })
        .collect()
}

fn percentiles(mut values: Vec<usize>) -> MetricPercentiles {
    values.sort_unstable();
    MetricPercentiles {
        p50: percentile(&values, 0.50),
        p75: percentile(&values, 0.75),
        p90: percentile(&values, 0.90),
        p95: percentile(&values, 0.95),
        max: values.last().copied().unwrap_or(0),
    }
}

fn percentile(values: &[usize], percentile: f64) -> usize {
    if values.is_empty() {
        return 0;
    }

    let index = ((values.len() - 1) as f64 * percentile).ceil() as usize;
    values[index.min(values.len() - 1)]
}

fn rank_hotspots(
    raw_metrics: &RawMetrics,
    metrics_summary: &MetricsSummary,
    model: HotspotModel,
) -> Vec<Hotspot> {
    let mut hotspots = Vec::new();

    for file in &raw_metrics.files {
        let static_risk = strongest_risk([
            threshold_risk(file.loc, DEFAULT_MAX_FILE_LINES),
            threshold_risk(file.imports, DEFAULT_MAX_IMPORTS) * 0.80,
            threshold_risk(file.public_items, DEFAULT_MAX_PUBLIC_ITEMS) * 0.80,
            threshold_risk(file.directory_source_files, DEFAULT_MAX_DIR_FILES) * 0.65,
            percentile_risk(file.loc, &metrics_summary.files, "loc") * 0.35,
        ]);
        let churn_risk = file_churn_risk(file, metrics_summary);
        hotspots.push(hotspot(
            HotspotLevel::File,
            file.path.clone(),
            None,
            None,
            static_risk,
            churn_risk,
            model,
        ));
    }

    for function in &raw_metrics.functions {
        let static_risk = strongest_risk([
            threshold_risk(function.loc, DEFAULT_MAX_FUNCTION_LINES),
            threshold_risk(function.complexity, DEFAULT_MAX_FUNCTION_COMPLEXITY),
            threshold_risk(function.nesting_depth, DEFAULT_MAX_NESTING_DEPTH) * 0.85,
            threshold_risk(function.parameter_count, DEFAULT_MAX_FUNCTION_PARAMETERS) * 0.75,
            percentile_risk(function.loc, &metrics_summary.functions, "loc") * 0.35,
        ]);
        let file_churn_risk = raw_metrics
            .files
            .iter()
            .find(|file| file.path == function.path)
            .map(|file| file_churn_risk(file, metrics_summary))
            .unwrap_or(0.0);
        let churn_risk = if static_risk >= 35.0 {
            file_churn_risk
        } else {
            0.0
        };
        hotspots.push(hotspot(
            HotspotLevel::Function,
            function.path.clone(),
            Some(function.line),
            Some(function.name.clone()),
            static_risk,
            churn_risk,
            model,
        ));
    }

    for type_metric in &raw_metrics.types {
        let static_risk = strongest_risk([
            threshold_risk(type_metric.loc, DEFAULT_MAX_TYPE_LINES),
            threshold_risk(type_metric.member_count, DEFAULT_MAX_TYPE_MEMBERS),
            percentile_risk(type_metric.loc, &metrics_summary.types, "loc") * 0.35,
        ]);
        let file_churn_risk = raw_metrics
            .files
            .iter()
            .find(|file| file.path == type_metric.path)
            .map(|file| file_churn_risk(file, metrics_summary))
            .unwrap_or(0.0);
        let churn_risk = if static_risk >= 35.0 {
            file_churn_risk
        } else {
            0.0
        };
        hotspots.push(hotspot(
            HotspotLevel::Type,
            type_metric.path.clone(),
            Some(type_metric.line),
            Some(type_metric.name.clone()),
            static_risk,
            churn_risk,
            model,
        ));
    }

    hotspots.retain(|hotspot| hotspot.score >= 35);
    hotspots.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.name.cmp(&right.name))
    });
    hotspots
}

fn strongest_risk<const N: usize>(risks: [f64; N]) -> f64 {
    risks.into_iter().fold(0.0, f64::max).clamp(0.0, 100.0)
}

fn percentile_risk(
    value: usize,
    summary: &BTreeMap<String, MetricPercentiles>,
    metric: &str,
) -> f64 {
    if value == 0 {
        return 0.0;
    }

    let Some(percentiles) = summary.get(metric) else {
        return 0.0;
    };

    if value >= percentiles.p95 {
        95.0
    } else if value >= percentiles.p90 {
        85.0
    } else if value >= percentiles.p75 {
        65.0
    } else if value >= percentiles.p50 {
        45.0
    } else {
        20.0
    }
}

fn threshold_risk(value: usize, threshold: usize) -> f64 {
    if threshold == 0 || value < threshold {
        return 0.0;
    }

    normalized_threshold_excess(value as f64 / threshold as f64) * 100.0
}

fn file_churn_risk(file: &FileRawMetric, metrics_summary: &MetricsSummary) -> f64 {
    strongest_risk([
        percentile_risk(
            file.churn.commits_touched,
            &metrics_summary.churn,
            "commits_touched",
        ),
        percentile_risk(
            file.churn.recent_weighted_churn,
            &metrics_summary.churn,
            "recent_weighted_churn",
        ),
        percentile_risk(
            file.churn.authors_count,
            &metrics_summary.churn,
            "authors_count",
        ) * 0.70,
    ])
}

fn hotspot(
    level: HotspotLevel,
    path: String,
    line: Option<usize>,
    name: Option<String>,
    static_risk: f64,
    churn_risk: f64,
    model: HotspotModel,
) -> Hotspot {
    let score = match model {
        HotspotModel::Static => static_risk,
        HotspotModel::Churn => churn_risk,
        HotspotModel::Hybrid => (static_risk * 0.65) + (churn_risk * 0.35),
    }
    .round()
    .clamp(0.0, 100.0) as u8;
    let reason = hotspot_reason(static_risk, churn_risk, model);

    Hotspot {
        level,
        path,
        line,
        name,
        score,
        severity: severity_for_score(score),
        static_risk,
        churn_risk,
        reason,
    }
}

fn hotspot_reason(static_risk: f64, churn_risk: f64, model: HotspotModel) -> String {
    let model_reason = match model {
        HotspotModel::Static => "static model",
        HotspotModel::Churn => "churn model",
        HotspotModel::Hybrid => "hybrid model",
    };
    if churn_risk >= 70.0 && static_risk >= 70.0 {
        format!("{model_reason}: high static risk and high churn")
    } else if churn_risk >= static_risk {
        format!("{model_reason}: churn dominates")
    } else {
        format!("{model_reason}: static risk dominates")
    }
}

fn apply_hotspot_scores_to_findings(findings: &mut [Finding], hotspots: &[Hotspot]) {
    for finding in findings {
        if let Some(hotspot) = best_hotspot_for_finding(finding, hotspots) {
            finding.score = hotspot.score;
            finding.severity = severity_for_score(finding.score);
            finding.rank_reason = format!("{}; {}", finding.rank_reason, hotspot.reason);
        }
    }
}

fn best_hotspot_for_finding<'a>(finding: &Finding, hotspots: &'a [Hotspot]) -> Option<&'a Hotspot> {
    hotspots
        .iter()
        .filter(|hotspot| {
            hotspot.path == finding.path
                && (finding.line.is_none()
                    || hotspot.line.is_none()
                    || hotspot.line == finding.line
                    || hotspot.level == HotspotLevel::File)
        })
        .max_by_key(|hotspot| hotspot.score)
}

struct ScanSignalContext<'a> {
    args: &'a ScanArgs,
    progress: &'a mut dyn ProgressSink,
    scan: &'a mut SourceScan,
}

impl ScanSignalContext<'_> {
    fn scan_agent_drift_signals(&mut self) {
        self.progress.report(&format!(
            "Analyzing agent drift signals in {} files",
            self.scan.structure_sources.len()
        ));
        let options = AgentDriftOptions {
            min_repeated_occurrences: self.args.min_repeated_literal_occurrences,
            min_data_shape_occurrences: self.args.min_data_clump_occurrences,
            max_dir_files: self.args.max_dir_files,
            include_test_structure: self.args.include_test_structure,
        };
        self.scan
            .findings
            .extend(scan_agent_drift(&self.scan.structure_sources, &options));
    }

    fn scan_structural_signals(&mut self) -> Result<()> {
        self.progress.report(&format!(
            "Analyzing structural signals in {} files",
            self.scan.parsed_sources.len()
        ));
        let structure_options = StructureOptions {
            max_function_lines: self.args.max_function_lines,
            max_function_complexity: self.args.max_function_complexity,
            max_nesting_depth: self.args.max_nesting_depth,
            max_function_parameters: self.args.max_function_parameters,
            max_type_lines: self.args.max_type_lines,
            max_type_members: self.args.max_type_members,
            max_imports: self.args.max_imports,
            max_public_items: self.args.max_public_items,
            min_repeated_literal_occurrences: self.args.min_repeated_literal_occurrences,
            min_data_clump_occurrences: self.args.min_data_clump_occurrences,
            max_dir_files: self.args.max_dir_files,
            include_test_structure: self.args.include_test_structure,
        };
        self.scan
            .findings
            .extend(crate::structural::scan_parsed_structure(
                &self.scan.parsed_sources,
                &structure_options,
            )?);
        Ok(())
    }

    fn scan_similarity_signals(&mut self) -> Result<usize> {
        self.progress.report(&format!(
            "Analyzing similar functions in {} files",
            self.scan.parsed_sources.len()
        ));

        let similarity_options = SimilarFunctionOptions {
            min_group_size: self.args.min_similar_functions,
            min_tokens: self.args.min_function_tokens,
            threshold: self.args.function_similarity,
            include_test_similarity: self.args.include_test_similarity,
        };
        let mut similarity_progress = ScanSimilarityProgress {
            progress: self.progress,
        };
        let similarity_scan = scan_parsed_similar_functions_report_with_progress(
            &self.scan.parsed_sources,
            &similarity_options,
            &mut similarity_progress,
        )?;
        self.scan.stats.function_candidates = similarity_scan.candidate_count;
        self.scan.findings.extend(similarity_scan.findings);

        Ok(self
            .scan
            .findings
            .iter()
            .filter(|finding| finding.kind == FindingKind::SimilarFunctions)
            .count())
    }
}

fn resolve_scan_root(args: &ScanArgs) -> Result<PathBuf> {
    args.path
        .canonicalize()
        .with_context(|| format!("failed to resolve path {}", args.path.display()))
}

fn report_scan_start(
    progress: &mut dyn ProgressSink,
    root: &Path,
    total_source_files: Option<usize>,
) {
    match total_source_files {
        Some(total) => progress.report(&format!(
            "Scanning {} ({total} source {})",
            display_path(root),
            pluralize(total, "file")
        )),
        None => progress.report(&format!("Scanning {}", display_path(root))),
    }
}

fn scan_sources(
    source_plan: SourceScanPlan,
    args: &ScanArgs,
    total_source_files: Option<usize>,
    progress: &mut dyn ProgressSink,
    scan: &mut SourceScan,
) -> Result<()> {
    scan.stats.directories_scanned = source_plan.directories_scanned;
    for path in &source_plan.source_files {
        let file_options = FileScanOptions {
            max_file_lines: args.max_file_lines,
            directory_source_files: path
                .parent()
                .and_then(|parent| source_plan.directory_source_files.get(parent))
                .copied()
                .unwrap_or(0),
        };
        scan_file(path, file_options, scan)?;
        report_file_scan_progress(progress, &scan.stats, total_source_files, path);
    }

    scan_directories(
        &source_plan.directory_source_files,
        args.max_dir_files,
        &mut scan.findings,
    );
    Ok(())
}

fn report_file_scan_progress(
    progress: &mut dyn ProgressSink,
    stats: &ScanStats,
    total_source_files: Option<usize>,
    path: &Path,
) {
    if let Some(total) = total_source_files {
        let detail = display_path(path);
        progress.report_scan_progress(ProgressEvent {
            completed: stats.source_files_scanned,
            total,
            detail: &detail,
        });
    }
}

struct ScanSimilarityProgress<'a> {
    progress: &'a mut dyn ProgressSink,
}

impl SimilarFunctionProgress for ScanSimilarityProgress<'_> {
    fn report_extract_progress(&mut self, completed: usize, total: usize, path: &str) {
        self.progress.report_analysis_progress(
            ProgressEvent {
                completed,
                total,
                detail: path,
            },
            "extracting candidates",
        );
    }

    fn report_compare_progress(&mut self, completed: usize, total: usize) {
        self.progress.report_analysis_progress(
            ProgressEvent {
                completed,
                total,
                detail: "",
            },
            "comparing candidates",
        );
    }
}

fn collect_source_scan_plan(root: &Path, args: &ScanArgs) -> Result<SourceScanPlan> {
    let mut plan = SourceScanPlan::default();

    if root.is_file() {
        if is_supported_source(root) {
            plan.source_files.push(root.to_path_buf());
        }
        return Ok(plan);
    }

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| should_visit_entry(entry, root, args))
    {
        let entry = entry?;
        if entry.file_type().is_dir() {
            plan.directories_scanned += 1;
        } else if entry.file_type().is_file() && is_supported_source(entry.path()) {
            let path = entry.path().to_path_buf();
            count_source_file_parent(&path, &mut plan.directory_source_files);
            plan.source_files.push(path);
        }
    }

    Ok(plan)
}

fn scan_file(path: &Path, options: FileScanOptions, scan: &mut SourceScan) -> Result<()> {
    if !is_supported_source(path) {
        return Ok(());
    }

    scan.stats.source_files_scanned += 1;

    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read source file {}", path.display()))?;
    let source: Arc<str> = Arc::from(source);
    let line_count = source.lines().count();
    let display_path = display_path(path);
    let is_test = is_test_source(path);

    scan.raw_metrics.files.push(FileRawMetric {
        path: display_path.clone(),
        loc: line_count,
        imports: 0,
        public_items: 0,
        directory_source_files: options.directory_source_files,
        is_test,
        churn: ChurnFileMetric::default(),
    });

    if line_count > options.max_file_lines {
        scan.findings.push(finding(
            FindingKind::LargeFile,
            display_path.clone(),
            Some(1),
            format!("file has {line_count} lines; consider splitting responsibilities"),
            vec![FindingMetric::threshold(
                "file_lines",
                line_count,
                options.max_file_lines,
                "lines",
            )],
            Vec::new(),
        ));
    }

    for (index, line) in source.lines().enumerate() {
        if has_debt_marker(line) {
            scan.findings.push(finding(
                FindingKind::DebtMarker,
                display_path.clone(),
                Some(index + 1),
                "technical-debt marker found",
                Vec::new(),
                Vec::new(),
            ));
        }
    }

    if is_supported_structure_source(path) {
        let source_file = SourceFile {
            path: path.to_path_buf(),
            display_path: display_path.clone(),
            source: Arc::clone(&source),
        };

        if let Some(parsed) = parse_source_file(source_file.clone())? {
            scan.parsed_sources.push(parsed);
        }

        scan.structure_sources.push(source_file);
    }

    Ok(())
}

fn count_source_file_parent(path: &Path, directory_source_files: &mut BTreeMap<PathBuf, usize>) {
    if let Some(parent) = path.parent() {
        *directory_source_files
            .entry(parent.to_path_buf())
            .or_insert(0) += 1;
    }
}

fn scan_directories(
    directory_source_files: &BTreeMap<PathBuf, usize>,
    max_dir_files: usize,
    findings: &mut Vec<Finding>,
) {
    for (directory, file_count) in directory_source_files {
        if *file_count > max_dir_files {
            findings.push(finding(
                FindingKind::LargeDirectory,
                display_path(directory),
                None,
                format!(
                    "directory contains {file_count} source files; consider grouping related responsibilities"
                ),
                vec![FindingMetric::threshold(
                    "directory_files",
                    *file_count,
                    max_dir_files,
                    "source files",
                )],
                Vec::new(),
            ));
        }
    }
}

fn has_debt_marker(line: &str) -> bool {
    let trimmed = line.trim_start();
    let is_comment = trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with("<!--");

    if !is_comment {
        return false;
    }

    let normalized = trimmed.to_ascii_lowercase();
    normalized.contains("todo") || normalized.contains("fixme")
}

fn is_supported_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(
            "c" | "cc"
                | "cpp"
                | "cs"
                | "go"
                | "java"
                | "js"
                | "jsx"
                | "kt"
                | "py"
                | "rb"
                | "rs"
                | "ts"
                | "tsx"
        )
    )
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| name.starts_with('.'))
}

fn is_default_excluded_dir(entry: &DirEntry) -> bool {
    entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .is_some_and(|name| DEFAULT_EXCLUDED_DIRS.contains(&name))
}

fn should_visit_entry(entry: &DirEntry, root: &Path, args: &ScanArgs) -> bool {
    let is_root = entry.path() == root;
    is_root
        || ((args.include_hidden || !is_hidden(entry))
            && (args.include_generated || !is_default_excluded_dir(entry)))
            && !is_ignored_path(entry.path(), root, args)
}

fn is_ignored_path(path: &Path, root: &Path, args: &ScanArgs) -> bool {
    if args.ignore_paths.is_empty() {
        return false;
    }

    let relative = path
        .strip_prefix(root)
        .ok()
        .map(display_path)
        .unwrap_or_else(|| display_path(path));
    args.ignore_paths.iter().any(|ignore| {
        let ignore = ignore.replace('\\', "/").trim_matches('/').to_string();
        !ignore.is_empty()
            && (relative == ignore
                || relative
                    .strip_prefix(&ignore)
                    .is_some_and(|suffix| suffix.starts_with('/')))
    })
}

fn pluralize(count: usize, noun: &str) -> String {
    if count == 1 {
        noun.to_string()
    } else {
        format!("{noun}s")
    }
}

pub(crate) fn is_test_source(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    if file_name.starts_with("test_")
        || file_name.contains(".test.")
        || file_name.contains(".spec.")
        || file_name.ends_with("_test.go")
        || file_name.ends_with("_test.py")
        || file_name.ends_with("_test.rs")
        || file_name.ends_with("_tests.rs")
    {
        return true;
    }

    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| matches!(name, "test" | "tests" | "__tests__" | "spec" | "specs"))
    })
}

fn display_path(path: &Path) -> String {
    let display = path.to_string_lossy().replace('\\', "/");
    display
        .strip_prefix("//?/")
        .unwrap_or(display.as_str())
        .to_string()
}

#[cfg(test)]
#[path = "scanner_tests.rs"]
mod tests;
