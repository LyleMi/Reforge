use std::collections::BTreeMap;

use crate::detectors::manifest::{action, entity_scope, evidence_role, issue_family};
use crate::model::{
    EntityScope, EvidenceRole, EvidenceSubject, Finding, Issue, IssueKey, RefactorAction,
    SignalMechanism,
};

pub(crate) fn cluster_findings(findings: &mut [Finding]) -> Vec<Issue> {
    let mut groups =
        BTreeMap::<(String, EvidenceSubject, SignalMechanism, RefactorAction), Vec<usize>>::new();
    for (index, finding) in findings.iter().enumerate() {
        if evidence_role(finding.kind) != EvidenceRole::CompositeSummary {
            groups
                .entry((
                    issue_family(finding.kind).into(),
                    subject(finding),
                    finding.mechanism,
                    action(finding.kind),
                ))
                .or_default()
                .push(index);
        }
    }
    let mut clusters = groups
        .into_iter()
        .map(|((family, subject, _, _), members)| {
            build_cluster(findings, &family, subject, members)
        })
        .collect::<Vec<_>>();
    clusters.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.id.cmp(&right.id))
    });
    clusters
}

fn subject(finding: &Finding) -> EvidenceSubject {
    match entity_scope(finding.kind) {
        EntityScope::Repository => EvidenceSubject::Repository,
        EntityScope::Directory => EvidenceSubject::Directory {
            path: finding.path.clone(),
        },
        EntityScope::File => EvidenceSubject::File {
            path: finding.path.clone(),
        },
        EntityScope::Function => EvidenceSubject::Function {
            path: finding.path.clone(),
            line: finding.line.unwrap_or(0),
        },
        EntityScope::Type => EvidenceSubject::Type {
            path: finding.path.clone(),
            line: finding.line.unwrap_or(0),
        },
        EntityScope::FindingGroup => EvidenceSubject::Group {
            locations: std::iter::once(format!("{}:{}", finding.path, finding.line.unwrap_or(0)))
                .chain(
                    finding
                        .related_locations
                        .iter()
                        .map(|location| format!("{}:{}", location.path, location.line)),
                )
                .collect(),
        },
    }
}

fn build_cluster(
    findings: &mut [Finding],
    family: &str,
    subject: EvidenceSubject,
    mut members: Vec<usize>,
) -> Issue {
    members.sort_by(|left, right| findings[*left].id.cmp(&findings[*right].id));
    let primary = &findings[members[0]];
    let construct = primary.construct;
    let mechanism = primary.mechanism;
    let action = action(primary.kind);
    let path = primary.path.clone();
    let line = primary.line;
    let primary_finding_id = primary.id.clone();
    let mut finding_ids = members
        .iter()
        .map(|index| findings[*index].id.clone())
        .collect::<Vec<_>>();
    finding_ids.sort();
    let id = IssueKey::from_family_and_subject(family, &subject);
    let mut kinds = members
        .iter()
        .map(|index| findings[*index].kind)
        .collect::<Vec<_>>();
    kinds.sort();
    kinds.dedup();

    for index in members {
        findings[index].issue_id = Some(id.clone());
    }

    Issue {
        id,
        family: family.to_string(),
        summary: issue_summary(family, &subject),
        construct,
        mechanism,
        action,
        path,
        line,
        primary_finding_id,
        finding_ids,
        kinds,
        subject,
    }
}

fn issue_summary(family: &str, subject: &EvidenceSubject) -> String {
    format!("{} at {}", family.replace('_', " "), subject.identity())
}
