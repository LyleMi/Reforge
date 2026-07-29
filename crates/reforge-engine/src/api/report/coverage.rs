use super::*;

pub(super) fn analysis_coverage(
    run: &RunResult,
    config: &Config,
) -> BTreeMap<String, AnalysisCoverage> {
    config
        .enabled
        .iter()
        .map(|analysis| {
            (
                analysis.as_str().into(),
                configured_analysis_coverage(run, config, *analysis),
            )
        })
        .collect()
}

fn configured_analysis_coverage(
    run: &RunResult,
    config: &Config,
    analysis: Analysis,
) -> AnalysisCoverage {
    let mut coverage = analysis_coverage_entry(
        run,
        &config.enabled,
        analysis,
        language_coverage(run, analysis),
    );
    for spec in crate::detectors::manifest::rule_registry()
        .iter()
        .filter(|spec| spec.analysis == analysis.as_str())
    {
        if let Some(execution) = coverage.rules.get_mut(&spec.rule) {
            execution.maturity = enum_name(&spec.maturity);
            execution.enabled_source = rule_activation(spec, config);
        }
    }
    coverage
}

fn rule_activation(
    spec: &crate::model::RuleSpec,
    config: &Config,
) -> reforge_schema::RuleActivation {
    [
        (
            config.rule_enforced(&spec.rule),
            reforge_schema::RuleActivation::Enforce,
        ),
        (
            config.rules.enabled.contains(&spec.rule),
            reforge_schema::RuleActivation::Enable,
        ),
        (
            spec.default_enabled && !config.rules.disabled.contains(&spec.rule),
            reforge_schema::RuleActivation::Default,
        ),
        (
            spec.maturity == crate::model::RuleMaturity::Experimental,
            reforge_schema::RuleActivation::Internal,
        ),
    ]
    .into_iter()
    .find_map(|(selected, activation)| selected.then_some(activation))
    .unwrap_or(reforge_schema::RuleActivation::Disabled)
}

fn analysis_coverage_entry(
    run: &RunResult,
    analyses: &BTreeSet<Analysis>,
    analysis: Analysis,
    languages: BTreeMap<String, LanguageCoverage>,
) -> AnalysisCoverage {
    let (status, limitations) = match analysis {
        Analysis::Codebase => (
            if run.parse_failures.is_empty() && run.source_failures.is_empty() {
                CoverageStatus::Observed
            } else {
                CoverageStatus::Partial
            },
            structure_limitations(run),
        ),
        Analysis::Dataflow => {
            let limitations = dataflow_limitations(run);
            (
                if limitations.is_empty() {
                    CoverageStatus::Observed
                } else {
                    CoverageStatus::Partial
                },
                limitations,
            )
        }
    };
    AnalysisCoverage {
        status,
        scanned_files: run.stats.source_files_analyzed,
        languages,
        rules: rule_execution(run, |kind| {
            owner_selected(analyses, kind)
                && crate::detectors::manifest::analysis_name(kind) == analysis.as_str()
        }),
        limitations,
    }
}

fn rule_execution(
    run: &RunResult,
    selected: impl Fn(Rule) -> bool,
) -> BTreeMap<String, RuleExecution> {
    run.rule_execution
        .iter()
        .filter(|(kind, _)| selected(**kind))
        .map(|(kind, execution)| (unified_rule(*kind), execution.clone()))
        .collect()
}

fn language_coverage(run: &RunResult, analysis: Analysis) -> BTreeMap<String, LanguageCoverage> {
    let mut languages = BTreeMap::<String, LanguageCoverage>::new();
    for file in &run.raw_metrics.files {
        if let Some(language) = language_for_path(&file.path) {
            languages.entry(language.into()).or_default().files += 1;
        }
    }
    for function in &run.raw_metrics.functions {
        if let Some(language) = language_for_path(&function.path) {
            languages.entry(language.into()).or_default().functions += 1;
        }
    }
    for (language, coverage) in &mut languages {
        apply_language_status(run, analysis, language, coverage);
    }
    languages
}

fn apply_language_status(
    run: &RunResult,
    analysis: Analysis,
    language: &str,
    coverage: &mut LanguageCoverage,
) {
    if analysis == Analysis::Dataflow && !dataflow_supports(language) {
        coverage.status = CoverageStatus::Unsupported;
        coverage.limitations.push(CoverageLimitation {
            code: "language_unsupported".into(),
            count: coverage.files.max(1),
            message: format!("{language} is not supported by Dataflow analysis"),
        });
        return;
    }
    let parse_failures = run
        .parse_failures
        .iter()
        .filter(|failure| failure.language == language)
        .count();
    if parse_failures > 0 {
        coverage.status = CoverageStatus::Partial;
        coverage.limitations.push(parse_failure(parse_failures));
    }
    if analysis == Analysis::Dataflow {
        coverage.capabilities = dataflow_capability_receipts(run, language, parse_failures);
    }
}

#[derive(Clone, Copy)]
enum CapabilityMode {
    Syntax,
    DirectCalls,
    Unsupported {
        code: &'static str,
        message: &'static str,
    },
}

struct CapabilitySpec {
    name: &'static str,
    mode: CapabilityMode,
}

