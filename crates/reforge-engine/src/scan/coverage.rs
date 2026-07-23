use reforge_schema::{CoverageLimitation, CoverageObservation, CoverageStatus, RuleExecution};

struct CoverageProjectionInput<'a> {
    manifest: &'a [crate::model::RuleSpec],
    stats: &'a RunStats,
    source_files: &'a [SourceFile],
    function_count: usize,
    type_count: usize,
    dependency_nodes: usize,
    parse_failures: &'a [ParseFailure],
    source_failures: &'a [SourceFailure],
    unresolved_dependency_edges: usize,
    flow_analysis: &'a crate::model::FlowAnalysisSummary,
    similarity_comparisons: &'a crate::detectors::similarity::SimilarityComparisonStats,
}

fn coverage(input: CoverageProjectionInput<'_>) -> BTreeMap<Rule, RuleExecution> {
    let detected_languages =
        coverage_languages(input.source_files, input.source_failures);
    let observations = observation_counts(&input);
    let context = CoverageContext {
        input: &input,
        detected_languages: &detected_languages,
        observations: &observations,
    };
    input
        .manifest
        .iter()
        .map(|entry| (entry.kind, rule_execution(entry, &context)))
        .collect()
}

struct CoverageContext<'a, 'input> {
    input: &'a CoverageProjectionInput<'input>,
    detected_languages: &'a BTreeSet<String>,
    observations: &'a BTreeMap<crate::model::ObservationSource, usize>,
}

fn coverage_languages(
    source_files: &[SourceFile],
    source_failures: &[SourceFailure],
) -> BTreeSet<String> {
    let mut detected_languages = source_files
        .iter()
        .filter_map(|source| detected_language(&source.path))
        .collect::<BTreeSet<_>>();
    detected_languages.extend(
        source_failures
            .iter()
            .filter_map(|failure| detected_language(Path::new(&failure.path))),
    );
    detected_languages
}

fn observation_counts(
    input: &CoverageProjectionInput<'_>,
) -> BTreeMap<crate::model::ObservationSource, usize> {
    use crate::model::ObservationSource as O;
    BTreeMap::from([
        (O::Repositories, 1),
        (O::Directories, input.stats.directories_scanned),
        (O::Files, input.stats.source_files_analyzed),
        (O::Functions, input.function_count),
        (O::Types, input.type_count),
        (
            O::FunctionPairs,
            input.similarity_comparisons.total_candidate_pairs,
        ),
        (O::DependencyNodes, input.dependency_nodes),
        (O::DataflowSources, 0),
    ])
}

fn rule_execution(
    entry: &crate::model::RuleSpec,
    context: &CoverageContext<'_, '_>,
) -> RuleExecution {
    let applicable = detector_runtime_applicable(entry, context);
    if entry.kind == Rule::AdapterFlowBypass && !context.input.flow_analysis.policy_configured {
        return RuleExecution {
            status: CoverageStatus::NotApplicable,
            observations: Vec::new(),
            limitations: vec![CoverageLimitation {
                code: "policy_not_configured".into(),
                count: 1,
                message: "no adapter boundary policy is configured".into(),
            }],
        };
    }
    if !applicable {
        return RuleExecution {
            status: CoverageStatus::NotApplicable,
            observations: Vec::new(),
            limitations: Vec::new(),
        };
    }
    let limitations = rule_limitations(entry, context);
    RuleExecution {
        status: if limitations.is_empty() {
            CoverageStatus::Observed
        } else {
            CoverageStatus::Partial
        },
        observations: vec![CoverageObservation {
            name: observation_name(entry.observation_source).into(),
            count: observation_count(entry, context),
            unit: observation_unit(entry.observation_source).into(),
        }],
        limitations,
    }
}

fn observation_count(
    entry: &crate::model::RuleSpec,
    context: &CoverageContext<'_, '_>,
) -> usize {
    match entry.kind {
        Rule::AdapterFlowBypass => context.input.flow_analysis.protected_sources_evaluated,
        Rule::ExcessiveRelay | Rule::FlowFanOut => {
            context.input.flow_analysis.relay_sources_evaluated
        }
        _ => context
            .observations
            .get(&entry.observation_source)
            .copied()
            .unwrap_or_default(),
    }
}

