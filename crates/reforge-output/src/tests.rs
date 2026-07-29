use super::*;
use reforge_schema::{
    AnalysisCoverage, BaselineComparison, BaselineEntry, BaselineState, CapabilityReceipt,
    CoverageLimitation, CoverageStatus, EntityRef, Evidence, Issue, IssueInput, LanguageCoverage,
    Producer, ReportInput, Subject, SuppressionSummary, Target, default_provenance,
};
use std::collections::BTreeMap;

#[test]
fn infers_extensions() {
    assert_eq!(
        OutputFormat::infer(None, Some(Path::new("report.sarif"))),
        OutputFormat::Sarif
    );
    assert_eq!(
        OutputFormat::infer(None, Some(Path::new("report.yml"))),
        OutputFormat::Yaml
    );
}

#[test]
fn schema_27_is_a_hard_input_boundary() {
    ensure_schema_27(&serde_json::json!({ "schema_version": 27 })).unwrap();
    let error = ensure_schema_27(&serde_json::json!({ "schema_version": 26 }))
        .unwrap_err()
        .to_string();
    assert!(error.contains("expected schema 27"));
}

fn report_with_issue() -> Report {
    let coverage = BTreeMap::from([(
        "codebase".into(),
        AnalysisCoverage {
            status: CoverageStatus::Observed,
            scanned_files: 1,
            languages: BTreeMap::from([(
                "rust".into(),
                LanguageCoverage {
                    status: CoverageStatus::Partial,
                    files: 1,
                    functions: 1,
                    capabilities: BTreeMap::from([(
                        "direct_calls".into(),
                        CapabilityReceipt {
                            status: CoverageStatus::Partial,
                            limitations: vec![CoverageLimitation {
                                code: "unresolved_direct_call".into(),
                                count: 1,
                                message: "one call could not be resolved".into(),
                            }],
                        },
                    )]),
                    limitations: Vec::new(),
                },
            )]),
            rules: BTreeMap::new(),
            limitations: Vec::new(),
        },
    )]);
    let issues = vec![Issue::new(IssueInput {
        kind: reforge_schema::IssueKind::Advisory,
        analysis: "codebase".into(),
        family: "reforge.codebase.responsibility_decomposition".into(),
        subject: Subject::File {
            entity: EntityRef::new("file:src/lib.rs", "src/lib.rs", None),
        },
        title: "Large file".into(),
        guidance: "Split it".into(),
        evidence: vec![Evidence::new(
            "reforge.codebase.large_file",
            "file:src/lib.rs",
            "large file",
        )],
    })];
    Report::new(ReportInput {
        producer: Producer {
            name: "reforge.analyze".into(),
            version: "test".into(),
            revision: None,
        },
        target: Target {
            root: "/tmp/work".into(),
            workspace_identity: "rw5-test".into(),
            source_revision: None,
        },
        provenance: default_provenance(&coverage, &issues),
        suppression: SuppressionSummary::default(),
        coverage,
        issues,
    })
}

#[test]
fn sarif_omits_missing_and_unknown_baseline_states() {
    let mut report = report_with_issue();
    let issue_id = report.issues[0].id.clone();
    let without_baseline = sarif(&report);
    assert!(
        without_baseline["runs"][0]["results"][0]
            .get("baselineState")
            .is_none()
    );

    report.baseline_comparison = Some(BaselineComparison {
        issues: BTreeMap::from([(
            issue_id,
            BaselineEntry {
                state: BaselineState::Unknown,
                reason: Some("scope_changed".into()),
            },
        )]),
    });
    let unknown = sarif(&report);
    let result = &unknown["runs"][0]["results"][0];
    assert!(result.get("baselineState").is_none());
    assert_eq!(result["properties"]["baselineReason"], "scope_changed");
}

#[test]
fn sarif_emits_only_standard_baseline_state_values() {
    for (state, expected) in [
        (BaselineState::New, "new"),
        (BaselineState::Unchanged, "unchanged"),
        (BaselineState::Updated, "updated"),
        (BaselineState::Absent, "absent"),
    ] {
        let mut report = report_with_issue();
        report.baseline_comparison = Some(BaselineComparison {
            issues: BTreeMap::from([(
                report.issues[0].id.clone(),
                BaselineEntry {
                    state,
                    reason: None,
                },
            )]),
        });
        assert_eq!(
            sarif(&report)["runs"][0]["results"][0]["baselineState"],
            expected
        );
    }
}

#[test]
fn human_output_includes_language_capability_receipts() {
    let mut output = Vec::new();
    write_report(&mut output, &report_with_issue(), OutputFormat::Human).unwrap();
    let output = String::from_utf8(output).unwrap();

    assert!(output.contains("language rust: Partial (1 files, 1 functions)"));
    assert!(output.contains("capability direct_calls: Partial"));
    assert!(output.contains("unresolved_direct_call (1): one call could not be resolved"));
}
