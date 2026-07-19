use std::collections::{BTreeMap, BTreeSet};

use crate::cli::HotspotModel;
use crate::detectors::manifest::{
    actionability, classification, default_detection_reliability,
    default_interpretation_reliability, impact, input_metrics,
};
use crate::model::{
    FileRawMetric, Finding, FindingKind, FindingMetric, Hotspot, HotspotLevel, MetricId,
    MetricPercentiles, MetricsSummary, PriorityFactors, RawMetrics, RelatedLocation, Severity,
    SignalMechanism,
};

mod clusters;
mod hotspots;
mod policy;

pub(crate) use clusters::cluster_findings;
pub(crate) use hotspots::{StaticRiskThresholds, rank_hotspots};
pub(crate) use policy::load_scoring_policy;

const PERCENTILE_MIN_SAMPLE: usize = 5;
const MIN_SCORE: f64 = 0.0;
const MAX_SCORE: f64 = 100.0;

#[derive(Debug, Clone)]
pub struct FindingInput {
    kind: FindingKind,
    path: String,
    line: Option<usize>,
    message: String,
    metrics: Vec<FindingMetric>,
    detection_reliability: Option<f64>,
    interpretation_reliability: Option<f64>,
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
        let declared_metrics = input_metrics(kind);
        assert!(
            metrics
                .iter()
                .all(|metric| declared_metrics.contains(&metric.name)),
            "finding {kind:?} emitted a metric outside its detector contract"
        );
        Self {
            kind,
            path: path.into(),
            line,
            message: message.into(),
            metrics,
            detection_reliability: None,
            interpretation_reliability: None,
            related_locations: Vec::new(),
        }
    }

    pub fn with_detection_reliability(mut self, reliability: f64) -> Self {
        self.detection_reliability = Some(reliability);
        self
    }

    pub fn with_related_locations(mut self, related_locations: Vec<RelatedLocation>) -> Self {
        self.related_locations = related_locations;
        self
    }
}

impl From<FindingInput> for Finding {
    fn from(input: FindingInput) -> Self {
        let detection_reliability = input
            .detection_reliability
            .unwrap_or_else(|| detection_reliability(input.kind));
        let interpretation_reliability = input
            .interpretation_reliability
            .unwrap_or_else(|| default_interpretation_reliability(input.kind));
        let (construct, mechanism) = classification(input.kind);
        let mut finding = Self {
            id: Default::default(),
            kind: input.kind,
            severity: Severity::Info,
            path: input.path,
            line: input.line,
            metrics: input.metrics,
            construct,
            mechanism,
            issue_id: None,
            priority: 0,
            detection_reliability,
            interpretation_reliability,
            priority_factors: priority_factors(PriorityFactorInput {
                kind: input.kind,
                metrics: &[],
                detection_reliability,
                interpretation_reliability,
                related_locations: &input.related_locations,
                change_pressure: 0.0,
            }),
            rank_explanation: String::new(),
            message: input.message,
            related_locations: input.related_locations,
        };
        refresh_finding_priority(&mut finding, 0.0);
        finding.refresh_id();
        finding
    }
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
    detection_reliability: f64,
    related_locations: &[RelatedLocation],
) -> u8 {
    let factors = priority_factors(PriorityFactorInput {
        kind,
        metrics,
        detection_reliability,
        interpretation_reliability: 0.90,
        related_locations,
        change_pressure: 0.0,
    });
    priority_from_factors(&factors)
}

pub fn detection_reliability(kind: FindingKind) -> f64 {
    default_detection_reliability(kind)
}

#[cfg(test)]
pub(crate) fn finalize_scoring(
    findings: &mut [Finding],
    raw_metrics: &RawMetrics,
    hotspots: &[Hotspot],
) {
    finalize_scoring_with_policy(
        findings,
        raw_metrics,
        hotspots,
        &crate::model::EffectiveScoringPolicy::builtin(),
    );
}

