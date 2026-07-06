use std::collections::{BTreeMap, BTreeSet};

use crate::cli::HotspotModel;
use crate::model::{
    FileRawMetric, Finding, FindingKind, FindingMetric, Hotspot, HotspotLevel, MetricDimension,
    MetricPercentiles, MetricsSummary, PriorityFactors, RawMetrics, RelatedLocation, Severity,
};

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
        FindingKind::GenericBucketDrift => 0.70,
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
    (weighted * factors.confidence).round().clamp(0.0, 100.0) as u8
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
            return scoped.churn_risk.clamp(0.0, 100.0);
        }

        return best_file_hotspot_for_finding(finding, hotspots)
            .map(|hotspot| hotspot.churn_risk * 0.50)
            .unwrap_or(0.0)
            .clamp(0.0, 100.0);
    }

    best_file_hotspot_for_finding(finding, hotspots)
        .map(|hotspot| hotspot.churn_risk)
        .unwrap_or(0.0)
        .clamp(0.0, 100.0)
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
        FindingKind::MissingMetricsModelDocs | FindingKind::MissingArchitectureDocs => 70.0,
        FindingKind::MissingDocumentationSet
        | FindingKind::MissingUserGuide
        | FindingKind::MissingReportSchemaDocs
        | FindingKind::StaleCliDocumentation
        | FindingKind::StaleSchemaDocumentation => 85.0,
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

pub(crate) fn metric_dimension(kind: FindingKind, metric_name: &str) -> MetricDimension {
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

pub(crate) fn rank_hotspots(
    raw_metrics: &RawMetrics,
    metrics_summary: &MetricsSummary,
    model: HotspotModel,
) -> Vec<Hotspot> {
    HotspotRanking::new(raw_metrics, metrics_summary, model).rank()
}

struct HotspotRanking<'a> {
    raw_metrics: &'a RawMetrics,
    metrics_summary: &'a MetricsSummary,
    model: HotspotModel,
}

impl<'a> HotspotRanking<'a> {
    fn new(
        raw_metrics: &'a RawMetrics,
        metrics_summary: &'a MetricsSummary,
        model: HotspotModel,
    ) -> Self {
        Self {
            raw_metrics,
            metrics_summary,
            model,
        }
    }

    fn rank(self) -> Vec<Hotspot> {
        let mut hotspots = Vec::new();

        self.append_file_hotspots(&mut hotspots);
        self.append_function_hotspots(&mut hotspots);
        self.append_type_hotspots(&mut hotspots);

        hotspots.retain(|hotspot| hotspot.priority >= 35);
        sort_hotspots(&mut hotspots);
        hotspots
    }

    fn append_file_hotspots(&self, hotspots: &mut Vec<Hotspot>) {
        for file in &self.raw_metrics.files {
            let static_risk = file_static_risk(file, self.metrics_summary);
            hotspots.push(hotspot(HotspotInput {
                level: HotspotLevel::File,
                path: file.path.clone(),
                line: None,
                name: None,
                static_risk,
                churn_risk: file_churn_risk(file, self.metrics_summary),
                model: self.model,
            }));
        }
    }

    fn append_function_hotspots(&self, hotspots: &mut Vec<Hotspot>) {
        for function in &self.raw_metrics.functions {
            let static_risk = function_static_risk(function, self.metrics_summary);
            hotspots.push(hotspot(HotspotInput {
                level: HotspotLevel::Function,
                path: function.path.clone(),
                line: Some(function.line),
                name: Some(function.name.clone()),
                static_risk,
                churn_risk: self.scoped_churn_risk(&function.path, static_risk),
                model: self.model,
            }));
        }
    }

    fn append_type_hotspots(&self, hotspots: &mut Vec<Hotspot>) {
        for type_metric in &self.raw_metrics.types {
            let static_risk = type_static_risk(type_metric, self.metrics_summary);
            hotspots.push(hotspot(HotspotInput {
                level: HotspotLevel::Type,
                path: type_metric.path.clone(),
                line: Some(type_metric.line),
                name: Some(type_metric.name.clone()),
                static_risk,
                churn_risk: self.scoped_churn_risk(&type_metric.path, static_risk),
                model: self.model,
            }));
        }
    }

    fn scoped_churn_risk(&self, path: &str, static_risk: f64) -> f64 {
        if static_risk < 35.0 {
            return 0.0;
        }

        self.raw_metrics
            .files
            .iter()
            .find(|file| file.path == path)
            .map(|file| file_churn_risk(file, self.metrics_summary))
            .unwrap_or(0.0)
    }
}

fn sort_hotspots(hotspots: &mut [Hotspot]) {
    hotspots.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.name.cmp(&right.name))
    });
}

fn file_static_risk(file: &FileRawMetric, metrics_summary: &MetricsSummary) -> f64 {
    strongest_risk([
        threshold_risk(file.loc, DEFAULT_MAX_FILE_LINES),
        threshold_risk(file.imports, DEFAULT_MAX_IMPORTS) * 0.80,
        threshold_risk(file.public_items, DEFAULT_MAX_PUBLIC_ITEMS) * 0.80,
        threshold_risk(file.directory_source_files, DEFAULT_MAX_DIR_FILES) * 0.65,
        percentile_risk(file.loc, &metrics_summary.files, "loc") * 0.35,
    ])
}

fn function_static_risk(
    function: &crate::model::FunctionRawMetric,
    metrics_summary: &MetricsSummary,
) -> f64 {
    strongest_risk([
        threshold_risk(function.loc, DEFAULT_MAX_FUNCTION_LINES),
        threshold_risk(function.complexity, DEFAULT_MAX_FUNCTION_COMPLEXITY),
        threshold_risk(function.nesting_depth, DEFAULT_MAX_NESTING_DEPTH) * 0.85,
        threshold_risk(function.parameter_count, DEFAULT_MAX_FUNCTION_PARAMETERS) * 0.75,
        percentile_risk(function.loc, &metrics_summary.functions, "loc") * 0.35,
    ])
}

fn type_static_risk(
    type_metric: &crate::model::TypeRawMetric,
    metrics_summary: &MetricsSummary,
) -> f64 {
    strongest_risk([
        threshold_risk(type_metric.loc, DEFAULT_MAX_TYPE_LINES),
        threshold_risk(type_metric.member_count, DEFAULT_MAX_TYPE_MEMBERS),
        percentile_risk(type_metric.loc, &metrics_summary.types, "loc") * 0.35,
    ])
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

struct HotspotInput {
    level: HotspotLevel,
    static_risk: f64,
    churn_risk: f64,
    path: String,
    line: Option<usize>,
    name: Option<String>,
    model: HotspotModel,
}

fn hotspot(input: HotspotInput) -> Hotspot {
    let priority = match input.model {
        HotspotModel::Static => input.static_risk,
        HotspotModel::Churn => input.churn_risk,
        HotspotModel::Hybrid => (input.static_risk * 0.65) + (input.churn_risk * 0.35),
    }
    .round()
    .clamp(0.0, 100.0) as u8;
    let reason = hotspot_reason(input.static_risk, input.churn_risk, input.model);

    Hotspot {
        level: input.level,
        path: input.path,
        line: input.line,
        name: input.name,
        priority,
        severity: severity_for_priority(priority),
        static_risk: input.static_risk,
        churn_risk: input.churn_risk,
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
