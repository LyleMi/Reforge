use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use crate::model::Severity;

fn test_root(name: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("reforge-{name}-{suffix}"))
}

fn scan_args(path: std::path::PathBuf) -> ScanArgs {
    ScanArgs {
        path,
        threshold_overrides: crate::cli::ThresholdOverrideFlags::default(),
        preset: None,
        max_file_lines: 800,
        max_dir_files: 40,
        filters: crate::cli::ScanFilterArgs {
            include_hidden: false,
            include_generated: false,
            no_gitignore: false,
            exclude_tests: false,
            ignore_paths: Vec::new(),
        },
        finding_controls: crate::cli::FindingControlArgs {
            only: None,
            exclude_detector: None,
            min_priority: None,
            severity: None,
        },
        min_similar_functions: 3,
        min_function_tokens: 80,
        function_similarity: 0.85,
        include_test_similarity: false,
        max_function_lines: 80,
        max_function_complexity: 15,
        max_nesting_depth: 4,
        max_function_parameters: 5,
        max_type_lines: 250,
        max_type_members: 30,
        max_imports: 35,
        max_public_items: 30,
        function_proliferation: crate::cli::FunctionProliferationArgs {
            max_functions_per_file: 40,
            max_functions_per_100_lines: 12,
            max_small_function_ratio: 70,
        },
        min_repeated_literal_occurrences: 4,
        min_data_clump_occurrences: 3,
        include_test_structure: false,
        config: None,
        ci: crate::cli::CiArgs {
            baseline: None,
            baseline_mode: crate::cli::BaselineMode::NewOrWorse,
            show: crate::cli::BaselineShow::All,
            fail_on: None,
        },
        churn: Some(crate::cli::ChurnMode::Off),
        hotspot_model: None,
        churn_window_days: None,
        churn_max_commit_lines: None,
        output: Some(crate::cli::OutputFormat::Human),
        output_file: None,
        progress: crate::cli::ProgressMode::Auto,
        color: crate::cli::ColorMode::Auto,
    }
}

fn metric_value(finding: &Finding, name: &str) -> Option<usize> {
    finding
        .metrics
        .iter()
        .find(|metric| metric.name.as_str() == name)
        .map(|metric| metric.value)
}

fn has_kind(findings: &[Finding], kind: FindingKind) -> bool {
    findings.iter().any(|finding| finding.kind == kind)
}

fn write_project_marker(root: &Path) -> Result<()> {
    fs::create_dir_all(root)?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"reforge\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("src/main.rs"), "fn main() {}\n")?;
    Ok(())
}

fn cli_flags_doc() -> &'static str {
    "--preset --max-file-lines --max-dir-files --include-hidden --include-generated \
--no-gitignore --exclude-tests --ignore-path --min-similar-functions --min-function-tokens --function-similarity \
--only --exclude-detector --min-priority --severity \
--include-test-similarity --max-function-lines --max-function-complexity \
--max-nesting-depth --max-function-parameters --max-type-lines \
--max-type-members --max-imports --max-public-items \
--max-functions-per-file --max-functions-per-100-lines --max-small-function-ratio \
--min-repeated-literal-occurrences --min-data-clump-occurrences \
--include-test-structure --config --churn --hotspot-model \
--baseline --baseline-mode --show --fail-on --churn-window-days --churn-max-commit-lines --output --output-file \
--progress --color"
}

fn schema_fields_doc() -> &'static str {
    "schema_version summary stats metrics_summary raw_metrics raw_metric_manifest dependency_graph hotspots suppression_summary coverage_manifest coverage_summary issues detector_manifest findings \
 id kind severity path line metrics priority detection_reliability interpretation_reliability priority_factors construct mechanism action entity_scope issue_family evidence_role constituent_kinds issue_count \
 rank_explanation recommendation related_locations"
}

fn write_complete_docs(root: &Path) -> Result<()> {
    write_project_marker(root)?;
    fs::write(
        root.join("README.md"),
        "Reforge\n\nSee docs/README.md for the maintained documentation set.\n",
    )?;
    let docs = root.join("docs");
    fs::create_dir_all(&docs)?;
    fs::write(
        docs.join("README.md"),
        "Documentation index\n\n- user guide\n- configuration\n- report schema\n- metrics model\n- detectors\n- architecture\n- contributing\n",
    )?;
    fs::write(
        docs.join("user-guide.md"),
        format!(
            "Installation and install notes.\nQuick start: run reforge scan .\nCLI command reference: {}.\nConfiguration uses reforge.toml config.\nOutput supports json yaml report formats.\nTroubleshooting and troubleshoot debug notes.\n",
            cli_flags_doc()
        ),
    )?;
    fs::write(
        docs.join("configuration.md"),
        format!(
            "Configuration reference for reforge.toml config.\n{}\n",
            cli_flags_doc()
        ),
    )?;
    fs::write(
        docs.join("report-schema.md"),
        format!(
            "Report schema fields and compatibility.\n{}\n",
            schema_fields_doc()
        ),
    )?;
    fs::write(
        docs.join("metrics-model.md"),
        "Metrics model covers raw metrics, findings, hotspots, priority, scoring, detection reliability, and interpretation reliability.\n",
    )?;
    fs::write(
        docs.join("detectors.md"),
        "Detector goals, inputs, false-positive boundaries, and tuning advice.\n",
    )?;
    fs::write(
        docs.join("architecture.md"),
        "Architecture covers scan pipeline, detector boundaries, data flow, and extension points.\n",
    )?;
    fs::write(
        docs.join("contributing.md"),
        "Contributing covers development, testing, CI, and release checks.\n",
    )?;
    Ok(())
}

