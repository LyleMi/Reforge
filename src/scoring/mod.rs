use std::collections::{BTreeMap, BTreeSet};

use crate::cli::HotspotModel;
use crate::model::{
    FileRawMetric, Finding, FindingKind, FindingMetric, Hotspot, HotspotLevel,
    METRIC_NESTING_DEPTH, METRIC_PUBLIC_ITEMS, MetricDimension, MetricPercentiles, MetricsSummary,
    PriorityFactors, RawMetrics, RelatedLocation, Severity,
};

mod hotspots;

pub(crate) use hotspots::rank_hotspots;

const PERCENTILE_MIN_SAMPLE: usize = 5;
const DEFAULT_MAX_FILE_LINES: usize = 800;
const DEFAULT_MAX_DIR_FILES: usize = 40;
const DEFAULT_MAX_FUNCTION_LINES: usize = 80;
const DEFAULT_MAX_FUNCTION_COMPLEXITY: usize = 15;
const DEFAULT_MAX_NESTING_DEPTH: usize = 4;
const DEFAULT_MAX_FUNCTION_PARAMETERS: usize = 5;
const DEFAULT_MAX_TYPE_LINES: usize = 250;
const DEFAULT_MAX_TYPE_MEMBERS: usize = 30;
const DEFAULT_MAX_IMPORTS: usize = 35;
const DEFAULT_MAX_PUBLIC_ITEMS: usize = 30;
const MIN_SCORE: f64 = 0.0;
const MAX_SCORE: f64 = 100.0;

#[derive(Debug, Clone)]
pub struct FindingInput {
    kind: FindingKind,
    path: String,
    line: Option<usize>,
    message: String,
    metrics: Vec<FindingMetric>,
    confidence: Option<f64>,
    related_locations: Vec<RelatedLocation>,
}

impl FindingInput {
    pub fn new(
        kind: FindingKind,
        path: impl Into<String>,
        line: Option<usize>,
        message: impl Into<String>,
        metrics: Vec<FindingMetric>,
    ) -> Self {
        Self {
            kind,
            path: path.into(),
            line,
            message: message.into(),
            metrics,
            confidence: None,
            related_locations: Vec::new(),
        }
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence);
        self
    }

    pub fn with_related_locations(mut self, related_locations: Vec<RelatedLocation>) -> Self {
        self.related_locations = related_locations;
        self
    }
}

pub fn finding(input: FindingInput) -> Finding {
    build_finding(input)
}

pub fn scored_finding(input: FindingInput) -> Finding {
    build_finding(input)
}

fn build_finding(input: FindingInput) -> Finding {
    let confidence = input
        .confidence
        .unwrap_or_else(|| default_confidence(input.kind));
    let mut finding = Finding {
        kind: input.kind,
        severity: Severity::Info,
        path: input.path,
        line: input.line,
        metrics: input.metrics,
        priority: 0,
        confidence,
        priority_factors: priority_factors(
            input.kind,
            &[],
            confidence,
            &input.related_locations,
            0.0,
        ),
        rank_explanation: String::new(),
        message: input.message,
        related_locations: input.related_locations,
    };
    refresh_finding_priority(&mut finding, 0.0);
    finding
}

