use crate::model::{
    AgentEvidence, AgentEvidenceCoverageStatus, AgentTestReachability, EvidenceDispersion,
    FileAgentEvidence, Issue, IssueAgentEvidence, IssueKey,
};

const TEST_PATH_LIMIT: usize = 20;

fn build_agent_evidence(scan: &SourceScan, issues: &[Issue]) -> AgentEvidence {
    let context = AgentEvidenceContext::new(scan);
    let files = context.file_evidence(scan);
    let file_evidence = files
        .iter()
        .map(|evidence| (evidence.path.clone(), evidence))
        .collect::<BTreeMap<_, _>>();
    let finding_by_id = findings_by_id(&scan.findings);
    let issues = issues
        .iter()
        .map(|issue| context.issue_evidence(issue, &finding_by_id, &file_evidence))
        .collect();

    AgentEvidence { files, issues }
}

struct AgentEvidenceContext {
    file_loc: BTreeMap<String, usize>,
    test_files: BTreeSet<String>,
    languages: BTreeMap<String, String>,
    adjacency: BTreeMap<String, BTreeSet<String>>,
    closure_by_file: BTreeMap<String, BTreeSet<String>>,
}

impl AgentEvidenceContext {
    fn new(scan: &SourceScan) -> Self {
        let file_loc = file_locations(scan);
        Self {
            test_files: test_files(scan),
            languages: source_languages(scan),
            adjacency: dependency_adjacency(&scan.dependency_graph),
            closure_by_file: direct_context_closures(&scan.dependency_graph, file_loc.keys()),
            file_loc,
        }
    }

    fn file_evidence(&self, scan: &SourceScan) -> Vec<FileAgentEvidence> {
        self.file_loc
            .keys()
            .map(|path| self.file_agent_evidence(path, scan))
            .collect()
    }

    fn file_agent_evidence(&self, path: &str, scan: &SourceScan) -> FileAgentEvidence {
        let closure = self.file_closure(path);
        let unresolved = scan
            .unresolved_dependency_edges_by_file
            .get(path)
            .copied()
            .unwrap_or(0);

        FileAgentEvidence {
            path: path.to_string(),
            coverage_status: file_coverage_status(path, self.test_files.contains(path), unresolved),
            context_closure_files: closure.len(),
            context_closure_loc: closure
                .iter()
                .filter_map(|path| self.file_loc.get(path))
                .sum(),
            unresolved_local_dependencies: unresolved,
            test_reachability: self.test_reachability(path),
        }
    }

    fn issue_evidence(
        &self,
        issue: &Issue,
        finding_by_id: &BTreeMap<&str, &Finding>,
        file_evidence: &BTreeMap<String, &FileAgentEvidence>,
    ) -> IssueAgentEvidence {
        let evidence_files = issue_evidence_files(issue, finding_by_id);
        let mut accumulator = IssueEvidenceAccumulator::default();
        for path in &evidence_files {
            accumulator.add_path(path, self);
            if let Some(evidence) = file_evidence.get(path) {
                accumulator.add_file_evidence(evidence);
            }
        }
        accumulator.into_agent_evidence(issue.id.clone(), evidence_files, self)
    }

    fn file_closure(&self, path: &str) -> BTreeSet<String> {
        self.closure_by_file
            .get(path)
            .cloned()
            .unwrap_or_else(|| BTreeSet::from([path.to_string()]))
    }

    fn test_reachability(&self, path: &str) -> AgentTestReachability {
        if self.test_files.contains(path) {
            return AgentTestReachability::default();
        }

        let distances = shortest_test_distances(path, &self.adjacency, &self.test_files);
        let nearest_test_distance = distances.values().copied().min();
        let nearest_test_paths = nearest_paths(&distances, nearest_test_distance);

        AgentTestReachability {
            direct_test_files: self.direct_test_files(path),
            reachable_test_files: sorted_limited_paths(distances.keys().cloned()),
            reachable_test_file_count: distances.len(),
            nearest_test_distance,
            paths_truncated: nearest_test_paths.len() > TEST_PATH_LIMIT,
            nearest_test_paths: limit_paths(nearest_test_paths),
        }
    }

    fn direct_test_files(&self, path: &str) -> Vec<String> {
        self.adjacency
            .get(path)
            .into_iter()
            .flatten()
            .filter(|candidate| self.test_files.contains(*candidate))
            .cloned()
            .collect()
    }
}

#[derive(Debug, Default)]
struct IssueEvidenceAccumulator {
    closure: BTreeSet<String>,
    unresolved: usize,
    reachable_tests: BTreeSet<String>,
    direct_tests: BTreeSet<String>,
    nearest: Option<usize>,
    nearest_paths: BTreeSet<String>,
    statuses: BTreeSet<AgentEvidenceCoverageStatus>,
}

