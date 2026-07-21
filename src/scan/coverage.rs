struct CoverageProjectionInput<'a> {
    manifest: &'a [crate::model::DetectorManifestEntry],
    stats: &'a ScanStats,
    source_files: &'a [SourceFile],
    function_count: usize,
    type_count: usize,
    findings: &'a [Finding],
    parse_failures: &'a [ParseFailure],
    unresolved_dependency_edges: usize,
    churn: &'a crate::model::ChurnSummary,
    unity_observed: bool,
}

fn coverage(
    input: CoverageProjectionInput<'_>,
) -> (
    Vec<CoverageManifestEntry>,
    CoverageSummary,
    Vec<DetectorExecutionReceipt>,
    Vec<RawMetricCoverage>,
) {
    let detected_languages = coverage_languages(input.source_files, input.unity_observed);
    let entity_counts = coverage_entity_counts(&input);
    let context = CoverageContext {
        input: &input,
        detected_languages: &detected_languages,
        entity_counts: &entity_counts,
    };
    let coverage_manifest = coverage_manifest_entries(&context);
    let coverage_summary = coverage_summary(&context);
    let detector_execution = detector_execution_receipts(&context);
    let metric_context = RawMetricObservationContext {
        stats: input.stats,
        function_count: input.function_count,
        type_count: input.type_count,
        parse_failures: input.parse_failures,
        churn: input.churn,
    };
    let raw_metric_coverage = canonical_raw_metrics()
        .iter()
        .copied()
        .map(|metric| raw_metric_observation(metric, &metric_context))
        .collect();
    (
        coverage_manifest,
        coverage_summary,
        detector_execution,
        raw_metric_coverage,
    )
}

struct CoverageContext<'a, 'input> {
    input: &'a CoverageProjectionInput<'input>,
    detected_languages: &'a BTreeSet<String>,
    entity_counts: &'a BTreeMap<crate::model::EntityScope, usize>,
}

fn coverage_languages(source_files: &[SourceFile], unity_observed: bool) -> BTreeSet<String> {
    let mut detected_languages = source_files
        .iter()
        .filter_map(|source| detected_language(&source.path))
        .collect::<BTreeSet<_>>();
    if unity_observed {
        detected_languages.insert("unity".to_string());
    }
    detected_languages
}

fn coverage_entity_counts(
    input: &CoverageProjectionInput<'_>,
) -> BTreeMap<crate::model::EntityScope, usize> {
    BTreeMap::from([
        (crate::model::EntityScope::Repository, 1),
        (
            crate::model::EntityScope::Directory,
            input.stats.directories_scanned,
        ),
        (
            crate::model::EntityScope::File,
            input.stats.source_files_scanned,
        ),
        (crate::model::EntityScope::Function, input.function_count),
        (crate::model::EntityScope::Type, input.type_count),
    ])
}

fn detector_execution_receipts(context: &CoverageContext<'_, '_>) -> Vec<DetectorExecutionReceipt> {
    context
        .input
        .manifest
        .iter()
        .map(|entry| {
            let applicable = detector_is_applicable(entry, context.detected_languages);
            let analyzed_entities = if applicable {
                context
                    .entity_counts
                    .get(&entry.entity_scope)
                    .copied()
                    .unwrap_or_else(|| {
                        context
                            .input
                            .findings
                            .iter()
                            .filter(|finding| finding.kind == entry.kind)
                            .count()
                    })
            } else {
                0
            };
            let parse_sensitive = detector_requires_parse(entry);
            let unresolved = if entry.approach == crate::model::DetectionApproach::GraphAnalysis {
                context.input.unresolved_dependency_edges
            } else {
                0
            };
            DetectorExecutionReceipt {
                kind: entry.kind,
                status: if applicable {
                    DetectorExecutionStatus::Completed
                } else {
                    DetectorExecutionStatus::NotApplicable
                },
                analyzed_entities,
                candidate_groups: if entry.entity_scope == crate::model::EntityScope::FindingGroup {
                    context
                        .input
                        .findings
                        .iter()
                        .filter(|finding| finding.kind == entry.kind)
                        .count()
                } else {
                    0
                },
                unobservable_count: if applicable && parse_sensitive {
                    context.input.parse_failures.len() + unresolved
                } else {
                    0
                },
                unobservable_reasons: if applicable {
                    [
                        (!context.input.parse_failures.is_empty() && parse_sensitive).then(|| {
                            format!(
                                "{} source files failed syntax parsing",
                                context.input.parse_failures.len()
                            )
                        }),
                        (unresolved > 0).then(|| {
                            format!("{unresolved} dependency edges could not be resolved")
                        }),
                    ]
                    .into_iter()
                    .flatten()
                    .collect()
                } else {
                    Vec::new()
                },
            }
        })
        .collect()
}

