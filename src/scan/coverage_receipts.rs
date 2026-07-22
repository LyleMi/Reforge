fn detector_execution_receipts(context: &CoverageContext<'_, '_>) -> Vec<DetectorExecutionReceipt> {
    context
        .input
        .manifest
        .iter()
        .map(|entry| detector_execution_receipt(entry, context))
        .collect()
}

fn detector_execution_receipt(
    entry: &crate::model::DetectorManifestEntry,
    context: &CoverageContext<'_, '_>,
) -> DetectorExecutionReceipt {
    if entry.kind == FindingKind::AdapterFlowBypass
        && context.input.flow_analysis.status == crate::model::FlowAnalysisStatus::Disabled
    {
        return DetectorExecutionReceipt {
            kind: entry.kind,
            status: DetectorExecutionStatus::NotApplicable,
            observations: Vec::new(),
            candidate_groups_before_threshold: 0,
            raw_emitted: 0,
            cli_filtered: 0,
            suppression_removed: 0,
            final_findings: 0,
            unobservable_count: 0,
            unobservable_reasons: vec!["data-flow mode is off".into()],
        };
    }
    let applicable = detector_runtime_applicable(entry, context);
    let parse_sensitive = detector_requires_parse(entry);
    let source_sensitive = !is_unity_detector(entry.kind);
    let unresolved = detector_unresolved_count(entry, context);
    let unobservable_count = if applicable {
        usize::from(parse_sensitive) * context.input.parse_failures.len()
            + usize::from(source_sensitive) * context.input.source_failures.len()
            + unresolved
    } else {
        0
    };
    DetectorExecutionReceipt {
        kind: entry.kind,
        status: if !applicable {
            DetectorExecutionStatus::NotApplicable
        } else if unobservable_count > 0 {
            DetectorExecutionStatus::PartiallyObserved
        } else {
            DetectorExecutionStatus::Completed
        },
        observations: detector_observations(entry, context, applicable),
        candidate_groups_before_threshold: if entry.entity_scope == crate::model::EntityScope::FindingGroup {
            context.input.emitted_by_kind.get(&entry.kind).copied().unwrap_or(0)
        } else { 0 },
        raw_emitted: context.input.emitted_by_kind.get(&entry.kind).copied().unwrap_or(0),
        cli_filtered: context.input.cli_filtered_by_kind.get(&entry.kind).copied().unwrap_or(0),
        suppression_removed: context.input.suppressed_by_kind.get(&entry.kind).copied().unwrap_or(0),
        final_findings: context.input.findings.iter().filter(|finding| finding.kind == entry.kind).count(),
        unobservable_count,
        unobservable_reasons: detector_unobservable_reasons(
            entry,
            context,
            applicable,
        ),
    }
}

fn detector_observations(
    manifest_entry: &crate::model::DetectorManifestEntry,
    coverage_context: &CoverageContext<'_, '_>,
    is_applicable: bool,
) -> Vec<crate::model::DetectorObservation> {
    if !is_applicable {
        return Vec::new();
    }
    if manifest_entry.kind == FindingKind::AdapterFlowBypass {
        return flow_observations(coverage_context);
    }
    if manifest_entry.kind == FindingKind::SimilarFunctions {
        return similarity_observations(coverage_context.input.similarity_comparisons);
    }
    if is_unity_detector(manifest_entry.kind) {
        return unity_observations(coverage_context);
    }
    if manifest_entry.approach == crate::model::DetectionApproach::GraphAnalysis {
        return dependency_graph_observations(coverage_context);
    }
    vec![entity_observation(
        manifest_entry,
        coverage_context,
        is_applicable,
    )]
}

fn flow_observations(
    context: &CoverageContext<'_, '_>,
) -> Vec<crate::model::DetectorObservation> {
    vec![
            crate::model::DetectorObservation {
                stage: "flow_analysis".into(),
                unit: "flow_function".into(),
                count: context.input.flow_analysis.functions_analyzed,
            },
            crate::model::DetectorObservation {
                stage: "path_composition".into(),
                unit: "flow_path".into(),
                count: context.input.flow_analysis.exact_edges,
            },
        ]
}

fn similarity_observations(
    stats: &crate::similar_functions::SimilarityComparisonStats,
) -> Vec<crate::model::DetectorObservation> {
    vec![
            crate::model::DetectorObservation {
                stage: "candidate_pairs".into(),
                unit: "function_pair".into(),
                count: stats.total_candidate_pairs,
            },
            crate::model::DetectorObservation {
                stage: "indexed_candidate_pairs".into(),
                unit: "function_pair".into(),
                count: stats.indexed_candidate_pairs,
            },
            crate::model::DetectorObservation {
                stage: "multiset_pruned_pairs".into(),
                unit: "function_pair".into(),
                count: stats.multiset_pruned_pairs,
            },
            crate::model::DetectorObservation {
                stage: "lcs_comparisons".into(),
                unit: "function_pair".into(),
                count: stats.lcs_comparisons,
            },
        ]
}

