fn parallel_implementation_findings(
    functions: &[FunctionSignal],
    _options: &ConceptDriftOptions,
) -> Vec<Finding> {
    let threshold = 3;
    let mut groups = BTreeMap::<String, Vec<Occurrence>>::new();

    for function in functions.iter().filter(|function| !function.is_test) {
        if !function
            .words
            .iter()
            .any(|word| PARALLEL_CAPABILITY_WORDS.contains(&word.as_str()))
        {
            continue;
        }

        let key = concept_key(&function.words, PARALLEL_STOP_WORDS, 4);
        if key.split(' ').count() < 2 {
            continue;
        }
        groups
            .entry(key)
            .or_default()
            .push(function.occurrence.clone());
    }

    groups_to_findings(
        groups,
        OccurrenceGroupSpec {
            threshold,
            kind: FindingKind::ParallelImplementation,
            message: parallel_implementation_message,
            require_cross_file: true,
        },
    )
}

fn shadowed_abstraction_findings(
    functions: &[FunctionSignal],
    _options: &ConceptDriftOptions,
) -> Vec<Finding> {
    let threshold = 3;
    let mut groups = BTreeMap::<String, Vec<Occurrence>>::new();

    for function in functions.iter().filter(|function| !function.is_test) {
        let has_helper_signal = function
            .file_words
            .iter()
            .any(|word| SHADOW_HELPER_WORDS.contains(&word.as_str()));
        if !has_helper_signal {
            continue;
        }

        let key = concept_key(&function.words, SHADOW_STOP_WORDS, 3);
        if key.is_empty() {
            continue;
        }
        groups
            .entry(key)
            .or_default()
            .push(function.occurrence.clone());
    }

    groups_to_findings(
        groups,
        OccurrenceGroupSpec {
            threshold,
            kind: FindingKind::ShadowedAbstraction,
            message: shadowed_abstraction_message,
            require_cross_file: true,
        },
    )
}