pub(crate) fn finalize_scoring_with_policy(
    findings: &mut [Finding],
    raw_metrics: &RawMetrics,
    hotspots: &[Hotspot],
    policy: &crate::model::EffectiveScoringPolicy,
) {
    let percentile_values = percentile_metric_values(raw_metrics);

    for finding in findings {
        if let Some(reliability) = policy.detector_reliability.get(&finding.kind) {
            finding.detection_reliability = reliability.detection;
            finding.interpretation_reliability = reliability.interpretation;
        }
        for metric in &mut finding.metrics {
            let threshold_normalized = metric
                .excess_ratio
                .map(normalized_threshold_excess)
                .unwrap_or(0.20);
            metric.percentile = metric_percentile(metric, &percentile_values);
            metric.normalized = Some(normalized_metric_value(metric, threshold_normalized));
        }

        let change_pressure = change_pressure_score(finding, hotspots);
        refresh_finding_priority_with_weights(finding, change_pressure, policy.global_weights);
    }
}

fn refresh_finding_priority(finding: &mut Finding, change_pressure: f64) {
    refresh_finding_priority_with_weights(
        finding,
        change_pressure,
        crate::model::ScoringWeights::default(),
    );
}

fn refresh_finding_priority_with_weights(
    finding: &mut Finding,
    change_pressure: f64,
    weights: crate::model::ScoringWeights,
) {
    finding.priority_factors = priority_factors(PriorityFactorInput {
        kind: finding.kind,
        metrics: &finding.metrics,
        detection_reliability: finding.detection_reliability,
        interpretation_reliability: finding.interpretation_reliability,
        related_locations: &finding.related_locations,
        change_pressure,
    });
    finding.priority = priority_from_factors_with_weights(&finding.priority_factors, weights);
    finding.severity = severity_for_priority(finding.priority);
    finding.rank_explanation = rank_explanation(
        finding.kind,
        &finding.priority_factors,
        &finding.related_locations,
    );
    finding.refresh_id();
}

struct PriorityFactorInput<'a> {
    kind: FindingKind,
    metrics: &'a [FindingMetric],
    detection_reliability: f64,
    interpretation_reliability: f64,
    related_locations: &'a [RelatedLocation],
    change_pressure: f64,
}

fn priority_factors(input: PriorityFactorInput<'_>) -> PriorityFactors {
    PriorityFactors {
        impact: impact_score(input.kind),
        intensity: intensity_score(input.metrics),
        spread: spread_score(input.related_locations),
        change_pressure: input.change_pressure,
        actionability: actionability_score(input.kind),
        detection_reliability: input.detection_reliability.clamp(0.0, 1.0),
        interpretation_reliability: input.interpretation_reliability.clamp(0.0, 1.0),
    }
}

pub(crate) fn priority_from_factors(factors: &PriorityFactors) -> u8 {
    priority_from_factors_with_weights(factors, crate::model::ScoringWeights::default())
}

fn priority_from_factors_with_weights(
    factors: &PriorityFactors,
    weights: crate::model::ScoringWeights,
) -> u8 {
    let weighted = (factors.impact * weights.impact)
        + (factors.intensity * weights.intensity)
        + (factors.spread * weights.spread)
        + (factors.change_pressure * weights.change_pressure)
        + (factors.actionability * weights.actionability);
    (weighted * factors.detection_reliability * factors.interpretation_reliability)
        .round()
        .clamp(MIN_SCORE, MAX_SCORE) as u8
}

fn intensity_score(metrics: &[FindingMetric]) -> f64 {
    let strongest = metrics
        .iter()
        .filter(|metric| {
            !matches!(
                metric.name,
                MetricId::FileIsTest | MetricId::FunctionIsTest | MetricId::TypeIsTest
            )
        })
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
    impact(kind)
}

fn actionability_score(kind: FindingKind) -> f64 {
    actionability(kind)
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
    let mut reasons = vec![mechanism_explanation(classification(kind).1)];

    if unique_related_file_count(related_locations) > 1 {
        reasons.push("cross-file spread");
    }
    if let Some(reason) = change_pressure_explanation(factors.change_pressure) {
        reasons.push(reason);
    }
    reasons.push(action_probability_explanation(factors));
    if factors.actionability < 60.0 {
        reasons.push("lower actionability");
    }
    reasons.join(", ")
}

fn mechanism_explanation(mechanism: SignalMechanism) -> &'static str {
    match mechanism {
        SignalMechanism::CognitiveLoad => "cognitive-load signal",
        SignalMechanism::DependencyPropagation => "dependency-propagation signal",
        SignalMechanism::ResponsibilityDispersion => "responsibility-dispersion signal",
        SignalMechanism::DuplicationDivergence => "duplication-divergence signal",
        SignalMechanism::ChangePressure => "change-pressure signal",
        SignalMechanism::VerificationDifficulty => "verification-difficulty signal",
        SignalMechanism::KnowledgeDrift => "knowledge-drift signal",
    }
}

