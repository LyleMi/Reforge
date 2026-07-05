use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use serde::{Serialize, Serializer, ser::SerializeStruct};
use walkdir::{DirEntry, WalkDir};

use crate::agent_drift::{AgentDriftOptions, scan_agent_drift};
use crate::cli::ScanArgs;
use crate::similar_functions::{
    ParsedSourceFile, SimilarFunctionOptions, SimilarFunctionProgress, SourceFile,
    parse_source_file, scan_parsed_similar_functions_report_with_progress,
};
use crate::structural::{StructureOptions, is_supported_structure_source};

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
pub const SCAN_REPORT_SCHEMA_VERSION: u8 = 3;
const SERIALIZED_SIMILAR_LOCATION_LIMIT: usize = 50;
const PERCENTILE_MIN_SAMPLE: usize = 5;

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

fn finalize_scoring(findings: &mut [Finding]) {
    let percentile_values = percentile_metric_values(findings);

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

fn percentile_metric_values(findings: &[Finding]) -> BTreeMap<String, Vec<usize>> {
    let mut values = BTreeMap::<String, Vec<usize>>::new();

    for finding in findings {
        for metric in &finding.metrics {
            let dimension = metric_dimension(finding.kind, &metric.name);
            if matches!(
                dimension,
                MetricDimension::Size | MetricDimension::Complexity
            ) {
                values
                    .entry(metric.name.clone())
                    .or_default()
                    .push(metric.value);
            }
        }
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
    pub similar_function_group_count: usize,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ScanStats {
    pub source_files_scanned: usize,
    pub directories_scanned: usize,
    pub function_candidates: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScanReport {
    pub schema_version: u8,
    pub summary: ScanSummary,
    pub stats: ScanStats,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Default)]
struct SourceScan {
    findings: Vec<Finding>,
    parsed_sources: Vec<ParsedSourceFile>,
    structure_sources: Vec<SourceFile>,
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
    let mut scan = SourceScan::default();
    let source_plan = collect_source_scan_plan(&root, args)?;

    let total_source_files = progress
        .wants_detailed_progress()
        .then_some(source_plan.source_files.len());

    report_scan_start(progress, &root, total_source_files);
    scan_sources(source_plan, args, total_source_files, progress, &mut scan)?;
    let similar_function_group_count = {
        let mut signals = ScanSignalContext {
            args,
            progress,
            scan: &mut scan,
        };
        signals.scan_structural_signals()?;
        signals.scan_agent_drift_signals();
        signals.scan_similarity_signals()?
    };
    finalize_scoring(&mut scan.findings);

    let summary = ScanSummary {
        scanned_files: scan.stats.source_files_scanned,
        finding_count: scan.findings.len(),
        similar_function_group_count,
        duration_ms: started_at.elapsed().as_millis(),
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
        findings: scan.findings,
    })
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
    let file_options = FileScanOptions {
        max_file_lines: args.max_file_lines,
    };

    scan.stats.directories_scanned = source_plan.directories_scanned;
    for path in &source_plan.source_files {
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

    if line_count > options.max_file_lines {
        scan.findings.push(finding(
            FindingKind::LargeFile,
            display_path(path),
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
                display_path(path),
                Some(index + 1),
                "technical-debt marker found",
                Vec::new(),
                Vec::new(),
            ));
        }
    }

    let display_path = display_path(path);
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
