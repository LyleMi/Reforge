use super::*;

pub(super) fn build_report(run: RunResult, root: &Path, analyses: &BTreeSet<Analysis>) -> Report {
    let detections = selected_detections(&run, analyses);
    let issues = aggregate_issues(&detections);
    let coverage = analysis_coverage(&run, analyses);
    let suppression = suppression_summary(&run, analyses);
    Report::new(
        Producer {
            name: "reforge.analyze".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            revision: option_env!("REFORGE_BUILD_REVISION").map(str::to_owned),
        },
        Target {
            root: root.to_string_lossy().into_owned(),
            workspace_identity: crate::pathing::workspace_identity(root),
            source_revision: run.source_revision.clone(),
        },
        suppression,
        coverage,
        issues,
    )
}

fn selected_detections<'a>(
    run: &'a RunResult,
    analyses: &BTreeSet<Analysis>,
) -> BTreeMap<String, &'a DetectedEvidence> {
    run.detected_evidence
        .iter()
        .filter(|detection| owner_selected(analyses, detection.kind))
        .map(|detection| {
            (
                format!(
                    "{}\0{}",
                    unified_rule(detection.kind),
                    detection.semantic_anchor
                ),
                detection,
            )
        })
        .collect()
}

fn aggregate_issues(detections: &BTreeMap<String, &DetectedEvidence>) -> Vec<Issue> {
    let mut grouped = BTreeMap::<(String, Subject), (String, IssueFamily, Vec<Evidence>)>::new();

    for detection in detections.values().copied() {
        let definition = rule_definition(detection.kind);
        let family = definition.family;
        let subject = candidate_subject(detection).canonicalized();
        grouped
            .entry((family, subject))
            .or_insert_with(|| {
                (
                    crate::detectors::manifest::analysis_name(detection.kind).into(),
                    definition.issue_family,
                    Vec::new(),
                )
            })
            .2
            .push(convert_evidence(detection));
    }

    grouped
        .into_iter()
        .map(|((family, subject), (analysis, issue_family, evidence))| {
            let title = format!("{}: {}", issue_family.title(), subject.display_name());
            Issue::new(
                analysis,
                family,
                subject,
                (title, issue_family.guidance()),
                evidence,
            )
        })
        .collect()
}

fn candidate_subject(detection: &DetectedEvidence) -> Subject {
    if crate::detectors::manifest::analysis_name(detection.kind) == ANALYSIS_DATAFLOW {
        if let Some(witness) = &detection.flow_witness {
            return Subject::Symbol {
                path: witness.source.path.clone(),
                symbol: human_flow_symbol(&witness.source.function, &witness.source.name),
            };
        }
    }
    match crate::detectors::manifest::subject_kind(detection.kind) {
        crate::model::SubjectKind::Repository => return Subject::Repository,
        crate::model::SubjectKind::Directory => {
            return Subject::Directory {
                path: detection.path.clone(),
            };
        }
        crate::model::SubjectKind::Symbol => {
            return Subject::Symbol {
                path: detection.path.clone(),
                symbol: anchor_symbol(&detection.semantic_anchor),
            };
        }
        crate::model::SubjectKind::Group => {}
        crate::model::SubjectKind::File => {
            return Subject::File {
                path: detection.path.clone(),
            };
        }
    }
    {
        let mut members = detection
            .related_locations
            .iter()
            .map(|location| {
                format!(
                    "{}#{}",
                    location.path,
                    location.name.as_deref().unwrap_or("member")
                )
            })
            .collect::<Vec<_>>();
        if members.is_empty() {
            members.push(format!(
                "{}#{}",
                detection.path,
                anchor_symbol(&detection.semantic_anchor)
            ));
        }
        Subject::Group { members }
    }
}

fn human_flow_symbol(function: &str, name: &str) -> String {
    if !function.is_empty() {
        function.to_owned()
    } else if !name.is_empty() && !name.starts_with("flow:") {
        name.to_owned()
    } else {
        "source".into()
    }
}

fn analysis_coverage(
    run: &RunResult,
    analyses: &BTreeSet<Analysis>,
) -> BTreeMap<String, AnalysisCoverage> {
    let mut coverage = BTreeMap::new();
    for analysis in analyses {
        coverage.insert(
            analysis.as_str().into(),
            analysis_coverage_entry(run, analyses, *analysis, language_coverage(run, *analysis)),
        );
    }
    coverage
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
        coverage.limitations.push(CoverageLimitation {
            code: "parse_failure".into(),
            count: parse_failures,
            message: "source files could not be parsed".into(),
        });
    }
}

fn dataflow_supports(language: &str) -> bool {
    matches!(
        language,
        "rust" | "javascript" | "typescript" | "tsx" | "python"
    )
}