pub fn severity_for_priority(priority: u8) -> Severity {
    match priority {
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
    let factors = priority_factors(kind, metrics, confidence, related_locations, 0.0);
    priority_from_factors(&factors)
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
        FindingKind::GenericBucketDrift | FindingKind::StaleCompatibilityPath => 0.70,
        FindingKind::HappyPathOnlyTests => 0.60,
        FindingKind::MissingDocumentationSet
        | FindingKind::MissingUserGuide
        | FindingKind::MissingReportSchemaDocs
        | FindingKind::MissingMetricsModelDocs
        | FindingKind::MissingArchitectureDocs
        | FindingKind::StaleCliDocumentation
        | FindingKind::StaleSchemaDocumentation => 0.95,
        FindingKind::FileNamingDrift
        | FindingKind::DirectoryDrift
        | FindingKind::FunctionProliferation
        | FindingKind::UnusedFunction
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

pub(crate) fn finalize_scoring(
    findings: &mut [Finding],
    raw_metrics: &RawMetrics,
    hotspots: &[Hotspot],
) {
    let percentile_values = percentile_metric_values(raw_metrics);

    for finding in findings {
        for metric in &mut finding.metrics {
            metric.dimension = metric_dimension(finding.kind, &metric.name);
            let threshold_normalized = metric
                .excess_ratio
                .map(normalized_threshold_excess)
                .unwrap_or(0.20);
            metric.percentile = metric_percentile(metric, &percentile_values);
            metric.normalized = Some(normalized_metric_value(metric, threshold_normalized));
        }

        let change_pressure = change_pressure_score(finding, hotspots);
        refresh_finding_priority(finding, change_pressure);
    }
}

fn refresh_finding_priority(finding: &mut Finding, change_pressure: f64) {
    for metric in &mut finding.metrics {
        metric.dimension = metric_dimension(finding.kind, &metric.name);
    }

    finding.priority_factors = priority_factors(
        finding.kind,
        &finding.metrics,
        finding.confidence,
        &finding.related_locations,
        change_pressure,
    );
    finding.priority = priority_from_factors(&finding.priority_factors);
    finding.severity = severity_for_priority(finding.priority);
    finding.rank_explanation = rank_explanation(
        finding.kind,
        &finding.priority_factors,
        &finding.related_locations,
    );
}

fn priority_factors(
    kind: FindingKind,
    metrics: &[FindingMetric],
    confidence: f64,
    related_locations: &[RelatedLocation],
    change_pressure: f64,
) -> PriorityFactors {
    PriorityFactors {
        impact: impact_score(kind),
        intensity: intensity_score(metrics),
        spread: spread_score(related_locations),
        change_pressure,
        actionability: actionability_score(kind),
        confidence: confidence.clamp(0.0, 1.0),
    }
}

fn priority_from_factors(factors: &PriorityFactors) -> u8 {
    let weighted = (factors.impact * 0.30)
        + (factors.intensity * 0.30)
        + (factors.spread * 0.15)
        + (factors.change_pressure * 0.15)
        + (factors.actionability * 0.10);
    (weighted * factors.confidence)
        .round()
        .clamp(MIN_SCORE, MAX_SCORE) as u8
}

fn intensity_score(metrics: &[FindingMetric]) -> f64 {
    let strongest = metrics
        .iter()
        .filter_map(|metric| metric.normalized)
        .max_by(f64::total_cmp)
        .unwrap_or(0.20);
    (strongest * MAX_SCORE).clamp(MIN_SCORE, MAX_SCORE)
}

fn spread_score(related_locations: &[RelatedLocation]) -> f64 {
    let unique_related_files = unique_related_file_count(related_locations);
    if unique_related_files <= 1 {
        return 0.0;
    }

    let file_spread = (unique_related_files as f64).ln_1p() / 8.0_f64.ln_1p();
    (35.0 + file_spread * 50.0).min(85.0)
}

fn change_pressure_score(finding: &Finding, hotspots: &[Hotspot]) -> f64 {
    if hotspots.is_empty() {
        return 0.0;
    }

    if finding.line.is_some() {
        if let Some(scoped) = best_scoped_hotspot_for_finding(finding, hotspots) {
            return scoped.churn_risk.clamp(MIN_SCORE, MAX_SCORE);
        }

        return best_file_hotspot_for_finding(finding, hotspots)
            .map(|hotspot| hotspot.churn_risk * 0.50)
            .unwrap_or(0.0)
            .clamp(MIN_SCORE, MAX_SCORE);
    }

    best_file_hotspot_for_finding(finding, hotspots)
        .map(|hotspot| hotspot.churn_risk)
        .unwrap_or(0.0)
        .clamp(MIN_SCORE, MAX_SCORE)
}

fn best_scoped_hotspot_for_finding<'a>(
    finding: &Finding,
    hotspots: &'a [Hotspot],
) -> Option<&'a Hotspot> {
    hotspots
        .iter()
        .filter(|hotspot| {
            hotspot.path == finding.path
                && matches!(hotspot.level, HotspotLevel::Function | HotspotLevel::Type)
                && hotspot.line == finding.line
        })
        .max_by_key(|hotspot| hotspot.priority)
}

