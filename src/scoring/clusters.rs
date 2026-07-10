use std::collections::BTreeSet;

use crate::model::{Finding, FindingKind, IssueCluster, SignalMechanism};

pub(crate) fn cluster_findings(findings: &mut [Finding]) -> Vec<IssueCluster> {
    let candidate_indices = findings
        .iter()
        .enumerate()
        .filter_map(|(index, finding)| is_cluster_candidate(finding).then_some(index))
        .collect::<Vec<_>>();
    let mut parents = (0..candidate_indices.len()).collect::<Vec<_>>();

    for left in 0..candidate_indices.len() {
        for right in (left + 1)..candidate_indices.len() {
            if findings_overlap(
                &findings[candidate_indices[left]],
                &findings[candidate_indices[right]],
            ) {
                union(&mut parents, left, right);
            }
        }
    }

    let mut components = std::collections::BTreeMap::<usize, Vec<usize>>::new();
    for (candidate, finding_index) in candidate_indices.into_iter().enumerate() {
        let root = find(&mut parents, candidate);
        components.entry(root).or_default().push(finding_index);
    }

    let mut clusters = components
        .into_values()
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
    match finding.mechanism {
        SignalMechanism::CognitiveLoad | SignalMechanism::DuplicationDivergence => true,
        SignalMechanism::KnowledgeDrift => is_documentation_finding(finding.kind),
        _ => false,
    }
}

fn is_documentation_finding(kind: FindingKind) -> bool {
    matches!(
        kind,
        FindingKind::MissingDocumentationSet
            | FindingKind::MissingUserGuide
            | FindingKind::MissingReportSchemaDocs
            | FindingKind::MissingMetricsModelDocs
            | FindingKind::MissingArchitectureDocs
            | FindingKind::StaleCliDocumentation
            | FindingKind::StaleSchemaDocumentation
    )
}

fn findings_overlap(left: &Finding, right: &Finding) -> bool {
    if left.construct != right.construct || left.mechanism != right.mechanism {
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
        path,
        line,
        primary_finding_id,
        finding_ids,
        kinds,
        priority,
        severity,
    }
}

fn find(parents: &mut [usize], value: usize) -> usize {
    if parents[value] != value {
        parents[value] = find(parents, parents[value]);
    }
    parents[value]
}

fn union(parents: &mut [usize], left: usize, right: usize) {
    let left_root = find(parents, left);
    let right_root = find(parents, right);
    if left_root != right_root {
        parents[right_root] = left_root;
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(clusters[0].primary_finding_id, findings[0].id);
        assert_eq!(findings[0].issue_cluster_id, findings[1].issue_cluster_id);
        assert!(findings[2].issue_cluster_id.is_none());
    }
}