fn coverage_manifest_entries(context: &CoverageContext<'_, '_>) -> Vec<CoverageManifestEntry> {
    coverage_targets()
        .into_iter()
        .map(|target| coverage_manifest_entry(context, target))
        .collect()
}

fn coverage_manifest_entry(
    context: &CoverageContext<'_, '_>,
    (mechanism, entity_scope, expectation): (
        crate::model::SignalMechanism,
        crate::model::EntityScope,
        CoverageExpectation,
    ),
) -> CoverageManifestEntry {
            let entries = context.input.manifest.iter().filter(|entry| entry.mechanism == mechanism && entry.entity_scope == entity_scope).collect::<Vec<_>>();
            let applicable = entries.iter().filter(|entry| detector_is_applicable(entry, context.detected_languages)).collect::<Vec<_>>();
            let completed_detectors = applicable.iter().map(|entry| entry.kind).collect::<Vec<_>>();
            let unsupported_detectors = entries.iter().filter(|entry| !detector_is_applicable(entry, context.detected_languages)).map(|entry| entry.kind).collect::<Vec<_>>();
            let entity_count = context.entity_counts.get(&entity_scope).copied().unwrap_or_else(|| applicable.iter().map(|entry| context.input.findings.iter().filter(|finding| finding.kind == entry.kind).count()).sum());
            let graph_cell = applicable.iter().any(|entry| entry.approach == crate::model::DetectionApproach::GraphAnalysis);
            let partial = !unsupported_detectors.is_empty() || (!context.input.parse_failures.is_empty() && applicable.iter().any(|entry| detector_requires_parse(entry))) || (graph_cell && context.input.unresolved_dependency_edges > 0);
            let status = match expectation {
                CoverageExpectation::Planned => CoverageStatus::Planned,
                CoverageExpectation::IntentionallyOutOfScope => CoverageStatus::IntentionallyOutOfScope,
                CoverageExpectation::Required if applicable.is_empty() => CoverageStatus::Unsupported,
                CoverageExpectation::Required if partial => CoverageStatus::PartiallyObserved,
                CoverageExpectation::Required if entity_count == 0 => CoverageStatus::NoEntities,
                CoverageExpectation::Required => CoverageStatus::Observed,
            };
            CoverageManifestEntry {
                mechanism,
                entity_scope,
                expectation,
                status,
                reason: coverage_reason(status).into(),
                detectors: entries.into_iter().map(|entry| entry.kind).collect(),
                completed_detectors,
                entity_count,
                unobservable_reasons: coverage_unobservable_reasons(context, &unsupported_detectors, graph_cell, partial),
            }
}

