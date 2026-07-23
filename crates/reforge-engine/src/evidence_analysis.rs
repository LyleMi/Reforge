use crate::detectors::manifest::input_metrics;
use crate::model::{DetectedEvidence, DetectedMeasurement, RelatedLocation, Rule};

#[derive(Debug, Clone)]
pub struct DetectedEvidenceInput {
    kind: Rule,
    path: String,
    line: Option<usize>,
    message: String,
    metrics: Vec<DetectedMeasurement>,
    related_locations: Vec<RelatedLocation>,
}

impl DetectedEvidenceInput {
    pub fn new(
        kind: Rule,
        path: impl Into<String>,
        line: Option<usize>,
        message: impl Into<String>,
        metrics: Vec<DetectedMeasurement>,
    ) -> Self {
        let declared_metrics = input_metrics(kind);
        assert!(
            metrics
                .iter()
                .all(|metric| declared_metrics.contains(&metric.name)),
            "detection {kind:?} emitted a metric outside its detector contract"
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

impl From<DetectedEvidenceInput> for DetectedEvidence {
    fn from(input: DetectedEvidenceInput) -> Self {
        DetectedEvidence {
            semantic_anchor: format!("path:{}", crate::pathing::normalize_path_text(&input.path)),
            kind: input.kind,
            path: input.path,
            line: input.line,
            metrics: input.metrics,
            message: input.message,
            related_locations: input.related_locations,
            flow_witness: None,
        }
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use crate::model::MetricId;

    #[test]
    #[should_panic(expected = "outside its detector contract")]
    fn rejects_metrics_not_declared_by_detector() {
        let _ = DetectedEvidence::from(DetectedEvidenceInput::new(
            Rule::LargeFile,
            "src/lib.rs",
            Some(1),
            "",
            vec![DetectedMeasurement::threshold(
                MetricId::GroupSize,
                3,
                2,
                "items",
            )],
        ));
    }
}
