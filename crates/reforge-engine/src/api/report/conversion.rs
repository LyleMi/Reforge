use super::coverage::language_for_path;
use super::*;

pub(super) fn selected_detections<'a>(
    run: &'a RunResult,
    config: &Config,
) -> BTreeMap<String, &'a DetectedEvidence> {
    run.detected_evidence
        .iter()
        .filter(|detection| {
            if !owner_selected(&config.enabled, detection.kind) {
                return false;
            }
            let rule = crate::detectors::manifest::rule_registry()
                .iter()
                .find(|entry| entry.kind == detection.kind)
                .expect("detected rule is registered");
            config.rule_enabled(&rule.rule, rule.default_enabled)
        })
        .map(|detection| {
            (
                format!(
                    "{}\0{}",
                    unified_rule(detection.kind),
                    detection.semantic_anchor
                ),
                detection,
            )
        })
        .collect()
}

pub(super) fn aggregate_issues(
    detections: &BTreeMap<String, &DetectedEvidence>,
    config: &Config,
) -> Vec<Issue> {
    let mut grouped = BTreeMap::<
        (reforge_schema::IssueKind, String, Subject),
        (String, IssueFamily, Vec<Evidence>),
    >::new();

    for detection in detections.values().copied() {
        let definition = rule_definition(detection.kind);
        let family = definition.family;
        let subject = candidate_subject(detection).canonicalized();
        let kind = if config.rule_enforced(&definition.rule) {
            reforge_schema::IssueKind::Policy
        } else {
            reforge_schema::IssueKind::Advisory
        };
        grouped
            .entry((kind, family, subject))
            .or_insert_with(|| {
                (
                    crate::detectors::manifest::analysis_name(detection.kind).into(),
                    definition.issue_family,
                    Vec::new(),
                )
            })
            .2
            .push(convert_evidence(detection));
    }

    grouped
        .into_iter()
        .map(
            |((kind, family, subject), (analysis, issue_family, evidence))| {
                let title = format!("{}: {}", issue_family.title(), subject.display_name());
                Issue::new(reforge_schema::IssueInput {
                    kind,
                    analysis,
                    family,
                    subject,
                    title,
                    guidance: issue_family.guidance().into(),
                    evidence,
                })
            },
        )
        .collect()
}

fn candidate_subject(detection: &DetectedEvidence) -> Subject {
    match &detection.subject {
        crate::model::DetectedSubject::Repository => Subject::Repository,
        crate::model::DetectedSubject::Directory => Subject::Directory {
            entity: reforge_schema::EntityRef::new(
                detection.semantic_anchor.clone(),
                detection.path.clone(),
                None,
            ),
        },
        crate::model::DetectedSubject::File => Subject::File {
            entity: reforge_schema::EntityRef::new(
                detection.semantic_anchor.clone(),
                detection.path.clone(),
                None,
            ),
        },
        crate::model::DetectedSubject::Symbol { name, .. } => Subject::Symbol {
            entity: reforge_schema::EntityRef::new(
                detection.semantic_anchor.clone(),
                detection.path.clone(),
                Some(name.clone()),
            ),
        },
        crate::model::DetectedSubject::Group => {
            let mut members = detection
                .related_locations
                .iter()
                .map(|location| {
                    reforge_schema::EntityRef::new(
                        location.entity_key.clone().unwrap_or_else(|| {
                            format!(
                                "{}:{}:{}",
                                language_for_path(&location.path).unwrap_or("unknown"),
                                location.path,
                                location.name.as_deref().unwrap_or("member")
                            )
                        }),
                        location.path.clone(),
                        location.name.clone(),
                    )
                })
                .collect::<Vec<_>>();
            if members.is_empty() {
                members.push(reforge_schema::EntityRef::new(
                    detection.semantic_anchor.clone(),
                    detection.path.clone(),
                    None,
                ));
            }
            Subject::Group { members }
        }
    }
}

fn convert_evidence(detection: &DetectedEvidence) -> Evidence {
    let rule = unified_rule(detection.kind);
    let mut evidence = Evidence::new(rule, &detection.semantic_anchor, &detection.message);
    evidence.measurements = detection
        .metrics
        .iter()
        .map(|metric| Measurement {
            name: metric.name.to_string(),
            value: metric.value as f64,
            threshold: metric.threshold.map(|value| value as f64),
            unit: metric.unit.clone(),
        })
        .collect();
    evidence.locations.push(Location {
        path: detection.path.clone(),
        line: detection.line,
        symbol: None,
    });
    evidence
        .locations
        .extend(detection.related_locations.iter().map(|location| Location {
            path: location.path.clone(),
            line: Some(location.line),
            symbol: location.name.clone(),
        }));
    evidence.witness = detection.flow_witness.as_ref().map(convert_flow_witness);
    evidence
}

fn convert_flow_witness(witness: &crate::model::FlowWitness) -> reforge_schema::FlowWitness {
    reforge_schema::FlowWitness {
        source: convert_endpoint(&witness.source),
        sink: convert_endpoint(&witness.sink),
        ordered_steps: witness
            .ordered_steps
            .iter()
            .map(|step| reforge_schema::FlowStep {
                path: step.path.clone(),
                symbol: step.name.clone(),
                line: Some(step.line),
                operation: enum_name(&step.kind),
                resolution: convert_resolution(step.resolution),
            })
            .collect(),
        function_hops: witness.function_hops,
        module_hops: witness.module_hops,
        resolution: convert_resolution(witness.resolution),
    }
}

fn convert_endpoint(location: &crate::model::FlowLocation) -> reforge_schema::FlowEndpoint {
    reforge_schema::FlowEndpoint {
        path: location.path.clone(),
        symbol: human_flow_symbol(&location.function, &location.name),
        language: location.language.clone(),
        line: Some(location.line),
    }
}

fn human_flow_symbol(function: &str, name: &str) -> String {
    if !function.is_empty() {
        function.to_owned()
    } else if !name.is_empty() && !name.starts_with("flow:") {
        name.to_owned()
    } else {
        "source".into()
    }
}

fn convert_resolution(value: crate::model::FlowResolution) -> reforge_schema::FlowResolution {
    match value {
        crate::model::FlowResolution::Exact => reforge_schema::FlowResolution::Exact,
        crate::model::FlowResolution::Modeled => reforge_schema::FlowResolution::Modeled,
        crate::model::FlowResolution::Unresolved => reforge_schema::FlowResolution::Unresolved,
        crate::model::FlowResolution::Unsupported => reforge_schema::FlowResolution::Unsupported,
    }
}