fn coverage_unobservable_reasons(
    context: &CoverageContext<'_, '_>,
    unsupported_detectors: &[FindingKind],
    graph_cell: bool,
    partial: bool,
) -> Vec<String> {
    if !partial {
        return Vec::new();
    }
    [
        (!unsupported_detectors.is_empty()).then(|| format!("{} detectors do not support the detected languages: {}", unsupported_detectors.len(), unsupported_detectors.iter().map(|kind| serialized_finding_kind(*kind)).collect::<Vec<_>>().join(", "))),
        (!context.input.parse_failures.is_empty()).then(|| format!("{} source files failed syntax parsing", context.input.parse_failures.len())),
        (graph_cell && context.input.unresolved_dependency_edges > 0).then(|| format!("{} dependency edges could not be resolved", context.input.unresolved_dependency_edges)),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn coverage_summary(context: &CoverageContext<'_, '_>) -> CoverageSummary {
    CoverageSummary {
        detected_languages: context.detected_languages.iter().cloned().collect(),
        applicable_detectors: context.input.manifest
            .iter()
            .filter(|entry| detector_is_applicable(entry, context.detected_languages))
            .map(|entry| entry.kind)
            .collect(),
        analyzed_entities: context.entity_counts.clone(),
        parse_failures: context.input.parse_failures.to_vec(),
        unresolved_dependency_edges: context.input.unresolved_dependency_edges,
        unobservable_reasons: if context.input.parse_failures.is_empty() {
            Vec::new()
        } else {
            vec![format!(
                "{} source files failed syntax parsing",
                context.input.parse_failures.len()
            )]
        },
    }
}

fn detector_requires_parse(entry: &crate::model::DetectorManifestEntry) -> bool {
    matches!(
        entry.entity_scope,
        crate::model::EntityScope::Function | crate::model::EntityScope::Type
    ) || matches!(
        entry.approach,
        crate::model::DetectionApproach::ParsedAnalysis
            | crate::model::DetectionApproach::GraphAnalysis
    )
}

fn coverage_targets() -> Vec<(
    crate::model::SignalMechanism,
    crate::model::EntityScope,
    CoverageExpectation,
)> {
    use crate::model::{EntityScope as E, SignalMechanism as M};
    const MECHANISMS: [M; 7] = [
        M::CognitiveLoad,
        M::DependencyPropagation,
        M::ResponsibilityDispersion,
        M::DuplicationDivergence,
        M::ChangePressure,
        M::VerificationDifficulty,
        M::KnowledgeDrift,
    ];
    const SCOPES: [E; 6] = [
        E::Repository,
        E::Directory,
        E::File,
        E::Function,
        E::Type,
        E::FindingGroup,
    ];
    let required = |m, e| {
        matches!(
            (m, e),
            (M::CognitiveLoad, E::Function)
                | (M::DependencyPropagation, E::File | E::FindingGroup)
                | (
                    M::ResponsibilityDispersion,
                    E::Directory | E::File | E::Type
                )
                | (M::DuplicationDivergence, E::FindingGroup)
                | (M::ChangePressure, E::File | E::FindingGroup)
                | (M::VerificationDifficulty, E::FindingGroup)
                | (M::KnowledgeDrift, E::Directory | E::Repository)
        )
    };
    MECHANISMS
        .into_iter()
        .flat_map(|m| {
            SCOPES.into_iter().map(move |e| {
                (
                    m,
                    e,
                    if required(m, e) {
                        CoverageExpectation::Required
                    } else {
                        CoverageExpectation::IntentionallyOutOfScope
                    },
                )
            })
        })
        .collect()
}

fn coverage_reason(status: CoverageStatus) -> &'static str {
    match status {
        CoverageStatus::Observed => "all applicable detectors completed",
        CoverageStatus::PartiallyObserved => {
            "coverage is incomplete for the detected languages or available entities"
        }
        CoverageStatus::Unsupported => "no detector supports the detected languages",
        CoverageStatus::NoEntities => "no entities were available for analysis",
        CoverageStatus::Planned => "coverage is planned for a future schema",
        CoverageStatus::IntentionallyOutOfScope => {
            "this mechanism and scope are intentionally out of scope"
        }
    }
}

fn canonical_raw_metrics() -> &'static [MetricId] {
    use MetricId::*;
    &[
        FileLoc,
        FileImports,
        FilePublicItems,
        FileIsTest,
        DirectorySourceFiles,
        FunctionLoc,
        FunctionComplexity,
        FunctionNestingDepth,
        FunctionParameterCount,
        FunctionIsTest,
        TypeLoc,
        TypeMemberCount,
        TypeIsTest,
        ChurnCommitsTouched,
        ChurnLinesAdded,
        ChurnLinesDeleted,
        ChurnAuthorsCount,
        ChurnRecentWeighted,
    ]
}

struct RawMetricObservationContext<'a> {
    stats: &'a ScanStats,
    function_count: usize,
    type_count: usize,
    parse_failures: &'a [ParseFailure],
    churn: &'a crate::model::ChurnSummary,
}