impl IssueEvidenceAccumulator {
    fn add_path(&mut self, path: &str, context: &AgentEvidenceContext) {
        if let Some(file_closure) = context.closure_by_file.get(path) {
            self.closure.extend(file_closure.iter().cloned());
        }
    }

    fn add_file_evidence(&mut self, evidence: &FileAgentEvidence) {
        self.statuses.insert(evidence.coverage_status);
        self.unresolved += evidence.unresolved_local_dependencies;
        self.direct_tests
            .extend(evidence.test_reachability.direct_test_files.iter().cloned());
        self.reachable_tests.extend(
            evidence
                .test_reachability
                .reachable_test_files
                .iter()
                .cloned(),
        );
        self.add_nearest_tests(evidence);
    }

    fn add_nearest_tests(&mut self, evidence: &FileAgentEvidence) {
        let Some(distance) = evidence.test_reachability.nearest_test_distance else {
            return;
        };
        if self.nearest.is_none_or(|current| distance < current) {
            self.nearest = Some(distance);
            self.nearest_paths.clear();
        }
        if self.nearest == Some(distance) {
            self.nearest_paths.extend(
                evidence
                    .test_reachability
                    .nearest_test_paths
                    .iter()
                    .cloned(),
            );
        }
    }

    fn into_agent_evidence(
        self,
        issue_id: IssueKey,
        evidence_files: Vec<String>,
        context: &AgentEvidenceContext,
    ) -> IssueAgentEvidence {
        let nearest_paths_len = self.nearest_paths.len();
        IssueAgentEvidence {
            issue_id,
            coverage_status: combined_coverage_status(&self.statuses),
            evidence_dispersion: evidence_dispersion(&evidence_files, &context.languages),
            context_closure_files: self.closure.len(),
            context_closure_loc: self
                .closure
                .iter()
                .filter_map(|path| context.file_loc.get(path))
                .sum(),
            unresolved_local_dependencies: self.unresolved,
            test_reachability: AgentTestReachability {
                direct_test_files: self.direct_tests.into_iter().collect(),
                reachable_test_files: sorted_limited_paths(
                    self.reachable_tests.clone().into_iter(),
                ),
                reachable_test_file_count: self.reachable_tests.len(),
                nearest_test_distance: self.nearest,
                paths_truncated: nearest_paths_len > TEST_PATH_LIMIT,
                nearest_test_paths: sorted_limited_paths(self.nearest_paths.into_iter()),
            },
        }
    }
}

fn file_locations(scan: &SourceScan) -> BTreeMap<String, usize> {
    scan.raw_metrics
        .files
        .iter()
        .map(|file| (file.path.clone(), file.loc))
        .collect()
}

fn test_files(scan: &SourceScan) -> BTreeSet<String> {
    scan.raw_metrics
        .files
        .iter()
        .filter(|file| file.is_test)
        .map(|file| file.path.clone())
        .collect()
}

fn source_languages(scan: &SourceScan) -> BTreeMap<String, String> {
    scan.structure_sources
        .iter()
        .map(|source| {
            (
                source.display_path.clone(),
                language_label(&source.display_path).to_string(),
            )
        })
        .collect()
}

fn findings_by_id(findings: &[Finding]) -> BTreeMap<&str, &Finding> {
    findings
        .iter()
        .map(|finding| (finding.id.as_str(), finding))
        .collect()
}

fn issue_evidence_files(issue: &Issue, finding_by_id: &BTreeMap<&str, &Finding>) -> Vec<String> {
    let mut evidence_files = BTreeSet::new();
    for id in &issue.finding_ids {
        if let Some(finding) = finding_by_id.get(id.as_str()) {
            evidence_files.insert(finding.path.clone());
            evidence_files.extend(
                finding
                    .related_locations
                    .iter()
                    .map(|location| location.path.clone()),
            );
        }
    }
    evidence_files.into_iter().collect()
}