fn best_file_hotspot_for_finding<'a>(
    finding: &Finding,
    hotspots: &'a [Hotspot],
) -> Option<&'a Hotspot> {
    hotspots
        .iter()
        .filter(|hotspot| hotspot.path == finding.path && hotspot.level == HotspotLevel::File)
        .max_by_key(|hotspot| hotspot.priority)
}

fn impact_score(kind: FindingKind) -> f64 {
    match kind {
        FindingKind::DebtMarker => 25.0,
        FindingKind::RepeatedLiteral | FindingKind::FileNamingDrift => 40.0,
        FindingKind::HappyPathOnlyTests | FindingKind::ShadowedAbstraction => 45.0,
        FindingKind::ConfigKeyDrift | FindingKind::FixtureFactoryDrift => 50.0,
        FindingKind::MissingDocumentationSet
        | FindingKind::MissingUserGuide
        | FindingKind::MissingMetricsModelDocs
        | FindingKind::MissingArchitectureDocs => 70.0,
        FindingKind::StaleCliDocumentation => 75.0,
        FindingKind::MissingReportSchemaDocs | FindingKind::StaleSchemaDocumentation => 90.0,
        FindingKind::LargeFile | FindingKind::LargeDirectory => 65.0,
        FindingKind::LongFunction
        | FindingKind::DeepNesting
        | FindingKind::ManyParameters
        | FindingKind::LargeType => 70.0,
        FindingKind::LargePublicSurface
        | FindingKind::ImportHeavyFile
        | FindingKind::FunctionProliferation
        | FindingKind::UnusedFunction => 60.0,
        FindingKind::RepeatedErrorPattern
        | FindingKind::TestDuplication
        | FindingKind::DirectoryDrift
        | FindingKind::DataClump
        | FindingKind::DuplicateTypeShape
        | FindingKind::GenericBucketDrift
        | FindingKind::StaleCompatibilityPath => 65.0,
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
        FindingKind::MissingMetricsModelDocs | FindingKind::MissingArchitectureDocs => 70.0,
        FindingKind::MissingDocumentationSet
        | FindingKind::MissingUserGuide
        | FindingKind::MissingReportSchemaDocs
        | FindingKind::StaleCliDocumentation
        | FindingKind::StaleSchemaDocumentation => 85.0,
        FindingKind::ShadowedAbstraction
        | FindingKind::ConfigKeyDrift
        | FindingKind::FixtureFactoryDrift
        | FindingKind::GenericBucketDrift
        | FindingKind::StaleCompatibilityPath => 65.0,
        FindingKind::RepeatedErrorPattern | FindingKind::TestDuplication => 70.0,
        FindingKind::LargeDirectory
        | FindingKind::ImportHeavyFile
        | FindingKind::LargePublicSurface
        | FindingKind::FunctionProliferation
        | FindingKind::UnusedFunction
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

pub(crate) fn metric_dimension(kind: FindingKind, metric_name: &str) -> MetricDimension {
    match kind {
        FindingKind::LargeFile
        | FindingKind::LargeDirectory
        | FindingKind::LongFunction
        | FindingKind::LargeType
        | FindingKind::UnusedFunction => MetricDimension::Size,
        FindingKind::ComplexFunction
        | FindingKind::DeepNesting
        | FindingKind::ManyParameters
        | FindingKind::FunctionProliferation => MetricDimension::Complexity,
        FindingKind::LargePublicSurface
        | FindingKind::ImportHeavyFile
        | FindingKind::AdapterBoundaryBypass => MetricDimension::Coupling,
        FindingKind::SimilarFunctions
        | FindingKind::RepeatedLiteral
        | FindingKind::RepeatedErrorPattern
        | FindingKind::DataClump
        | FindingKind::DuplicateTypeShape => MetricDimension::Duplication,
        FindingKind::TestDuplication | FindingKind::HappyPathOnlyTests => MetricDimension::TestRisk,
        FindingKind::MissingDocumentationSet
        | FindingKind::MissingUserGuide
        | FindingKind::MissingReportSchemaDocs
        | FindingKind::MissingMetricsModelDocs
        | FindingKind::MissingArchitectureDocs
        | FindingKind::StaleCliDocumentation
        | FindingKind::StaleSchemaDocumentation => MetricDimension::Documentation,
        FindingKind::FileNamingDrift
        | FindingKind::DirectoryDrift
        | FindingKind::ParallelImplementation
        | FindingKind::ShadowedAbstraction
        | FindingKind::ConfigKeyDrift
        | FindingKind::FixtureFactoryDrift
        | FindingKind::GenericBucketDrift
        | FindingKind::StaleCompatibilityPath => MetricDimension::Drift,
        FindingKind::DebtMarker => match metric_name {
            "imports" | METRIC_PUBLIC_ITEMS => MetricDimension::Coupling,
            "function_complexity" | METRIC_NESTING_DEPTH | "function_parameters" => {
                MetricDimension::Complexity
            }
            _ => MetricDimension::Size,
        },
    }
}

pub fn normalized_threshold_excess(ratio: f64) -> f64 {
    let ratio = ratio.max(0.0);
    if ratio <= 1.0 {
        return 0.35;
    }

    let log_component = ratio.ln_1p() / 5.0_f64.ln_1p();
    (0.35 + (log_component * 0.65)).clamp(0.35, 1.0)
}

fn rank_explanation(
    kind: FindingKind,
    factors: &PriorityFactors,
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
        MetricDimension::Documentation => "documentation signal",
    });

    if unique_related_file_count(related_locations) > 1 {
        reasons.push("cross-file spread");
    }

    if factors.change_pressure >= 70.0 {
        reasons.push("high churn pressure");
    } else if factors.change_pressure >= 35.0 {
        reasons.push("moderate churn pressure");
    }

    if factors.confidence >= 0.85 {
        reasons.push("high confidence");
    } else if factors.confidence >= 0.65 {
        reasons.push("medium confidence");
    } else {
        reasons.push("low confidence");
    }

    if factors.actionability < 60.0 {
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
        .collect::<BTreeSet<_>>()
        .len()
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
            .entry(METRIC_PUBLIC_ITEMS.to_string())
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
            .entry(METRIC_NESTING_DEPTH.to_string())
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

fn metric_percentile(
    metric: &FindingMetric,
    percentile_values: &BTreeMap<String, Vec<usize>>,
) -> Option<f64> {
    let values = percentile_values.get(&metric.name)?;
    if values.len() < PERCENTILE_MIN_SAMPLE {
        return None;
    }

    let rank = values.partition_point(|value| *value <= metric.value);
    Some((rank as f64 / values.len() as f64).clamp(0.0, 1.0))
}

fn normalized_metric_value(metric: &FindingMetric, threshold_normalized: f64) -> f64 {
    if !matches!(
        metric.dimension,
        MetricDimension::Size | MetricDimension::Complexity | MetricDimension::Coupling
    ) {
        return threshold_normalized;
    }

    let Some(percentile) = metric.percentile else {
        return threshold_normalized;
    };

    ((threshold_normalized * 0.65) + (percentile * 0.35)).clamp(0.0, 1.0)
}

pub(crate) fn summarize_raw_metrics(raw_metrics: &RawMetrics) -> MetricsSummary {
    MetricsSummary {
        files: file_percentiles(raw_metrics),
        functions: function_percentiles(raw_metrics),
        types: type_percentiles(raw_metrics),
        churn: churn_percentiles(raw_metrics),
    }
}

fn file_percentiles(raw_metrics: &RawMetrics) -> BTreeMap<String, MetricPercentiles> {
    percentile_map([
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
            METRIC_PUBLIC_ITEMS,
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
    ])
}

fn function_percentiles(raw_metrics: &RawMetrics) -> BTreeMap<String, MetricPercentiles> {
    percentile_map([
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
            METRIC_NESTING_DEPTH,
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
    ])
}

fn type_percentiles(raw_metrics: &RawMetrics) -> BTreeMap<String, MetricPercentiles> {
    percentile_map([
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
    ])
}

fn churn_percentiles(raw_metrics: &RawMetrics) -> BTreeMap<String, MetricPercentiles> {
    percentile_map([
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
    ])
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
