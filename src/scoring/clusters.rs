use std::collections::BTreeSet;

use crate::detectors::manifest::{action, parent_kind, relations};
use crate::model::{Finding, FindingKind, IssueCluster, SignalMechanism};

pub(crate) fn cluster_findings(findings: &mut [Finding]) -> Vec<IssueCluster> {
    let candidate_indices = findings
        .iter()
        .enumerate()
        .filter_map(|(index, finding)| is_cluster_candidate(finding).then_some(index))
        .collect::<Vec<_>>();
    let mut components = Vec::<Vec<usize>>::new();

    for finding_index in candidate_indices {
        if let Some(component) = components.iter_mut().find(|component| {
            component
                .iter()
                .all(|member| findings_overlap(&findings[finding_index], &findings[*member]))
        }) {
            component.push(finding_index);
        } else {
            components.push(vec![finding_index]);
        }
    }

    let mut clusters = components
        .into_iter()
        .filter(|members| members.len() > 1)
        .map(|members| build_cluster(findings, members))
        .collect::<Vec<_>>();
    clusters.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.line.cmp(&right.line))
    });
    clusters
}

fn is_cluster_candidate(finding: &Finding) -> bool {
    parent_kind(finding.kind).is_some()
        || !relations(finding.kind).is_empty()
        || matches!(
            finding.kind,
            FindingKind::ReadabilityRisk | FindingKind::MissingDocumentationSet
        )
}

fn kinds_related(left: FindingKind, right: FindingKind) -> bool {
    if left == right {
        return false;
    }

    if parent_kind(left) == Some(right) || parent_kind(right) == Some(left) {
        return true;
    }
    if parent_kind(left).is_some() && parent_kind(left) == parent_kind(right) {
        return true;
    }

    relations(left)
        .iter()
        .any(|relation| relation.kind == right)
        || relations(right)
            .iter()
            .any(|relation| relation.kind == left)
}

fn findings_overlap(left: &Finding, right: &Finding) -> bool {
    if left.construct != right.construct
        || left.mechanism != right.mechanism
        || action(left.kind) != action(right.kind)
        || !kinds_related(left.kind, right.kind)
    {
        return false;
    }
    if left.path == right.path && left.line == right.line {
        return true;
    }
    if left.mechanism == SignalMechanism::CognitiveLoad {
        return false;
    }

    let left_paths = evidence_paths(left);
    evidence_paths(right)
        .iter()
        .any(|path| left_paths.contains(path))
}

fn evidence_paths(finding: &Finding) -> BTreeSet<&str> {
    std::iter::once(finding.path.as_str())
        .chain(
            finding
                .related_locations
                .iter()
                .map(|location| location.path.as_str()),
        )
        .collect()
}

fn build_cluster(findings: &mut [Finding], mut members: Vec<usize>) -> IssueCluster {
    members.sort_by(|left, right| {
        findings[*right]
            .priority
            .cmp(&findings[*left].priority)
            .then_with(|| findings[*left].id.cmp(&findings[*right].id))
    });
    let primary = &findings[members[0]];
    let id = format!("ic1-{}", primary.id.trim_start_matches("rf1-"));
    let construct = primary.construct;
    let mechanism = primary.mechanism;
    let action = action(primary.kind);
    let path = primary.path.clone();
    let line = primary.line;
    let primary_finding_id = primary.id.clone();
    let priority = primary.priority;
    let severity = primary.severity;
    let mut finding_ids = members
        .iter()
        .map(|index| findings[*index].id.clone())
        .collect::<Vec<_>>();
    finding_ids.sort();
    let mut kinds = members
        .iter()
        .map(|index| findings[*index].kind)
        .collect::<Vec<_>>();
    kinds.sort();
    kinds.dedup();

    for index in members {
        findings[index].issue_cluster_id = Some(id.clone());
    }

    IssueCluster {
        id,
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
    }
}

#[cfg(test)]
mod tests {
    use crate::model::RelatedLocation;
    use crate::scoring::{FindingInput, finding};

    use super::*;

    fn sample(kind: FindingKind, line: usize, priority: u8) -> Finding {
        let mut finding = finding(FindingInput::new(
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
        finding(
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

        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].finding_ids.len(), 2);
        assert_eq!(
            clusters[0].action,
            crate::model::RefactorAction::SimplifyFunction
        );
        assert_eq!(clusters[0].primary_finding_id, findings[0].id);
        assert_eq!(findings[0].issue_cluster_id, findings[1].issue_cluster_id);
        assert!(findings[2].issue_cluster_id.is_none());
    }

    #[test]
    fn does_not_cluster_unrelated_detectors_at_the_same_location() {
        let mut findings = vec![
            sample(FindingKind::SimilarFunctions, 10, 60),
            sample(FindingKind::RepeatedErrorPattern, 10, 50),
        ];

        let clusters = cluster_findings(&mut findings);

        assert!(clusters.is_empty());
        assert!(
            findings
                .iter()
                .all(|finding| finding.issue_cluster_id.is_none())
        );
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

        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].finding_ids.len(), 2);
        assert!(findings[2].issue_cluster_id.is_none());
    }
}