fn duplicate_type_shape_findings(
    shapes: &[TypeShape],
    options: &ConceptDriftOptions,
) -> Vec<Finding> {
    let threshold = options.min_data_shape_occurrences.max(2);
    let mut ordered = shapes.to_vec();
    ordered.sort_by(|left, right| {
        left.occurrence
            .path
            .cmp(&right.occurrence.path)
            .then(left.occurrence.line.cmp(&right.occurrence.line))
    });

    let mut used = vec![false; ordered.len()];
    let mut findings = Vec::new();

    for index in 0..ordered.len() {
        if used[index] {
            continue;
        }

        let group = similar_shape_group(&ordered, &used, index);

        let unique_files = group
            .iter()
            .map(|shape| shape.occurrence.path.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        if group.len() < threshold || unique_files < 2 {
            continue;
        }

        mark_used_shapes(&ordered, &group, &mut used);
        findings.push(duplicate_shape_finding(&group, threshold));
    }

    findings
}
fn similar_shape_group(ordered: &[TypeShape], used: &[bool], index: usize) -> Vec<TypeShape> {
    let mut group = vec![ordered[index].clone()];
    for candidate_index in index + 1..ordered.len() {
        if !used[candidate_index]
            && field_overlap(&ordered[index].fields, &ordered[candidate_index].fields) >= 0.75
        {
            group.push(ordered[candidate_index].clone());
        }
    }
    group
}

fn mark_used_shapes(ordered: &[TypeShape], group: &[TypeShape], used: &mut [bool]) {
    for shape in group {
        if let Some(position) = ordered.iter().position(|item| {
            item.occurrence.path == shape.occurrence.path
                && item.occurrence.line == shape.occurrence.line
        }) {
            used[position] = true;
        }
    }
}

fn duplicate_shape_finding(group: &[TypeShape], threshold: usize) -> Finding {
    let fields = shared_fields(group);
    let representative = &group[0].occurrence;
    crate::scanner::Finding::from(
        FindingInput::new(
            FindingKind::DuplicateTypeShape,
            representative.path.clone(),
            Some(representative.line),
            format!(
                "{} type shapes share fields: {}",
                group.len(),
                fields.into_iter().take(6).collect::<Vec<_>>().join(", ")
            ),
            vec![FindingMetric::threshold(
                crate::model::MetricId::GroupSize,
                group.len(),
                threshold,
                "type shapes",
            )],
        )
        .with_related_locations(
            group
                .iter()
                .map(|shape| related_location(&shape.occurrence))
                .collect(),
        ),
    )
}

fn generic_bucket_findings(
    directories: &BTreeMap<PathBuf, GenericDirectory>,
    generic_files: &[(String, Occurrence)],
    options: &ConceptDriftOptions,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let concept_threshold = (options.max_dir_files / 4).clamp(4, 12);

    for directory in directories.values() {
        if directory.files.len() < 4 || directory.concepts.len() < concept_threshold {
            continue;
        }

        findings.push(crate::scanner::Finding::from(
            FindingInput::new(
                FindingKind::GenericBucketDrift,
                directory.display_path.clone(),
                None,
                format!(
                    "generic bucket mixes {} source concepts across {} files",
                    directory.concepts.len(),
                    directory.files.len()
                ),
                vec![FindingMetric::threshold(
                    crate::model::MetricId::GroupSize,
                    directory.concepts.len(),
                    concept_threshold,
                    "concepts",
                )],
            )
            .with_related_locations(
                directory
                    .files
                    .iter()
                    .map(|path| RelatedLocation {
                        path: path.clone(),
                        line: 1,
                        name: None,
                    })
                    .collect(),
            ),
        ));
    }

    let generic_file_threshold = concept_threshold.max(18);
    for (concepts, occurrence) in generic_files {
        let concept_count = concepts.split(", ").count();
        if concept_count < generic_file_threshold {
            continue;
        }

        findings.push(crate::scanner::Finding::from(
            FindingInput::new(
                FindingKind::GenericBucketDrift,
                occurrence.path.clone(),
                Some(occurrence.line),
                format!("generic file accumulates unrelated concepts: {concepts}"),
                vec![FindingMetric::threshold(
                    crate::model::MetricId::GroupSize,
                    concept_count,
                    generic_file_threshold,
                    "concepts",
                )],
            )
            .with_related_locations(vec![related_location(occurrence)]),
        ));
    }

    findings
}
fn adapter_boundary_bypass_findings(
    bypasses: &BTreeMap<BypassKind, Vec<Occurrence>>,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for (kind, occurrences) in bypasses {
        let mut group = occurrences.clone();
        group.sort_by(|left, right| left.path.cmp(&right.path).then(left.line.cmp(&right.line)));
        group.dedup_by(|left, right| left.path == right.path && left.line == right.line);

        let unique_files = group
            .iter()
            .map(|occurrence| occurrence.path.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        let threshold = 4;
        if group.len() < threshold || unique_files < 3 {
            continue;
        }

        let representative = &group[0];
        findings.push(crate::scanner::Finding::from(
            FindingInput::new(
                FindingKind::AdapterBoundaryBypass,
                representative.path.clone(),
                Some(representative.line),
                format!(
                    "{} direct {} calls bypass existing boundary files",
                    group.len(),
                    kind.label()
                ),
                vec![FindingMetric::threshold(
                    crate::model::MetricId::GroupSize,
                    group.len(),
                    threshold,
                    "bypasses",
                )],
            )
            .with_related_locations(group.iter().map(related_location).collect()),
        ));
    }

    findings
}

fn group_occurrences(
    occurrences: Vec<(String, Occurrence)>,
    spec: OccurrenceGroupSpec,
) -> Vec<Finding> {
    let mut groups = BTreeMap::<String, Vec<Occurrence>>::new();
    for (key, occurrence) in occurrences {
        groups.entry(key).or_default().push(occurrence);
    }

    groups_to_findings(groups, spec)
}

fn groups_to_findings(
    groups: BTreeMap<String, Vec<Occurrence>>,
    spec: OccurrenceGroupSpec,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for (key, mut group) in groups {
        group.sort_by(|left, right| left.path.cmp(&right.path).then(left.line.cmp(&right.line)));
        group.dedup_by(|left, right| {
            left.path == right.path && left.line == right.line && left.name == right.name
        });

        let unique_files = group
            .iter()
            .map(|occurrence| occurrence.path.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        if group.len() < spec.threshold || (spec.require_cross_file && unique_files < 2) {
            continue;
        }

        let representative = &group[0];
        findings.push(crate::scanner::Finding::from(
            FindingInput::new(
                spec.kind,
                representative.path.clone(),
                Some(representative.line),
                (spec.message)(&key, group.len()),
                vec![FindingMetric::threshold(
                    crate::model::MetricId::GroupSize,
                    group.len(),
                    spec.threshold,
                    "occurrences",
                )],
            )
            .with_related_locations(group.iter().map(related_location).collect()),
        ));
    }

    findings
}