#[test]
fn reports_missing_project_documentation_for_project_roots() -> Result<()> {
    let root = test_root("missing-project-docs");
    write_project_marker(&root)?;
    fs::write(root.join("README.md"), "Reforge fixture\n")?;

    let args = scan_args(root.clone());
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;

    fs::remove_dir_all(root)?;

    assert!(!has_kind(
        &report.findings,
        FindingKind::MissingDocumentationSet
    ));
    assert!(has_kind(
        &report.findings,
        FindingKind::MissingReportSchemaDocs
    ));
    assert!(has_kind(
        &report.findings,
        FindingKind::MissingMetricsModelDocs
    ));
    assert!(has_kind(
        &report.findings,
        FindingKind::MissingArchitectureDocs
    ));
    assert!(
        report
            .findings
            .iter()
            .filter(|finding| matches!(
                finding.kind,
                FindingKind::MissingDocumentationSet
                    | FindingKind::MissingReportSchemaDocs
                    | FindingKind::MissingMetricsModelDocs
                    | FindingKind::MissingArchitectureDocs
            ))
            .all(|finding| finding.severity == Severity::Warning)
    );
    Ok(())
}

#[test]
fn skips_reforge_documentation_contract_for_other_projects() -> Result<()> {
    let root = test_root("other-project-docs");
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"formatter\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    fs::write(root.join("README.md"), "Formatter fixture\n")?;
    fs::write(root.join("src/main.rs"), "fn main() {}\n")?;

    let args = scan_args(root.clone());
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;

    fs::remove_dir_all(root)?;

    assert!(
        report
            .findings
            .iter()
            .all(|finding| { finding.mechanism != crate::model::SignalMechanism::KnowledgeDrift })
    );
    Ok(())
}

#[test]
fn complete_project_documentation_suppresses_documentation_findings() -> Result<()> {
    let root = test_root("complete-project-docs");
    write_complete_docs(&root)?;

    let args = scan_args(root.clone());
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;

    fs::remove_dir_all(root)?;

    assert!(
        report
            .findings
            .iter()
            .all(|finding| { finding.mechanism != crate::model::SignalMechanism::KnowledgeDrift })
    );
    Ok(())
}

#[test]
fn readme_only_project_is_not_treated_as_complete_documentation() -> Result<()> {
    let root = test_root("readme-only-docs");
    write_project_marker(&root)?;
    fs::write(
        root.join("README.md"),
        "Install and quick start. Run reforge scan . with --output json.\n",
    )?;

    let args = scan_args(root.clone());
    let findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    assert!(!has_kind(&findings, FindingKind::MissingDocumentationSet));
    assert!(has_kind(&findings, FindingKind::MissingUserGuide));
    Ok(())
}

#[test]
fn reports_stale_cli_documentation_when_flags_are_missing() -> Result<()> {
    let root = test_root("stale-cli-docs");
    write_complete_docs(&root)?;
    fs::write(
        root.join("docs/user-guide.md"),
        "Installation install.\nQuick start run reforge scan .\nCLI command reference: --output --progress.\nConfiguration config reforge.toml.\nOutput json yaml report.\nTroubleshooting troubleshoot debug.\n",
    )?;
    fs::write(
        root.join("docs/configuration.md"),
        "Configuration reference without CLI flags.\n",
    )?;

    let args = scan_args(root.clone());
    let findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    let stale = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::StaleCliDocumentation)
        .expect("stale CLI docs should be reported");
    let documented_flags = 2;
    let expected_missing = cli_flags_doc().split_whitespace().count() - documented_flags;
    assert_eq!(
        metric_value(stale, "documentation.missing_cli_flags"),
        Some(expected_missing)
    );
    assert!(stale.message.contains("--max-file-lines"));
    Ok(())
}

#[test]
fn reports_stale_schema_documentation_when_fields_are_missing() -> Result<()> {
    let root = test_root("stale-schema-docs");
    write_complete_docs(&root)?;
    fs::write(
        root.join("docs/report-schema.md"),
        "Report schema fields: schema_version summary stats findings kind severity path.\n",
    )?;

    let args = scan_args(root.clone());
    let findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    let stale = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::StaleSchemaDocumentation)
        .expect("stale schema docs should be reported");
    assert!(metric_value(stale, "documentation.missing_schema_fields").unwrap() > 0);
    assert!(stale.message.contains("hotspots"));
    Ok(())
}
