use super::*;

pub(super) fn report_provenance(
    run: &RunResult,
    config: &Config,
) -> reforge_schema::ReportProvenance {
    reforge_schema::ReportProvenance {
        identity_scheme: reforge_schema::IDENTITY_SCHEME.into(),
        scope_digest: provenance_digest("scope", &scope_values(run, config)),
        analyses: analysis_provenance(config),
        rules: rule_provenance(config),
    }
}

fn scope_values(run: &RunResult, config: &Config) -> Vec<String> {
    let mut values = vec![
        config.scope.include_hidden.to_string(),
        config.scope.include_generated.to_string(),
        config.scope.no_gitignore.to_string(),
        config.scope.exclude_tests.to_string(),
    ];
    values.extend(config.scope.ignore_paths.iter().cloned());
    values.extend(run.raw_metrics.files.iter().map(|file| file.path.clone()));
    values.sort();
    values
}

fn analysis_provenance(config: &Config) -> BTreeMap<String, reforge_schema::AnalysisProvenance> {
    config
        .enabled
        .iter()
        .map(|analysis| {
            let analysis_name = analysis.as_str();
            let analysis_value = config
                .source
                .get(analysis_name)
                .map(ToString::to_string)
                .unwrap_or_default();
            let policy_value = if *analysis == Analysis::Dataflow {
                config
                    .source
                    .get(ANALYSIS_DATAFLOW)
                    .and_then(|value| value.get("policies"))
                    .map(ToString::to_string)
                    .unwrap_or_default()
            } else {
                String::new()
            };
            (
                analysis_name.into(),
                reforge_schema::AnalysisProvenance {
                    config_digest: provenance_digest("analysis-config", &[analysis_value]),
                    policy_digest: provenance_digest(
                        "analysis-policy",
                        &[
                            policy_value,
                            config
                                .rules
                                .enforced
                                .iter()
                                .cloned()
                                .collect::<Vec<_>>()
                                .join("|"),
                        ],
                    ),
                },
            )
        })
        .collect()
}

fn rule_provenance(config: &Config) -> BTreeMap<String, reforge_schema::RuleProvenance> {
    crate::detectors::manifest::rule_registry()
        .iter()
        .filter(|spec| {
            config
                .enabled
                .iter()
                .any(|analysis| analysis.as_str() == spec.analysis)
        })
        .map(|spec| {
            let analysis_config = config
                .source
                .get(&spec.analysis)
                .map(ToString::to_string)
                .unwrap_or_default();
            (
                spec.rule.clone(),
                reforge_schema::RuleProvenance {
                    semantic_version: spec.semantic_version.clone(),
                    evaluation_digest: provenance_digest(
                        "rule-evaluation",
                        &[
                            spec.rule.clone(),
                            analysis_config,
                            config
                                .rule_enabled(&spec.rule, spec.default_enabled)
                                .to_string(),
                            config.rule_enforced(&spec.rule).to_string(),
                        ],
                    ),
                },
            )
        })
        .collect()
}

fn provenance_digest(label: &str, values: &[String]) -> String {
    use sha2::{Digest, Sha256};

    let mut hash = Sha256::new();
    hash.update(label.as_bytes());
    hash.update([0]);
    for value in values {
        hash.update(value.as_bytes());
        hash.update([0]);
    }
    format!("rp7-{:x}", hash.finalize())[..24].to_owned()
}
