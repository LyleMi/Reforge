use std::collections::BTreeMap;
use std::io::{self, Write};

use serde_json::{Value, json};

use crate::model::{Finding, FindingKind, ScanReport, Severity, serialized_finding_kind};

pub fn print_sarif_report(report: &ScanReport) -> io::Result<()> {
    write_sarif_report(std::io::stdout().lock(), report)
}

pub fn write_sarif_report(mut writer: impl Write, report: &ScanReport) -> io::Result<()> {
    writer.write_all(render_sarif_report(report).as_bytes())?;
    writer.write_all(b"\n")
}

pub fn render_sarif_report(report: &ScanReport) -> String {
    serde_json::to_string_pretty(&sarif_log(report)).expect("SARIF values should serialize")
}

fn sarif_log(report: &ScanReport) -> Value {
    let rule_indices = rule_indices(&report.findings);
    let mut rules = rule_indices
        .iter()
        .map(|(kind, index)| (*index, sarif_rule(*kind)))
        .collect::<Vec<_>>();
    rules.sort_by_key(|(index, _)| *index);

    json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "Reforge",
                    "informationUri": "https://github.com/openai/reforge",
                    "rules": rules.into_iter().map(|(_, rule)| rule).collect::<Vec<_>>()
                }
            },
            "results": report.findings
                .iter()
                .map(|finding| sarif_result(finding, &rule_indices))
                .collect::<Vec<_>>()
        }]
    })
}

fn rule_indices(findings: &[Finding]) -> BTreeMap<FindingKind, usize> {
    let mut kinds = findings
        .iter()
        .map(|finding| finding.kind)
        .collect::<Vec<_>>();
    kinds.sort_unstable();
    kinds.dedup();

    kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| (kind, index))
        .collect()
}

fn sarif_rule(kind: FindingKind) -> Value {
    let id = serialized_finding_kind(kind);
    json!({
        "id": id,
        "name": title_label(&id),
        "shortDescription": {
            "text": title_label(&id)
        },
        "properties": {
            "kind": id
        }
    })
}

fn sarif_result(finding: &Finding, rule_indices: &BTreeMap<FindingKind, usize>) -> Value {
    let rule_id = serialized_finding_kind(finding.kind);
    let mut result = json!({
        "ruleId": rule_id,
        "ruleIndex": rule_indices.get(&finding.kind).copied().unwrap_or(0),
        "level": sarif_level(finding.severity),
        "message": {
            "text": result_message(finding)
        },
        "locations": [{
            "physicalLocation": physical_location(&finding.path, finding.line)
        }],
        "partialFingerprints": {
            "reforgeFindingId": finding.id
        },
        "properties": {
            "id": finding.id,
            "priority": finding.priority,
            "severity": severity_label(finding.severity),
            "construct": finding.construct,
            "mechanism": finding.mechanism,
            "issue_cluster_id": finding.issue_cluster_id,
            "confidence": finding.confidence,
            "rank_explanation": finding.rank_explanation,
            "recommendation": finding.recommendation()
        }
    });

    let related = related_locations(finding);
    if !related.is_empty()
        && let Some(object) = result.as_object_mut()
    {
        object.insert("relatedLocations".to_string(), Value::Array(related));
    }

    result
}

fn physical_location(path: &str, line: Option<usize>) -> Value {
    json!({
        "artifactLocation": {
            "uri": artifact_uri(path)
        },
        "region": {
            "startLine": line.unwrap_or(1).max(1)
        }
    })
}

fn related_locations(finding: &Finding) -> Vec<Value> {
    finding
        .related_locations
        .iter()
        .enumerate()
        .map(|(index, location)| {
            let mut related = json!({
                "id": index + 1,
                "physicalLocation": physical_location(&location.path, Some(location.line))
            });
            if let Some(name) = &location.name
                && let Some(object) = related.as_object_mut()
            {
                object.insert("message".to_string(), json!({ "text": name }));
            }
            related
        })
        .collect()
}

fn result_message(finding: &Finding) -> String {
    if finding.message.is_empty() {
        title_label(&serialized_finding_kind(finding.kind))
    } else {
        finding.message.clone()
    }
}

fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "critical",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

fn artifact_uri(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let encoded = normalized.replace(' ', "%20");
    if encoded.len() >= 3 && encoded.as_bytes()[1] == b':' && encoded.as_bytes()[2] == b'/' {
        format!("file:///{}", encoded)
    } else {
        encoded
    }
}

fn title_label(value: &str) -> String {
    value
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
