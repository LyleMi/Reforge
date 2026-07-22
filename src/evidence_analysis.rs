use std::collections::BTreeMap;

use crate::detectors::manifest::{classification, input_metrics};
use crate::model::{
    Finding, FindingKind, FindingMetric, MetricId, MetricPercentiles, MetricsSummary, RawMetrics,
    RelatedLocation,
};

#[path = "evidence_analysis/clusters.rs"]
mod clusters;

pub(crate) use clusters::cluster_findings;

const PERCENTILE_MIN_SAMPLE: usize = 5;

#[derive(Debug, Clone)]
pub struct FindingInput {
    kind: FindingKind,
    path: String,
    line: Option<usize>,
    message: String,
    metrics: Vec<FindingMetric>,
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
            related_locations: Vec::new(),
        }
    }

    pub fn with_related_locations(mut self, related_locations: Vec<RelatedLocation>) -> Self {
        self.related_locations = related_locations;
        self
    }
}

impl From<FindingInput> for Finding {
    fn from(input: FindingInput) -> Self {
        let (construct, mechanism) = classification(input.kind);
        let mut finding = Finding {
            id: Default::default(),
            anchor: format!("path:{}", crate::pathing::normalize_path_text(&input.path)),
            kind: input.kind,
            path: input.path,
            line: input.line,
            metrics: input.metrics,
            construct,
            mechanism,
            issue_id: None,
            message: input.message,
            related_locations: input.related_locations,
            flow_witness: None,
        };
        finding.refresh_id();
        finding
    }
}

pub(crate) fn finalize_metric_context(findings: &mut [Finding], raw_metrics: &RawMetrics) {
    let percentile_values = percentile_metric_values(raw_metrics);

    for finding in findings {
        for metric in &mut finding.metrics {
            let threshold_normalized = metric
                .excess_ratio
                .map(normalized_threshold_excess)
                .unwrap_or(0.20);
            metric.percentile = metric_percentile(metric, &percentile_values);
            metric.normalized = Some(normalized_metric_value(metric, threshold_normalized));
        }
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