fn evidence_dispersion(
    evidence_files: &[String],
    languages: &BTreeMap<String, String>,
) -> EvidenceDispersion {
    EvidenceDispersion {
        evidence_files: evidence_files.to_vec(),
        evidence_directories: evidence_directories(evidence_files),
        evidence_languages: evidence_files
            .iter()
            .filter_map(|path| languages.get(path).cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    }
}

fn evidence_directories(evidence_files: &[String]) -> Vec<String> {
    evidence_files
        .iter()
        .filter_map(|path| Path::new(path).parent())
        .map(|path| {
            let value = path.to_string_lossy();
            if value.is_empty() {
                ".".to_string()
            } else {
                value.to_string()
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn dependency_adjacency(graph: &DependencyGraphSnapshot) -> BTreeMap<String, BTreeSet<String>> {
    let mut adjacency = BTreeMap::<String, BTreeSet<String>>::new();
    for node in &graph.nodes {
        adjacency.entry(node.path.clone()).or_default();
    }
    for edge in &graph.edges {
        adjacency
            .entry(edge.from.clone())
            .or_default()
            .insert(edge.to.clone());
        adjacency
            .entry(edge.to.clone())
            .or_default()
            .insert(edge.from.clone());
    }
    adjacency
}

fn direct_context_closures<'a>(
    graph: &DependencyGraphSnapshot,
    file_paths: impl Iterator<Item = &'a String>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut closures = file_paths
        .map(|path| (path.clone(), BTreeSet::from([path.clone()])))
        .collect::<BTreeMap<String, BTreeSet<String>>>();
    for edge in &graph.edges {
        closures
            .entry(edge.from.clone())
            .or_default()
            .insert(edge.to.clone());
        closures
            .entry(edge.to.clone())
            .or_default()
            .insert(edge.from.clone());
    }
    closures
}

fn shortest_test_distances(
    path: &str,
    adjacency: &BTreeMap<String, BTreeSet<String>>,
    test_files: &BTreeSet<String>,
) -> BTreeMap<String, usize> {
    let mut queue = std::collections::VecDeque::from([(path.to_string(), 0)]);
    let mut seen = BTreeSet::from([path.to_string()]);
    let mut distances = BTreeMap::new();
    while let Some((current, distance)) = queue.pop_front() {
        for next in adjacency.get(&current).into_iter().flatten() {
            if !seen.insert(next.clone()) {
                continue;
            }
            let next_distance = distance + 1;
            if test_files.contains(next) {
                distances.insert(next.clone(), next_distance);
            }
            queue.push_back((next.clone(), next_distance));
        }
    }
    distances
}

fn nearest_paths(distances: &BTreeMap<String, usize>, nearest: Option<usize>) -> Vec<String> {
    let mut paths = distances
        .iter()
        .filter(|(_, distance)| Some(**distance) == nearest)
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn sorted_limited_paths(paths: impl Iterator<Item = String>) -> Vec<String> {
    let mut paths = paths.collect::<Vec<_>>();
    paths.sort();
    limit_paths(paths)
}

fn limit_paths(mut paths: Vec<String>) -> Vec<String> {
    paths.truncate(TEST_PATH_LIMIT);
    paths
}

fn file_coverage_status(
    path: &str,
    is_test: bool,
    unresolved: usize,
) -> AgentEvidenceCoverageStatus {
    if is_test {
        return AgentEvidenceCoverageStatus::NotApplicable;
    }
    if !dependency_language_supported(path) {
        return AgentEvidenceCoverageStatus::Unsupported;
    }
    if unresolved > 0 {
        return AgentEvidenceCoverageStatus::Partial;
    }
    AgentEvidenceCoverageStatus::Observed
}

fn combined_coverage_status(
    statuses: &BTreeSet<AgentEvidenceCoverageStatus>,
) -> AgentEvidenceCoverageStatus {
    if statuses.contains(&AgentEvidenceCoverageStatus::Partial) {
        AgentEvidenceCoverageStatus::Partial
    } else if statuses.contains(&AgentEvidenceCoverageStatus::Unsupported) {
        AgentEvidenceCoverageStatus::Unsupported
    } else if statuses.contains(&AgentEvidenceCoverageStatus::Observed) {
        AgentEvidenceCoverageStatus::Observed
    } else {
        AgentEvidenceCoverageStatus::NotApplicable
    }
}

fn dependency_language_supported(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str()),
        Some(
            "rs" | "js"
                | "jsx"
                | "mjs"
                | "cjs"
                | "ts"
                | "tsx"
                | "mts"
                | "cts"
                | "vue"
                | "py"
                | "rb"
                | "c"
                | "cc"
                | "cpp"
                | "cs"
                | "csx"
        )
    )
}

fn language_label(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("rs") => "rust",
        Some("js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" | "vue") => {
            "javascript_typescript"
        }
        Some("py") => "python",
        Some("go") => "go",
        Some("java") => "java",
        Some("cs" | "csx") => "csharp",
        Some("kt") => "kotlin",
        Some("php") => "php",
        Some("rb") => "ruby",
        Some("sh" | "bash") => "bash",
        Some("ps1" | "psm1") => "powershell",
        Some("c" | "cc" | "cpp") => "c_like",
        _ => "unknown",
    }
}