fn observation_name(source: crate::model::ObservationSource) -> &'static str {
    use crate::model::ObservationSource as O;
    match source {
        O::Repositories => "repositories_scanned",
        O::Directories => "directories_scanned",
        O::Files => "files_scanned",
        O::Functions => "functions_analyzed",
        O::Types => "types_analyzed",
        O::FunctionPairs => "function_pairs_compared",
        O::DependencyNodes => "dependency_nodes_analyzed",
        O::DataflowSources => "dataflow_sources_evaluated",
    }
}

fn observation_unit(source: crate::model::ObservationSource) -> &'static str {
    use crate::model::ObservationSource as O;
    match source {
        O::Repositories => "repository",
        O::Directories => "directory",
        O::Files => "file",
        O::Functions => "function",
        O::Types => "type",
        O::FunctionPairs => "function_pair",
        O::DependencyNodes => "dependency_node",
        O::DataflowSources => "dataflow_source",
    }
}

fn rule_limitations(
    entry: &crate::model::RuleSpec,
    context: &CoverageContext<'_, '_>,
) -> Vec<CoverageLimitation> {
    let mut limitations = Vec::new();
    if detector_requires_parse(entry) && !context.input.parse_failures.is_empty() {
        limitations.push(CoverageLimitation {
            code: "parse_failure".into(),
            count: context.input.parse_failures.len(),
            message: "source files could not be parsed".into(),
        });
    }
    if !context.input.source_failures.is_empty() {
        limitations.push(CoverageLimitation {
            code: "source_read_failure".into(),
            count: context.input.source_failures.len(),
            message: "source files could not be read".into(),
        });
    }
    if is_dependency_graph_rule(entry.kind) && context.input.unresolved_dependency_edges > 0 {
        limitations.push(CoverageLimitation {
            code: "unresolved_dependency_edge".into(),
            count: context.input.unresolved_dependency_edges,
            message: "dependency edges could not be resolved".into(),
        });
    }
    if is_dataflow_rule(entry.kind) && context.input.flow_analysis.unresolved_edges > 0 {
        limitations.push(CoverageLimitation {
            code: "unresolved_flow_edge".into(),
            count: context.input.flow_analysis.unresolved_edges,
            message: "flow edges could not be resolved exactly".into(),
        });
    }
    if is_dataflow_rule(entry.kind) && context.input.flow_analysis.truncated_paths > 0 {
        limitations.push(CoverageLimitation {
            code: "truncated_flow_path".into(),
            count: context.input.flow_analysis.truncated_paths,
            message: "flow path search reached a configured budget".into(),
        });
    }
    limitations
}

fn detector_requires_parse(entry: &crate::model::RuleSpec) -> bool {
    use crate::model::ObservationSource as O;
    matches!(
        entry.observation_source,
        O::Functions | O::Types | O::FunctionPairs | O::DataflowSources
    ) || entry.languages.iter().any(|language| {
        !matches!(
            language.as_str(),
            "repository" | "language_neutral_paths"
        )
    })
}

fn detector_is_applicable(
    entry: &crate::model::RuleSpec,
    detected_languages: &BTreeSet<String>,
) -> bool {
    entry.languages.iter().any(|language| {
        matches!(language.as_str(), "repository" | "language_neutral_paths")
            || detected_languages.contains(language)
    })
}

fn detector_runtime_applicable(
    entry: &crate::model::RuleSpec,
    context: &CoverageContext<'_, '_>,
) -> bool {
    detector_is_applicable(entry, context.detected_languages)
}

fn is_dataflow_rule(kind: Rule) -> bool {
    matches!(
        kind,
        Rule::AdapterFlowBypass | Rule::ExcessiveRelay | Rule::FlowFanOut
    )
}

fn is_dependency_graph_rule(kind: Rule) -> bool {
    matches!(
        kind,
        Rule::DependencyCycle
            | Rule::DependencyHub
    )
}

include!("coverage_languages.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

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

        let languages = coverage_languages(&files, &[]);

        assert!(languages.contains("bash"));
        assert!(languages.contains("powershell"));
        assert_eq!(detected_language(Path::new("module.psd1")), None);
    }
}