fn unity_observations(
    context: &CoverageContext<'_, '_>,
) -> Vec<crate::model::DetectorObservation> {
    vec![
            crate::model::DetectorObservation {
                stage: "unity_inventory".into(),
                unit: "unity_asset".into(),
                count: context.input.unity_assets,
            },
            crate::model::DetectorObservation {
                stage: "unity_inventory".into(),
                unit: "unity_assembly".into(),
                count: context.input.unity_assemblies,
            },
        ]
}

fn dependency_graph_observations(
    context: &CoverageContext<'_, '_>,
) -> Vec<crate::model::DetectorObservation> {
    vec![
            crate::model::DetectorObservation {
                stage: "graph_analysis".into(),
                unit: "dependency_node".into(),
                count: context.input.dependency_nodes,
            },
            crate::model::DetectorObservation {
                stage: "graph_analysis".into(),
                unit: "dependency_edge".into(),
                count: context.input.dependency_edges,
            },
        ]
}

fn entity_observation(
    detector: &crate::model::DetectorManifestEntry,
    coverage: &CoverageContext<'_, '_>,
    runtime_applicable: bool,
) -> crate::model::DetectorObservation {
    let count = analyzed_entity_count(detector, coverage, runtime_applicable);
    let unit = match detector.entity_scope {
        crate::model::EntityScope::Repository => "repository",
        crate::model::EntityScope::Directory => "directory",
        crate::model::EntityScope::File => "file",
        crate::model::EntityScope::Function => "function",
        crate::model::EntityScope::Type => "type",
        crate::model::EntityScope::FindingGroup => "finding_group",
    };
    crate::model::DetectorObservation { stage: "detector_input".into(), unit: unit.into(), count }
}

fn is_unity_detector(kind: FindingKind) -> bool {
    matches!(
        kind,
        FindingKind::UnityAssemblyCycle
            | FindingKind::UnityAssemblyHub
            | FindingKind::UnityUnresolvedAssemblyReference
            | FindingKind::UnityRuntimeEditorDependency
            | FindingKind::UnityDuplicateGuid
            | FindingKind::UnityMissingMeta
            | FindingKind::UnityOrphanMeta
            | FindingKind::UnityBrokenAssetReference
            | FindingKind::UnityMissingScript
            | FindingKind::UnityNonTextSerialization
            | FindingKind::UnitySceneBuildDrift
            | FindingKind::UnityLargeScene
            | FindingKind::UnityLargePrefab
            | FindingKind::UnitySerializedFieldBloat
            | FindingKind::UnityLifecycleOverload
            | FindingKind::UnityExpensiveFrameCall
            | FindingKind::UnityEditorApiInRuntime
            | FindingKind::UnityUnbalancedEventSubscription
    )
}

fn analyzed_entity_count(
    entry: &crate::model::DetectorManifestEntry,
    context: &CoverageContext<'_, '_>,
    applicable: bool,
) -> usize {
    if entry.kind == FindingKind::AdapterFlowBypass {
        return context.input.flow_analysis.functions_analyzed;
    }
    if !applicable {
        return 0;
    }
    context
        .entity_counts
        .get(&entry.entity_scope)
        .copied()
        .unwrap_or_else(|| finding_group_count(entry, context))
}

fn finding_group_count(
    entry: &crate::model::DetectorManifestEntry,
    context: &CoverageContext<'_, '_>,
) -> usize {
    if entry.entity_scope == crate::model::EntityScope::FindingGroup {
        context
            .input
            .emitted_by_kind
            .get(&entry.kind)
            .copied()
            .unwrap_or(0)
    } else {
        0
    }
}

fn detector_unresolved_count(
    entry: &crate::model::DetectorManifestEntry,
    context: &CoverageContext<'_, '_>,
) -> usize {
    if entry.kind == FindingKind::AdapterFlowBypass {
        context.input.flow_analysis.unresolved_edges + context.input.flow_analysis.truncated_paths
    } else if entry.approach == crate::model::DetectionApproach::GraphAnalysis {
        context.input.unresolved_dependency_edges
    } else {
        0
    }
}

fn detector_unobservable_reasons(
    detector: &crate::model::DetectorManifestEntry,
    coverage: &CoverageContext<'_, '_>,
    runtime_applicable: bool,
) -> Vec<String> {
    if !runtime_applicable {
        return Vec::new();
    }
    let parse_sensitive = detector_requires_parse(detector);
    let source_sensitive = !is_unity_detector(detector.kind);
    let unresolved = detector_unresolved_count(detector, coverage);
    [
        (!coverage.input.parse_failures.is_empty() && parse_sensitive).then(|| {
            format!(
                "{} source files failed syntax parsing",
                coverage.input.parse_failures.len()
            )
        }),
        (!coverage.input.source_failures.is_empty() && source_sensitive).then(|| {
            format!(
                "{} source files could not be decoded or read",
                coverage.input.source_failures.len()
            )
        }),
        (unresolved > 0).then(|| {
            if detector.kind == FindingKind::AdapterFlowBypass {
                format!("{unresolved} data-flow edges or bounded paths were unresolved")
            } else {
                format!("{unresolved} dependency edges could not be resolved")
            }
        }),
    ]
    .into_iter()
    .flatten()
    .collect()
}
