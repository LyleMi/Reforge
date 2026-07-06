use super::*;

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
    risks
        .into_iter()
        .fold(MIN_SCORE, f64::max)
        .clamp(MIN_SCORE, MAX_SCORE)
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

    normalized_threshold_excess(value as f64 / threshold as f64) * MAX_SCORE
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
    .clamp(MIN_SCORE, MAX_SCORE) as u8;
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
