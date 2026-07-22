fn lineage_candidates(
    current: &ScanReport,
    previous: &ScanReport,
    finding_diff: &BaselineDifferenceSet,
    issue_diff: &BaselineDifferenceSet,
) -> Vec<LineageCandidate> {
    let removed_finding_ids = change_ids(&finding_diff.removed);
    let added_finding_ids = change_ids(&finding_diff.added);
    let mut finding_candidates = Vec::new();
    for old in previous
        .findings
        .iter()
        .filter(|item| removed_finding_ids.contains(item.id.as_str()))
    {
        for new in current
            .findings
            .iter()
            .filter(|item| added_finding_ids.contains(item.id.as_str()) && item.kind == old.kind)
        {
            if let Some((confidence, reasons)) = finding_lineage_score(old, new) {
                finding_candidates.push(candidate(
                    LineageEntity::Finding,
                    old.id.as_str(),
                    new.id.as_str(),
                    confidence,
                    reasons,
                ));
            }
        }
    }

    let removed_issue_ids = change_ids(&issue_diff.removed);
    let added_issue_ids = change_ids(&issue_diff.added);
    let finding_scores = lineage_score_map(&finding_candidates);
    let mut issue_candidates = Vec::new();
    for old in previous
        .issues
        .iter()
        .filter(|item| removed_issue_ids.contains(item.id.as_str()))
    {
        for new in current
            .issues
            .iter()
            .filter(|item| added_issue_ids.contains(item.id.as_str()) && item.family == old.family)
        {
            let scores = old
                .finding_ids
                .iter()
                .flat_map(|old_id| {
                    new.finding_ids.iter().filter_map(|new_id| {
                        finding_scores
                            .get(&(old_id.as_str(), new_id.as_str()))
                            .copied()
                    })
                })
                .collect::<Vec<_>>();
            if let Some(confidence) = scores.iter().max().copied() {
                issue_candidates.push(candidate(
                    LineageEntity::Issue,
                    old.id.as_str(),
                    new.id.as_str(),
                    confidence,
                    vec![
                        format!("{} supporting finding match(es)", scores.len()),
                        "same issue family".into(),
                    ],
                ));
            }
        }
    }
    finding_candidates.extend(issue_candidates);
    finding_candidates.sort_by(|left, right| left.id.cmp(&right.id));
    finding_candidates
}

fn change_ids(changes: &[BaselineChange]) -> BTreeSet<&str> {
    changes.iter().map(|item| item.id.as_str()).collect()
}

fn lineage_score_map(candidates: &[LineageCandidate]) -> BTreeMap<(&str, &str), u8> {
    candidates
        .iter()
        .map(|item| {
            (
                (item.previous_id.as_str(), item.current_id.as_str()),
                item.confidence_percent,
            )
        })
        .collect()
}

fn finding_lineage_score(old: &Finding, new: &Finding) -> Option<(u8, Vec<String>)> {
    let old_paths = finding_paths(old);
    let new_paths = finding_paths(new);
    let path_overlap = !old_paths.is_disjoint(&new_paths);
    let old_symbols = finding_symbols(old);
    let new_symbols = finding_symbols(new);
    let symbol_overlap = !old_symbols.is_disjoint(&new_symbols);
    if !path_overlap && !symbol_overlap {
        return None;
    }
    let mut score = 0u8;
    let mut reasons = Vec::new();
    if path_overlap {
        score += 35;
        reasons.push("overlapping paths".into());
    }
    if symbol_overlap {
        score += 35;
        reasons.push("overlapping symbols".into());
    }
    let old_metrics = old
        .metrics
        .iter()
        .map(|metric| metric.name)
        .collect::<BTreeSet<_>>();
    let new_metrics = new
        .metrics
        .iter()
        .map(|metric| metric.name)
        .collect::<BTreeSet<_>>();
    if !old_metrics.is_disjoint(&new_metrics) {
        score += 20;
        reasons.push("overlapping metric names".into());
    }
    if Path::new(&old.path).file_name() == Path::new(&new.path).file_name() {
        score += 40;
        reasons.push("same primary basename".into());
    }
    (score >= 60).then_some((score.min(100), reasons))
}

fn finding_paths(finding: &Finding) -> BTreeSet<String> {
    std::iter::once(finding.path.replace('\\', "/"))
        .chain(
            finding
                .related_locations
                .iter()
                .map(|location| location.path.replace('\\', "/")),
        )
        .collect()
}

fn finding_symbols(finding: &Finding) -> BTreeSet<String> {
    let mut symbols = finding
        .related_locations
        .iter()
        .filter_map(|location| location.name.clone())
        .collect::<BTreeSet<_>>();
    if let Some(symbol) = finding
        .anchor
        .rsplit_once("::")
        .map(|(_, symbol)| symbol.trim_end_matches(|character: char| character == '#' || character.is_ascii_digit()))
        .filter(|symbol| !symbol.is_empty())
    {
        symbols.insert(symbol.to_string());
    }
    if let Some(witness) = &finding.flow_witness {
        symbols.insert(witness.source.name.clone());
        symbols.insert(witness.sink.name.clone());
    }
    symbols
}

fn candidate(
    entity: LineageEntity,
    previous_id: &str,
    current_id: &str,
    confidence_percent: u8,
    reasons: Vec<String>,
) -> LineageCandidate {
    let key = serde_json::json!({"algorithm": "lineage-v1", "entity": entity, "previous": previous_id, "current": current_id});
    let digest = fingerprint_json(&key);
    LineageCandidate {
        id: format!("rl1-{}", &digest[7..23]),
        entity,
        previous_id: previous_id.into(),
        current_id: current_id.into(),
        confidence_percent,
        reasons,
    }
}
