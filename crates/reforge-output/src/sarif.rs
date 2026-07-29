use reforge_schema::Report;

pub(super) fn sarif(report: &Report) -> serde_json::Value {
    let rule_ids = report.provenance.rules.keys().map(String::as_str);
    serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": report.producer.name,
                "version": report.producer.version,
                "rules": rule_ids.map(|rule| {
                    let provenance = report.provenance.rules.get(rule);
                    serde_json::json!({
                        "id": rule,
                        "name": rule.rsplit('.').next().unwrap_or(rule),
                        "properties": {
                            "semanticVersion": provenance.map(|value| value.semantic_version.as_str()),
                            "evaluationDigest": provenance.map(|value| value.evaluation_digest.as_str())
                        }
                    })
                }).collect::<Vec<_>>()
            } },
            "results": report.issues.iter().map(|issue| {
                let location = issue.evidence.iter().flat_map(|evidence| &evidence.locations).next();
                let rule = issue.evidence.first().map(|evidence| evidence.rule.as_str()).unwrap_or(&issue.family);
                let baseline = report.baseline_comparison.as_ref().and_then(|comparison| comparison.issues.get(&issue.id));
                let code_flows = issue.evidence.iter().filter_map(|evidence| evidence.witness.as_ref()).map(|witness| {
                    serde_json::json!({
                        "threadFlows": [{
                            "locations": witness.ordered_steps.iter().map(|step| serde_json::json!({
                                "location": {
                                    "physicalLocation": {
                                        "artifactLocation": { "uri": step.path },
                                        "region": { "startLine": step.line.unwrap_or(1) }
                                    },
                                    "message": { "text": step.operation }
                                }
                            })).collect::<Vec<_>>()
                        }]
                    })
                }).collect::<Vec<_>>();
                let mut properties = serde_json::json!({
                    "kind": format!("{:?}", issue.kind).to_ascii_lowercase()
                });
                if let Some(reason) = baseline.and_then(|entry| entry.reason.as_deref()) {
                    properties["baselineReason"] = reason.into();
                }
                let mut result = serde_json::json!({
                    "ruleId": rule,
                    "level": match issue.kind {
                        reforge_schema::IssueKind::Advisory => "note",
                        reforge_schema::IssueKind::Policy => "warning",
                    },
                    "message": { "text": issue.title },
                    "partialFingerprints": {
                        "reforgeIssueId/v7": issue.id,
                        "reforgeContent/v7": issue.content_fingerprint
                    },
                    "properties": properties,
                    "locations": location.map(|location| vec![serde_json::json!({
                        "physicalLocation": {
                            "artifactLocation": { "uri": location.path },
                            "region": { "startLine": location.line.unwrap_or(1) }
                        }
                    })]).unwrap_or_default(),
                    "codeFlows": code_flows
                });
                if let Some(state) = baseline.and_then(|entry| match entry.state {
                    reforge_schema::BaselineState::New => Some("new"),
                    reforge_schema::BaselineState::Unchanged => Some("unchanged"),
                    reforge_schema::BaselineState::Updated => Some("updated"),
                    reforge_schema::BaselineState::Absent => Some("absent"),
                    reforge_schema::BaselineState::Unknown => None,
                }) {
                    result["baselineState"] = state.into();
                }
                result
            }).collect::<Vec<_>>()
        }]
    })
}
