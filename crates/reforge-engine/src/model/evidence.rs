use super::*;

/// Detector output before it is projected into the public report schema.
///
/// Identity belongs to the schema projection. Internally we retain only the
/// semantic anchor needed to derive that identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectedEvidence {
    pub semantic_anchor: String,
    pub kind: Rule,
    pub path: String,
    pub line: Option<usize>,
    pub metrics: Vec<DetectedMeasurement>,
    pub message: String,
    pub related_locations: Vec<RelatedLocation>,
    pub flow_witness: Option<FlowWitness>,
}

impl DetectedEvidence {
    pub fn normalize_flow_anchor(&mut self) {
        if let Some(witness) = &self.flow_witness {
            self.semantic_anchor = format!(
                "flow:{}:{}:{}",
                witness.policy, witness.source.id, witness.sink.id
            );
        }
    }
}

pub fn serialized_rule(kind: Rule) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(ToString::to_string))
        .unwrap_or_else(|| format!("{kind:?}"))
}