fn change_pressure_explanation(change_pressure: f64) -> Option<&'static str> {
    if change_pressure >= 70.0 {
        Some("high churn pressure")
    } else if change_pressure >= 35.0 {
        Some("moderate churn pressure")
    } else {
        None
    }
}

fn action_probability_explanation(factors: &PriorityFactors) -> &'static str {
    let probability = factors.detection_reliability * factors.interpretation_reliability;
    if probability >= 0.85 {
        "high action probability"
    } else if probability >= 0.65 {
        "medium action probability"
    } else {
        "low action probability"
    }
}

fn unique_related_file_count(related_locations: &[RelatedLocation]) -> usize {
    related_locations
        .iter()
        .map(|location| location.path.as_str())
        .collect::<BTreeSet<_>>()
        .len()
}

fn percentile_metric_values(raw_metrics: &RawMetrics) -> BTreeMap<MetricId, Vec<usize>> {
    let mut values = BTreeMap::<MetricId, Vec<usize>>::new();

    for directory in &raw_metrics.directories {
        values
            .entry(MetricId::DirectorySourceFiles)
            .or_default()
            .push(directory.source_files);
    }

    for file in &raw_metrics.files {
        values.entry(MetricId::FileLoc).or_default().push(file.loc);
        values
            .entry(MetricId::FileImports)
            .or_default()
            .push(file.imports);
        values
            .entry(MetricId::FilePublicItems)
            .or_default()
            .push(file.public_items);
    }

    for function in &raw_metrics.functions {
        values
            .entry(MetricId::FunctionLoc)
            .or_default()
            .push(function.loc);
        values
            .entry(MetricId::FunctionComplexity)
            .or_default()
            .push(function.complexity);
        values
            .entry(MetricId::FunctionNestingDepth)
            .or_default()
            .push(function.nesting_depth);
        values
            .entry(MetricId::FunctionParameterCount)
            .or_default()
            .push(function.parameter_count);
    }

    for type_metric in &raw_metrics.types {
        values
            .entry(MetricId::TypeLoc)
            .or_default()
            .push(type_metric.loc);
        values
            .entry(MetricId::TypeMemberCount)
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
    percentile_values: &BTreeMap<MetricId, Vec<usize>>,
) -> Option<f64> {
    let values = percentile_values.get(&metric.name)?;
    if values.len() < PERCENTILE_MIN_SAMPLE {
        return None;
    }

    let rank = values.partition_point(|value| *value <= metric.value);
    Some((rank as f64 / values.len() as f64).clamp(0.0, 1.0))
}

fn normalized_metric_value(metric: &FindingMetric, threshold_normalized: f64) -> f64 {
    let Some(percentile) = metric.percentile else {
        return threshold_normalized;
    };

    threshold_normalized.max(percentile).clamp(0.0, 1.0)
}

pub(crate) fn summarize_raw_metrics(raw_metrics: &RawMetrics) -> MetricsSummary {
    MetricsSummary {
        directories: directory_percentiles(raw_metrics),
        files: file_percentiles(raw_metrics),
        functions: function_percentiles(raw_metrics),
        types: type_percentiles(raw_metrics),
        churn: churn_percentiles(raw_metrics),
    }
}

fn directory_percentiles(raw_metrics: &RawMetrics) -> BTreeMap<String, MetricPercentiles> {
    percentile_map([(
        "source_files",
        raw_metrics
            .directories
            .iter()
            .map(|metric| metric.source_files)
            .collect(),
    )])
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

#[cfg(test)]
mod contract_tests {
    use super::*;

    #[test]
    #[should_panic(expected = "outside its detector contract")]
    fn rejects_metrics_not_declared_by_detector() {
        let _ = Finding::from(FindingInput::new(
            FindingKind::LargeFile,
            "src/lib.rs",
            Some(1),
            "",
            vec![FindingMetric::threshold(MetricId::GroupSize, 3, 2, "items")],
        ));
    }

    #[test]
    fn normalization_uses_the_stronger_lens_without_adding_duplicate_evidence() {
        let mut metric = FindingMetric::threshold(MetricId::FileLoc, 900, 800, "lines");
        metric.percentile = Some(0.95);

        assert_eq!(normalized_metric_value(&metric, 0.40), 0.95);
    }
}