fn language_for_path(path: &str) -> Option<&'static str> {
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
        limitations.push(CoverageLimitation {
            code: "parse_failure".into(),
            count: run.parse_failures.len(),
            message: "source files could not be parsed".into(),
        });
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

fn suppression_summary(run: &RunResult, analyses: &BTreeSet<Analysis>) -> SuppressionSummary {
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

fn convert_evidence(detection: &DetectedEvidence) -> Evidence {
    let rule = unified_rule(detection.kind);
    let mut evidence = Evidence::new(rule, &detection.semantic_anchor, &detection.message);
    evidence.measurements = detection
        .metrics
        .iter()
        .map(|metric| Measurement {
            name: metric.name.to_string(),
            value: metric.value as f64,
            threshold: metric.threshold.map(|value| value as f64),
            unit: metric.unit.clone(),
        })
        .collect();
    evidence.locations.push(Location {
        path: detection.path.clone(),
        line: detection.line,
        symbol: None,
    });
    evidence
        .locations
        .extend(detection.related_locations.iter().map(|location| Location {
            path: location.path.clone(),
            line: Some(location.line),
            symbol: location.name.clone(),
        }));
    evidence.witness = detection.flow_witness.as_ref().map(convert_flow_witness);
    evidence
}

fn convert_flow_witness(witness: &crate::model::FlowWitness) -> reforge_schema::FlowWitness {
    reforge_schema::FlowWitness {
        source: convert_endpoint(&witness.source),
        sink: convert_endpoint(&witness.sink),
        ordered_steps: witness
            .ordered_steps
            .iter()
            .map(|step| reforge_schema::FlowStep {
                path: step.path.clone(),
                symbol: step.name.clone(),
                line: Some(step.line),
                operation: enum_name(&step.kind),
                resolution: convert_resolution(step.resolution),
            })
            .collect(),
        function_hops: witness.function_hops,
        module_hops: witness.module_hops,
        resolution: convert_resolution(witness.resolution),
    }
}

fn convert_endpoint(location: &crate::model::FlowLocation) -> reforge_schema::FlowEndpoint {
    reforge_schema::FlowEndpoint {
        path: location.path.clone(),
        symbol: human_flow_symbol(&location.function, &location.name),
        language: location.language.clone(),
        line: Some(location.line),
    }
}

fn convert_resolution(value: crate::model::FlowResolution) -> reforge_schema::FlowResolution {
    match value {
        crate::model::FlowResolution::Exact => reforge_schema::FlowResolution::Exact,
        crate::model::FlowResolution::Partial => reforge_schema::FlowResolution::Partial,
        crate::model::FlowResolution::Unresolved => reforge_schema::FlowResolution::Unresolved,
        crate::model::FlowResolution::Unsupported => reforge_schema::FlowResolution::Unsupported,
    }
}

pub(super) fn unified_rule(kind: Rule) -> String {
    rule_definition(kind).rule.to_owned()
}

pub(super) fn enum_name<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
}

fn anchor_symbol(semantic_anchor: &str) -> String {
    semantic_anchor
        .rsplit_once(':')
        .map(|(_, value)| value)
        .unwrap_or(semantic_anchor)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_analysis::DetectedEvidenceInput;
    use crate::model::{DetectedMeasurement, MetricId};

    fn detected(kind: Rule, metric: DetectedMeasurement) -> DetectedEvidence {
        DetectedEvidence::from(DetectedEvidenceInput::new(
            kind,
            "src/lib.rs",
            Some(10),
            "evidence",
            vec![metric],
        ))
    }

    #[test]
    fn aggregation_uses_narrow_family_guidance_independent_of_evidence_order() {
        let long = detected(
            Rule::LongFunction,
            DetectedMeasurement::threshold(MetricId::FunctionLoc, 90, 80, "lines"),
        );
        let complex = detected(
            Rule::ComplexFunction,
            DetectedMeasurement::threshold(MetricId::FunctionComplexity, 16, 15, "branches"),
        );
        let detections = BTreeMap::from([
            ("complex".to_string(), &complex),
            ("long".to_string(), &long),
        ]);
        let issues = aggregate_issues(&detections);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].family, "reforge.codebase.function_readability");
        assert_eq!(
            issues[0].guidance,
            IssueFamily::FunctionReadability.guidance()
        );
    }

    #[test]
    fn aggregation_does_not_merge_different_families_on_the_same_subject() {
        let large = detected(
            Rule::LargeFile,
            DetectedMeasurement::threshold(MetricId::FileLoc, 900, 800, "lines"),
        );
        let imports = detected(
            Rule::ImportHeavyFile,
            DetectedMeasurement::threshold(MetricId::FileImports, 40, 35, "imports"),
        );
        let detections = BTreeMap::from([
            ("large".to_string(), &large),
            ("imports".to_string(), &imports),
        ]);
        assert_eq!(aggregate_issues(&detections).len(), 2);
    }
}
