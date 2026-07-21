pub(crate) fn conflict_graph(
    report: &ScanReport,
    investigations: &[InvestigationArtifact],
) -> Vec<ConflictEdge> {
    let surfaces = investigations
        .iter()
        .map(|investigation| {
            (
                investigation.issue_id.clone(),
                issue_surface(report, investigation),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let ids = surfaces.keys().cloned().collect::<Vec<_>>();
    ids.iter()
        .enumerate()
        .flat_map(|(index, left_id)| {
            ids.iter().skip(index + 1).filter_map(|right_id| {
                conflict_edge(
                    left_id,
                    &surfaces[left_id],
                    right_id,
                    &surfaces[right_id],
                )
            })
        })
        .collect()
}

fn conflict_edge(
    left_id: &IssueKey,
    left: &IssueSurface,
    right_id: &IssueKey,
    right: &IssueSurface,
) -> Option<ConflictEdge> {
    let comparisons = [
        ("shared_write_file", &left.write_set, &right.write_set),
        ("shared_evidence_file", &left.evidence, &right.evidence),
        (
            "shared_dependency_boundary",
            &left.dependencies,
            &right.dependencies,
        ),
        ("shared_test", &left.tests, &right.tests),
        (
            "shared_unity_surface",
            &left.unity_surfaces,
            &right.unity_surfaces,
        ),
    ];
    let reasons = comparisons
        .into_iter()
        .filter(|(_, left, right)| !left.is_disjoint(right))
        .map(|(reason, _, _)| reason.to_string())
        .collect::<Vec<_>>();
    (!reasons.is_empty()).then(|| ConflictEdge {
        left_issue_id: left_id.clone(),
        right_issue_id: right_id.clone(),
        reasons,
    })
}

#[derive(Debug, Default)]
struct IssueSurface {
    write_set: BTreeSet<String>,
    evidence: BTreeSet<String>,
    dependencies: BTreeSet<String>,
    tests: BTreeSet<String>,
    unity_surfaces: BTreeSet<String>,
}

fn issue_surface(report: &ScanReport, investigation: &InvestigationArtifact) -> IssueSurface {
    let mut surface = IssueSurface {
        write_set: investigation.write_set.iter().cloned().collect(),
        evidence: investigation
            .inspected_files
            .iter()
            .chain(&investigation.read_set)
            .cloned()
            .collect(),
        ..IssueSurface::default()
    };
    add_issue_evidence(report, investigation, &mut surface);
    add_test_evidence(report, investigation, &mut surface);
    add_dependency_boundaries(report, &mut surface);
    add_unity_surfaces(&mut surface);
    surface
}

fn add_issue_evidence(
    report: &ScanReport,
    investigation: &InvestigationArtifact,
    surface: &mut IssueSurface,
) {
    let Some(issue) = report
        .issues
        .iter()
        .find(|issue| issue.id == investigation.issue_id)
    else {
        return;
    };
    surface.evidence.insert(issue.path.clone());
    for finding in report
        .findings
        .iter()
        .filter(|finding| issue.finding_ids.contains(&finding.id))
    {
        surface.evidence.insert(finding.path.clone());
        surface.evidence.extend(
            finding
                .related_locations
                .iter()
                .map(|location| location.path.clone()),
        );
    }
}

fn add_test_evidence(
    report: &ScanReport,
    investigation: &InvestigationArtifact,
    surface: &mut IssueSurface,
) {
    let Some(agent) = report
        .agent_evidence
        .issues
        .iter()
        .find(|item| item.issue_id == investigation.issue_id)
    else {
        return;
    };
    surface
        .tests
        .extend(agent.test_reachability.direct_test_files.iter().cloned());
    surface
        .tests
        .extend(agent.test_reachability.reachable_test_files.iter().cloned());
    surface
        .tests
        .extend(agent.test_reachability.nearest_test_paths.iter().cloned());
}

fn add_dependency_boundaries(report: &ScanReport, surface: &mut IssueSurface) {
    for edge in &report.dependency_graph.edges {
        let touches_surface = [&edge.from, &edge.to]
            .into_iter()
            .any(|path| surface.evidence.contains(path) || surface.write_set.contains(path));
        if touches_surface {
            surface.dependencies.insert(edge.from.clone());
            surface.dependencies.insert(edge.to.clone());
        }
    }
}

fn add_unity_surfaces(surface: &mut IssueSurface) {
    surface.unity_surfaces = surface
        .evidence
        .iter()
        .chain(&surface.write_set)
        .filter(|path| {
            let lower = path.to_ascii_lowercase();
            lower.ends_with(".asmdef") || lower.ends_with(".meta")
        })
        .map(|path| {
            Path::new(path)
                .parent()
                .map(portable_path)
                .unwrap_or_else(|| path.clone())
        })
        .collect();
}