fn raw_metric_observation(
    metric: MetricId,
    context: &RawMetricObservationContext<'_>,
) -> RawMetricCoverage {
    let is_churn = matches!(
        metric,
        MetricId::ChurnCommitsTouched
            | MetricId::ChurnLinesAdded
            | MetricId::ChurnLinesDeleted
            | MetricId::ChurnAuthorsCount
            | MetricId::ChurnRecentWeighted
    );
    let parse_sensitive = matches!(
        metric,
        MetricId::FunctionLoc
            | MetricId::FunctionComplexity
            | MetricId::FunctionNestingDepth
            | MetricId::FunctionParameterCount
            | MetricId::FunctionIsTest
            | MetricId::TypeLoc
            | MetricId::TypeMemberCount
            | MetricId::TypeIsTest
            | MetricId::FileImports
            | MetricId::FilePublicItems
    );
    let entity_count = match metric {
        MetricId::DirectorySourceFiles => context.stats.directories_scanned,
        MetricId::FunctionLoc
        | MetricId::FunctionComplexity
        | MetricId::FunctionNestingDepth
        | MetricId::FunctionParameterCount
        | MetricId::FunctionIsTest => context.function_count,
        MetricId::TypeLoc | MetricId::TypeMemberCount | MetricId::TypeIsTest => context.type_count,
        _ => context.stats.source_files_scanned,
    };
    let status = if is_churn && !context.churn.enabled {
        RawMetricCoverageStatus::Unavailable
    } else if parse_sensitive && !context.parse_failures.is_empty() {
        RawMetricCoverageStatus::PartiallyObserved
    } else {
        RawMetricCoverageStatus::Observed
    };
    RawMetricCoverage {
        metric,
        status,
        entity_count,
        reason: match status {
            RawMetricCoverageStatus::Observed => "metric observed for available entities",
            RawMetricCoverageStatus::PartiallyObserved => {
                "metric unavailable for files that failed parsing"
            }
            RawMetricCoverageStatus::Unavailable => {
                "Git churn collection was disabled or unavailable"
            }
        }
        .into(),
        unobservable_reasons: if status == RawMetricCoverageStatus::PartiallyObserved {
            vec![format!(
                "{} source files failed syntax parsing",
                context.parse_failures.len()
            )]
        } else if status == RawMetricCoverageStatus::Unavailable {
            context.churn.reason.clone().into_iter().collect()
        } else {
            Vec::new()
        },
    }
}

fn detector_is_applicable(
    entry: &crate::model::DetectorManifestEntry,
    detected_languages: &BTreeSet<String>,
) -> bool {
    entry.supported_languages.iter().any(|language| {
        matches!(language.as_str(), "repository" | "language_neutral_paths")
            || detected_languages.contains(language)
    })
}

fn detected_language(path: &Path) -> Option<String> {
    const EXTENSION_LANGUAGES: &[(&str, &str)] = &[
        ("rs", "rust"),
        ("js", "javascript"),
        ("jsx", "javascript"),
        ("mjs", "javascript"),
        ("cjs", "javascript"),
        ("ts", "typescript"),
        ("tsx", "tsx"),
        ("vue", "tsx"),
        ("mts", "typescript"),
        ("cts", "typescript"),
        ("py", "python"),
        ("go", "go"),
        ("java", "java"),
        ("cs", "csharp"),
        ("csx", "csharp"),
        ("kt", "kotlin"),
        ("php", "php"),
        ("rb", "ruby"),
        ("sh", "bash"),
        ("bash", "bash"),
        ("ps1", "powershell"),
        ("psm1", "powershell"),
        ("c", "c"),
        ("h", "c"),
        ("cc", "cpp"),
        ("cpp", "cpp"),
        ("cxx", "cpp"),
        ("hh", "cpp"),
        ("hpp", "cpp"),
        ("hxx", "cpp"),
    ];
    let extension = path.extension()?.to_str()?;
    EXTENSION_LANGUAGES
        .iter()
        .find_map(|(candidate, language)| (*candidate == extension).then(|| (*language).into()))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    fn source_file(path: &str) -> SourceFile {
        SourceFile {
            path: PathBuf::from(path),
            display_path: path.to_string(),
            source: "".into(),
        }
    }

    #[test]
    fn detects_bash_and_powershell_coverage_languages() {
        let files = vec![
            source_file("scripts/build.sh"),
            source_file("scripts/install.bash"),
            source_file("scripts/deploy.ps1"),
            source_file("scripts/module.psm1"),
        ];

        let languages = coverage_languages(&files, false);

        assert!(languages.contains("bash"));
        assert!(languages.contains("powershell"));
        assert_eq!(detected_language(Path::new("module.psd1")), None);
    }
}
