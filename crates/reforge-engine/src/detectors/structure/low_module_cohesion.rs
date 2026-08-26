use crate::detectors::concepts::identifier_concepts;

const GENERIC_CLUSTER_LABELS: &[&str] = &["code", "line"];

struct CohesionEvidenceData<'analysis, 'metric> {
    candidates: &'analysis [&'metric FunctionMetric],
    indices_by_name: &'analysis BTreeMap<&'metric str, Vec<usize>>,
    concepts: &'analysis [BTreeSet<String>],
    clusters: &'analysis [Vec<usize>],
    clustered_percent: usize,
}

fn scan_low_module_cohesion(
    file: &SourceFile,
    family: LanguageFamily,
    functions: &[FunctionMetric],
    options: &StructureOptions,
    signals: &mut FileSignals,
) {
    if family != LanguageFamily::JavaScriptTypeScript {
        return;
    }
    let mut candidates = functions
        .iter()
        .filter(|function| function.is_module_scope && !function.is_anonymous)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|function| (function.line, function.name.as_str()));
    if candidates.len() < options.min_module_functions {
        return;
    }
    let (indices_by_name, concepts) = index_candidate_concepts(&candidates);
    let clusters = strong_responsibility_clusters(&candidates, &indices_by_name, &concepts);
    if clusters.len() < 2 {
        return;
    }
    let clustered_count = clusters.iter().map(Vec::len).sum::<usize>();
    let clustered_percent = clustered_count.saturating_mul(100) / candidates.len();
    if clustered_percent < options.min_clustered_function_percent {
        return;
    }

    signals.detections.push(low_module_cohesion_evidence(file, options, CohesionEvidenceData {
        candidates: &candidates,
        indices_by_name: &indices_by_name,
        concepts: &concepts,
        clusters: &clusters,
        clustered_percent,
    }));
}

fn index_candidate_concepts<'a>(
    candidates: &[&'a FunctionMetric],
) -> (BTreeMap<&'a str, Vec<usize>>, Vec<BTreeSet<String>>) {
    let mut indices_by_name = BTreeMap::<&str, Vec<usize>>::new();
    let concepts = candidates
        .iter()
        .enumerate()
        .map(|(index, function)| {
            indices_by_name
                .entry(function.name.as_str())
                .or_default()
                .push(index);
            identifier_concepts(&function.name)
        })
        .collect();
    (indices_by_name, concepts)
}

fn strong_responsibility_clusters(
    candidates: &[&FunctionMetric],
    indices_by_name: &BTreeMap<&str, Vec<usize>>,
    concepts: &[BTreeSet<String>],
) -> Vec<Vec<usize>> {
    let mut parents = (0..candidates.len()).collect::<Vec<_>>();
    for (caller_index, caller) in candidates.iter().enumerate() {
        for callee_name in &caller.direct_calls {
            let Some(callee_indices) = indices_by_name
                .get(callee_name.as_str())
                .filter(|indices| indices.len() == 1)
                .filter(|_| !caller.shadowed_call_names.contains(callee_name))
            else {
                continue;
            };
            let callee_index = callee_indices[0];
            if caller_index != callee_index
                && !concepts[caller_index].is_disjoint(&concepts[callee_index])
            {
                union_components(&mut parents, caller_index, callee_index);
            }
        }
    }
    let mut grouped = BTreeMap::<usize, Vec<usize>>::new();
    for index in 0..candidates.len() {
        let root = find_component(&mut parents, index);
        grouped.entry(root).or_default().push(index);
    }
    let mut clusters = grouped
        .into_values()
        .filter(|members| members.len() >= 3)
        .collect::<Vec<_>>();
    clusters.sort_by_key(|members| candidates[members[0]].line);
    clusters
}

fn low_module_cohesion_evidence(
    file: &SourceFile,
    options: &StructureOptions,
    data: CohesionEvidenceData<'_, '_>,
) -> DetectedEvidence {
    let summaries = data
        .clusters
        .iter()
        .map(|members| {
            let label = responsibility_cluster_label(
                members,
                data.candidates,
                data.concepts,
                data.indices_by_name,
            );
            format!(
                "{label}: `{}` (+{} functions)",
                data.candidates[members[0]].name,
                members.len() - 1
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let related_locations = data
        .clusters
        .iter()
        .flatten()
        .map(|index| RelatedLocation {
            path: file.display_path.clone(),
            line: data.candidates[*index].line,
            name: Some(data.candidates[*index].name.clone()),
            entity_key: None,
        })
        .collect();
    DetectedEvidence::from(
        DetectedEvidenceInput::new(
            Rule::LowModuleCohesion,
            file.display_path.clone(),
            Some(1),
            format!(
                "file contains {} responsibility clusters across {clustered_percent}% of {} module functions: {summaries}",
                data.clusters.len(),
                data.candidates.len(),
                clustered_percent = data.clustered_percent,
            ),
            vec![
                DetectedMeasurement::threshold(
                    MetricId::FileModuleFunctionCount,
                    data.candidates.len(),
                    options.min_module_functions,
                    "functions",
                ),
                DetectedMeasurement::measurement(
                    MetricId::FileResponsibilityClusterCount,
                    data.clusters.len(),
                    "clusters",
                ),
                DetectedMeasurement::threshold(
                    MetricId::FileClusteredFunctionPercent,
                    data.clustered_percent,
                    options.min_clustered_function_percent,
                    "percent",
                ),
            ],
        )
        .with_related_locations(related_locations)
        .with_file_subject(),
    )
}

fn find_component(parents: &mut [usize], index: usize) -> usize {
    if parents[index] != index {
        parents[index] = find_component(parents, parents[index]);
    }
    parents[index]
}

fn union_components(parents: &mut [usize], left: usize, right: usize) {
    let left = find_component(parents, left);
    let right = find_component(parents, right);
    if left != right {
        let (first, second) = if left < right { (left, right) } else { (right, left) };
        parents[second] = first;
    }
}

fn responsibility_cluster_label(
    members: &[usize],
    candidates: &[&FunctionMetric],
    concepts: &[BTreeSet<String>],
    indices_by_name: &BTreeMap<&str, Vec<usize>>,
) -> String {
    let mut counts = BTreeMap::<&str, usize>::new();
    let member_set = members.iter().copied().collect::<BTreeSet<_>>();
    for caller in members {
        for callee_name in &candidates[*caller].direct_calls {
            if candidates[*caller]
                .shadowed_call_names
                .contains(callee_name)
            {
                continue;
            }
            let Some(callee) = indices_by_name
                .get(callee_name.as_str())
                .filter(|indices| indices.len() == 1)
                .map(|indices| indices[0])
                .filter(|index| member_set.contains(index))
            else {
                continue;
            };
            for concept in concepts[*caller].intersection(&concepts[callee]) {
                *counts.entry(concept).or_default() += 1;
            }
        }
    }
    counts
        .iter()
        .filter(|(word, _)| !GENERIC_CLUSTER_LABELS.contains(word))
        .max_by(|(left_word, left_count), (right_word, right_count)| {
            left_count
                .cmp(right_count)
                .then_with(|| right_word.cmp(left_word))
        })
        .or_else(|| counts.first_key_value())
        .map(|(word, _)| (*word).to_string())
        .unwrap_or_else(|| "module".into())
}
