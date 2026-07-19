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
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.path.cmp(&right.path))
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
    members.sort_by(|left, right| {
        findings[*right]
            .priority
            .cmp(&findings[*left].priority)
            .then_with(|| findings[*left].id.cmp(&findings[*right].id))
    });
    let primary = &findings[members[0]];
    let construct = primary.construct;
    let mechanism = primary.mechanism;
    let action = action(primary.kind);
    let path = primary.path.clone();
    let line = primary.line;
    let primary_finding_id = primary.id.clone();
    let priority_factors = primary.priority_factors.clone();
    let detection_reliability = priority_factors.detection_reliability;
    let interpretation_reliability = priority_factors.interpretation_reliability;
    let priority = primary.priority;
    let severity = primary.severity;
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
        priority,
        severity,
        priority_factors,
        subject,
        detection_reliability,
        interpretation_reliability,
    }
}

fn issue_summary(family: &str, subject: &EvidenceSubject) -> String {
    format!("{} at {}", family.replace('_', " "), subject.identity())
}

#[cfg(test)]
mod tests {
    use crate::model::{FindingKind, RelatedLocation};
    use crate::scoring::FindingInput;

    use super::*;

    fn sample(kind: FindingKind, line: usize, priority: u8) -> Finding {
        let mut finding = Finding::from(FindingInput::new(
            kind,
            "src/lib.rs",
            Some(line),
            "",
            Vec::new(),
        ));
        finding.priority = priority;
        finding
    }

    fn group_sample(kind: FindingKind, path: &str, related_paths: &[&str]) -> Finding {
        Finding::from(
            FindingInput::new(kind, path, Some(1), "", Vec::new()).with_related_locations(
                related_paths
                    .iter()
                    .map(|path| RelatedLocation {
                        path: (*path).to_string(),
                        line: 1,
                        name: None,
                    })
                    .collect(),
            ),
        )
    }

    #[test]
    fn groups_readability_facets_at_the_same_function() {
        let mut findings = vec![
            sample(FindingKind::ComplexFunction, 10, 60),
            sample(FindingKind::DeepNesting, 10, 45),
            sample(FindingKind::LongFunction, 20, 50),
        ];

        let clusters = cluster_findings(&mut findings);

        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0].finding_ids.len(), 2);
        assert_eq!(
            clusters[0].action,
            crate::model::RefactorAction::SimplifyFunction
        );
        assert_eq!(clusters[0].primary_finding_id, findings[0].id);
        assert_eq!(findings[0].issue_id, findings[1].issue_id);
        assert!(findings[2].issue_id.is_some());
    }

    #[test]
    fn does_not_cluster_unrelated_detectors_at_the_same_location() {
        let mut findings = vec![
            sample(FindingKind::SimilarFunctions, 10, 60),
            sample(FindingKind::RepeatedErrorPattern, 10, 50),
        ];

        let clusters = cluster_findings(&mut findings);

        assert_eq!(clusters.len(), 2);
        assert!(findings.iter().all(|finding| finding.issue_id.is_some()));
    }

    #[test]
    fn issue_inherits_the_complete_primary_score_without_factor_splicing() {
        let mut stronger = sample(FindingKind::LongFunction, 12, 80);
        stronger.priority_factors.impact = 91.0;
        stronger.priority_factors.intensity = 40.0;
        let expected = stronger.priority_factors.clone();
        let mut weaker = sample(FindingKind::ComplexFunction, 12, 30);
        weaker.priority_factors.intensity = 99.0;
        let mut findings = vec![weaker, stronger];
        let issues = cluster_findings(&mut findings);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].priority, 80);
        assert_eq!(issues[0].priority_factors, expected);
    }

    #[test]
    fn complete_link_clustering_prevents_transitive_bridge_merges() {
        let mut findings = vec![
            group_sample(FindingKind::SimilarFunctions, "src/a.rs", &["src/b.rs"]),
            group_sample(
                FindingKind::ParallelImplementation,
                "src/b.rs",
                &["src/c.rs"],
            ),
            group_sample(FindingKind::ShadowedAbstraction, "src/c.rs", &["src/d.rs"]),
        ];

        let clusters = cluster_findings(&mut findings);

        assert_eq!(clusters.len(), 3);
        assert!(clusters.iter().all(|issue| issue.finding_ids.len() == 1));
        assert_eq!(
            findings
                .iter()
                .filter(|finding| finding.issue_id.is_none())
                .count(),
            0
        );
    }

    #[test]
    fn clustering_is_stable_for_every_input_permutation() {
        let base = [
            group_sample(FindingKind::SimilarFunctions, "src/a.rs", &["src/b.rs"]),
            group_sample(
                FindingKind::ParallelImplementation,
                "src/b.rs",
                &["src/c.rs"],
            ),
            group_sample(FindingKind::ShadowedAbstraction, "src/c.rs", &["src/d.rs"]),
        ];
        let permutations = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        let mut expected = None;

        for permutation in permutations {
            let mut findings = permutation
                .into_iter()
                .map(|index| base[index].clone())
                .collect::<Vec<_>>();
            let clusters = cluster_findings(&mut findings);
            let snapshot = clusters
                .iter()
                .map(|cluster| {
                    (
                        cluster.id.clone(),
                        cluster.primary_finding_id.clone(),
                        cluster.finding_ids.clone(),
                    )
                })
                .collect::<Vec<_>>();

            if let Some(expected) = &expected {
                assert_eq!(&snapshot, expected);
            } else {
                expected = Some(snapshot);
            }
        }
    }

    #[test]
    fn issue_key_depends_only_on_family_and_subject() {
        let subject = EvidenceSubject::Function {
            path: "src/lib.rs".into(),
            line: 10,
        };
        assert_eq!(
            IssueKey::from_family_and_subject("readability", &subject),
            IssueKey::from_family_and_subject("readability", &subject)
        );
    }
}
