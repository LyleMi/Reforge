use std::collections::BTreeMap;
use std::io::{self, Write};

use serde_json::{Value, json};

use crate::model::{Finding, Issue, ScanReport};

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
    let rule_indices = rule_indices(&report.issues);
    let mut rules = rule_indices
        .iter()
        .map(|(family, index)| (*index, sarif_rule(family)))
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
            "results": report.issues
                .iter()
                .map(|issue| sarif_result(issue, &report.findings, &rule_indices))
                .collect::<Vec<_>>(),
            "properties": {
                "reforgeSchemaVersion": report.schema_version,
                "coverage": {
                    "cells": report.coverage_manifest,
                    "summary": report.coverage_summary
                },
            }
        }]
    })
}

fn rule_indices(issues: &[Issue]) -> BTreeMap<String, usize> {
    let mut families = issues
        .iter()
        .map(|issue| issue.family.clone())
        .collect::<Vec<_>>();
    families.sort();
    families.dedup();

    families
        .into_iter()
        .enumerate()
        .map(|(index, family)| (family, index))
        .collect()
}

fn sarif_rule(id: &str) -> Value {
    json!({
        "id": id,
        "name": title_label(id),
        "shortDescription": {
            "text": title_label(id)
        },
        "properties": {
            "kind": id
        }
    })
}

fn sarif_result(
    issue: &Issue,
    findings: &[Finding],
    rule_indices: &BTreeMap<String, usize>,
) -> Value {
    let witness_finding = findings
        .iter()
        .find(|finding| finding.id == issue.primary_finding_id);
    let related_locations = witness_finding
        .into_iter()
        .flat_map(|finding| finding.related_locations.iter())
        .enumerate()
        .map(|(index, location)| {
            json!({
                "id": index + 1,
                "message": { "text": location.name },
                "physicalLocation": physical_location(&location.path, Some(location.line))
            })
        })
        .collect::<Vec<_>>();
    json!({
        "ruleId": issue.family,
        "ruleIndex": rule_indices.get(&issue.family).copied().unwrap_or(0),
        "level": "note",
        "message": { "text": issue.summary },
        "locations": [{
            "physicalLocation": physical_location(&issue.path, issue.line)
        }],
        "relatedLocations": related_locations,
        "partialFingerprints": {
            "reforgeIssueId": issue.id
        },
        "properties": {
            "id": issue.id,
            "family": issue.family,
            "construct": issue.construct,
            "mechanism": issue.mechanism,
            "action": issue.action,
            "evidence_ids": issue.finding_ids,
            "flow_witness": witness_finding.and_then(|finding| finding.flow_witness.as_ref())
        }
    })
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