const DATAFLOW_CAPABILITIES: &[CapabilitySpec] = &[
    CapabilitySpec {
        name: "syntax",
        mode: CapabilityMode::Syntax,
    },
    CapabilitySpec {
        name: "symbols",
        mode: CapabilityMode::Syntax,
    },
    CapabilitySpec {
        name: "lexical_scopes",
        mode: CapabilityMode::Syntax,
    },
    CapabilitySpec {
        name: "local_def_use",
        mode: CapabilityMode::Syntax,
    },
    CapabilitySpec {
        name: "direct_calls",
        mode: CapabilityMode::DirectCalls,
    },
    CapabilitySpec {
        name: "call_return_composition",
        mode: CapabilityMode::DirectCalls,
    },
    CapabilitySpec {
        name: "field_flow",
        mode: CapabilityMode::Unsupported {
            code: "field_flow_unsupported",
            message: "field and heap alias flow is modeled for observation but not exact",
        },
    },
    CapabilitySpec {
        name: "dynamic_dispatch",
        mode: CapabilityMode::Unsupported {
            code: "dynamic_dispatch_unsupported",
            message: "method, trait, interface, member, and dynamic dispatch are not resolved",
        },
    },
];

fn dataflow_capability_receipts(
    run: &RunResult,
    language: &str,
    parse_failures: usize,
) -> BTreeMap<String, reforge_schema::CapabilityReceipt> {
    let syntax_status = status_for_limitations(parse_failures);
    let unresolved_count = run
        .flow_analysis
        .unresolved_edges_by_language
        .get(language)
        .copied()
        .unwrap_or_default();
    let direct_status = status_for_limitations(parse_failures + unresolved_count);

    DATAFLOW_CAPABILITIES
        .iter()
        .map(|spec| {
            let mut receipt = match spec.mode {
                CapabilityMode::Syntax => receipt(syntax_status),
                CapabilityMode::DirectCalls => {
                    let mut receipt = receipt(direct_status);
                    if unresolved_count > 0 {
                        receipt.limitations.push(CoverageLimitation {
                            code: "unresolved_direct_call".into(),
                            count: unresolved_count,
                            message: "ambiguous or unsupported calls were not connected".into(),
                        });
                    }
                    receipt
                }
                CapabilityMode::Unsupported { code, message } => {
                    reforge_schema::CapabilityReceipt {
                        status: CoverageStatus::Unsupported,
                        limitations: vec![CoverageLimitation {
                            code: code.into(),
                            count: 1,
                            message: message.into(),
                        }],
                    }
                }
            };
            if parse_failures > 0 && receipt.status.is_observable() {
                receipt.limitations.push(parse_failure(parse_failures));
            }
            (spec.name.into(), receipt)
        })
        .collect()
}

fn status_for_limitations(count: usize) -> CoverageStatus {
    if count == 0 {
        CoverageStatus::Observed
    } else {
        CoverageStatus::Partial
    }
}

fn receipt(status: CoverageStatus) -> reforge_schema::CapabilityReceipt {
    reforge_schema::CapabilityReceipt {
        status,
        limitations: Vec::new(),
    }
}

fn parse_failure(count: usize) -> CoverageLimitation {
    CoverageLimitation {
        code: "parse_failure".into(),
        count,
        message: "source files could not be parsed".into(),
    }
}

fn dataflow_supports(language: &str) -> bool {
    matches!(
        language,
        "rust" | "javascript" | "typescript" | "tsx" | "python"
    )
}

pub(super) fn language_for_path(path: &str) -> Option<&'static str> {
    let extension = Path::new(path).extension()?.to_str()?;
    match extension {
        "rs" => Some("rust"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "ts" | "mts" | "cts" => Some("typescript"),
        "tsx" | "vue" => Some("tsx"),
        "py" => Some("python"),
        "go" => Some("go"),
        "java" => Some("java"),
        "cs" | "csx" => Some("csharp"),
        "kt" => Some("kotlin"),
        "php" => Some("php"),
        "rb" => Some("ruby"),
        "sh" | "bash" => Some("bash"),
        "ps1" | "psm1" => Some("powershell"),
        "c" | "cc" | "cpp" => Some("cpp"),
        _ => None,
    }
}

fn structure_limitations(run: &RunResult) -> Vec<CoverageLimitation> {
    let mut limitations = Vec::new();
    if !run.source_failures.is_empty() {
        limitations.push(CoverageLimitation {
            code: "source_read_failure".into(),
            count: run.source_failures.len(),
            message: "source files could not be read".into(),
        });
    }
    if !run.parse_failures.is_empty() {
        limitations.push(parse_failure(run.parse_failures.len()));
    }
    limitations
}

fn dataflow_limitations(run: &RunResult) -> Vec<CoverageLimitation> {
    let mut limitations = structure_limitations(run);
    if run.flow_analysis.unresolved_edges > 0 {
        limitations.push(CoverageLimitation {
            code: "unresolved_flow_edge".into(),
            count: run.flow_analysis.unresolved_edges,
            message: "flow edges could not be resolved exactly".into(),
        });
    }
    if run.flow_analysis.truncated_paths > 0 {
        limitations.push(CoverageLimitation {
            code: "truncated_flow_path".into(),
            count: run.flow_analysis.truncated_paths,
            message: "flow path search reached a configured budget".into(),
        });
    }
    limitations
}

pub(super) fn suppression_summary(
    run: &RunResult,
    analyses: &BTreeSet<Analysis>,
) -> SuppressionSummary {
    SuppressionSummary {
        evidence_count: run
            .suppression_summary
            .suppressed_by_kind
            .iter()
            .filter(|(kind, _)| owner_selected(analyses, **kind))
            .map(|(_, count)| count)
            .sum(),
        by_rule: run
            .suppression_summary
            .suppressed_by_kind
            .iter()
            .filter(|(kind, _)| owner_selected(analyses, **kind))
            .map(|(kind, count)| (unified_rule(*kind), *count))
            .collect(),
    }
}
