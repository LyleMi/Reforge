use super::*;

mod conversion;
mod coverage;
mod provenance;

use conversion::{aggregate_issues, selected_detections};
use coverage::{analysis_coverage, suppression_summary};
use provenance::report_provenance;

pub(super) fn build_report(run: RunResult, root: &Path, config: &Config) -> Report {
    let detections = selected_detections(&run, config);
    let issues = aggregate_issues(&detections, config);
    let coverage = analysis_coverage(&run, config);
    let suppression = suppression_summary(&run, &config.enabled);
    let provenance = report_provenance(&run, config);
    Report::new(reforge_schema::ReportInput {
        producer: Producer {
            name: "reforge.analyze".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            revision: option_env!("REFORGE_BUILD_REVISION").map(str::to_owned),
        },
        target: Target {
            root: root.to_string_lossy().into_owned(),
            workspace_identity: crate::pathing::workspace_identity(root),
            source_revision: run.source_revision.clone(),
        },
        provenance,
        suppression,
        coverage,
        issues,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_analysis::DetectedEvidenceInput;
    use crate::model::{DetectedMeasurement, MetricId};

    fn detected(kind: Rule, metric: DetectedMeasurement) -> DetectedEvidence {
        let input =
            DetectedEvidenceInput::new(kind, "src/lib.rs", Some(10), "evidence", vec![metric]);
        let input = if matches!(kind, Rule::LongFunction | Rule::ComplexFunction) {
            input.with_symbol_subject("function", "sample", Some("arity:0".into()))
        } else {
            input.with_file_subject()
        };
        DetectedEvidence::from(input)
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
        let issues = aggregate_issues(&detections, &Config::defaults());
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
        assert_eq!(aggregate_issues(&detections, &Config::defaults()).len(), 2);
    }
}
