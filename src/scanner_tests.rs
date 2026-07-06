use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use crate::model::{Hotspot, HotspotLevel, Severity};

fn test_root(name: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("reforge-{name}-{suffix}"))
}

fn scan_args(path: std::path::PathBuf, include_generated: bool) -> ScanArgs {
    ScanArgs {
        path,
        max_file_lines: 800,
        max_dir_files: 40,
        include_hidden: false,
        include_generated,
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
        min_repeated_literal_occurrences: 4,
        min_data_clump_occurrences: 3,
        include_test_structure: false,
        ignore_paths: Vec::new(),
        config: None,
        churn: None,
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
        .find(|metric| metric.name == name)
        .map(|metric| metric.value)
}

fn has_kind(findings: &[Finding], kind: FindingKind) -> bool {
    findings.iter().any(|finding| finding.kind == kind)
}

fn write_project_marker(root: &Path) -> Result<()> {
    fs::create_dir_all(root)?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("src/main.rs"), "fn main() {}\n")?;
    Ok(())
}

fn cli_flags_doc() -> &'static str {
    "--max-file-lines --max-dir-files --include-hidden --include-generated \
--min-similar-functions --min-function-tokens --function-similarity \
--include-test-similarity --max-function-lines --max-function-complexity \
--max-nesting-depth --max-function-parameters --max-type-lines \
--max-type-members --max-imports --max-public-items \
--min-repeated-literal-occurrences --min-data-clump-occurrences \
--include-test-structure --config --churn --hotspot-model \
--churn-window-days --churn-max-commit-lines --output --output-file \
--progress --color"
}

fn schema_fields_doc() -> &'static str {
    "schema_version summary stats metrics_summary raw_metrics hotspots findings \
kind severity path line metrics priority confidence priority_factors \
rank_explanation related_locations"
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
        "Metrics model covers raw metrics, findings, hotspots, priority, scoring, and confidence.\n",
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
fn skips_generated_directories_by_default() -> Result<()> {
    let root = test_root("skip-generated");
    fs::create_dir_all(root.join("node_modules/pkg"))?;
    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("node_modules/pkg/index.js"), "// TODO: ignored\n")?;
    fs::write(root.join("src/main.rs"), "// TODO: reported\n")?;

    let findings = scan_path(&scan_args(root.clone(), false))?;

    fs::remove_dir_all(root)?;

    assert_eq!(findings.len(), 1);
    assert!(findings[0].path.ends_with("src/main.rs"));
    Ok(())
}

#[test]
fn can_include_generated_directories() -> Result<()> {
    let root = test_root("include-generated");
    fs::create_dir_all(root.join("dist"))?;
    fs::write(root.join("dist/app.js"), "// TODO: reported\n")?;

    let findings = scan_path(&scan_args(root.clone(), true))?;

    fs::remove_dir_all(root)?;

    assert_eq!(findings.len(), 1);
    assert!(findings[0].path.ends_with("dist/app.js"));
    Ok(())
}

#[test]
fn reports_directories_with_many_source_files() -> Result<()> {
    let root = test_root("large-directory");
    let source_dir = root.join("src");
    fs::create_dir_all(&source_dir)?;
    fs::write(source_dir.join("one.rs"), "fn one() {}\n")?;
    fs::write(source_dir.join("two.rs"), "fn two() {}\n")?;
    fs::write(source_dir.join("three.rs"), "fn three() {}\n")?;
    fs::write(source_dir.join("notes.md"), "not source\n")?;

    let mut args = scan_args(root.clone(), false);
    args.max_dir_files = 2;
    let findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, FindingKind::LargeDirectory);
    assert!(findings[0].path.ends_with("src"));
    assert_eq!(findings[0].line, None);
    assert_eq!(metric_value(&findings[0], "directory_files"), Some(3));
    assert!(
        findings[0]
            .message
            .contains("directory contains 3 source files")
    );
    Ok(())
}

#[test]
fn excludes_generated_directories_from_source_file_counts_by_default() -> Result<()> {
    let root = test_root("directory-count-generated");
    let dist_dir = root.join("dist");
    fs::create_dir_all(&dist_dir)?;
    fs::write(dist_dir.join("one.js"), "const one = 1;\n")?;
    fs::write(dist_dir.join("two.js"), "const two = 2;\n")?;

    let mut args = scan_args(root.clone(), false);
    args.max_dir_files = 1;
    let findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    assert!(findings.is_empty());
    Ok(())
}

#[test]
fn reports_similar_functions_using_scan_thresholds() -> Result<()> {
    let root = test_root("similar-functions");
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("src/app.js"),
        r#"
function alpha(items) {
  let total = 0;
  for (const item of items) {
    if (item.score > 10) {
      total += item.score * 2;
    } else {
      total += item.score;
    }
  }
  return total;
}

function beta(records) {
  let sum = 1;
  for (const record of records) {
    if (record.score > 20) {
      sum += record.score * 2;
    } else {
      sum += record.score;
    }
  }
  return sum;
}

function gamma(rows) {
  let acc = 2;
  for (const row of rows) {
    if (row.score > 30) {
      acc += row.score * 2;
    } else {
      acc += row.score;
    }
  }
  return acc;
}
"#,
    )?;

    let mut args = scan_args(root.clone(), false);
    args.min_function_tokens = 12;
    let findings = scan_path(&args)?;

    args.min_similar_functions = 4;
    let stricter_findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    let similar_findings = findings
        .iter()
        .filter(|finding| finding.kind == FindingKind::SimilarFunctions)
        .collect::<Vec<_>>();
    assert_eq!(similar_findings.len(), 1);
    assert_eq!(metric_value(similar_findings[0], "group_size"), Some(3));
    assert!(
        stricter_findings
            .iter()
            .all(|finding| !finding.message.contains("structurally similar"))
    );
    Ok(())
}

#[test]
fn excludes_test_files_from_similarity_by_default() -> Result<()> {
    let root = test_root("exclude-test-similarity");
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("src/app_test.go"),
        r#"
package app

func Alpha(items []Item) int {
    total := 0
    for _, item := range items {
        if item.Score > 10 {
            total += item.Score * 2
        } else {
            total += item.Score
        }
    }
    return total
}

func Beta(records []Item) int {
    sum := 1
    for _, record := range records {
        if record.Score > 20 {
            sum += record.Score * 2
        } else {
            sum += record.Score
        }
    }
    return sum
}

func Gamma(rows []Item) int {
    acc := 2
    for _, row := range rows {
        if row.Score > 30 {
            acc += row.Score * 2
        } else {
            acc += row.Score
        }
    }
    return acc
}
"#,
    )?;

    let mut args = scan_args(root.clone(), false);
    args.min_function_tokens = 12;
    let default_findings = scan_path(&args)?;

    args.include_test_similarity = true;
    let included_findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    assert!(
        default_findings
            .iter()
            .all(|finding| finding.kind != FindingKind::SimilarFunctions)
    );
    assert!(
        included_findings
            .iter()
            .any(|finding| finding.kind == FindingKind::SimilarFunctions)
    );
    Ok(())
}

#[test]
fn recognizes_common_test_source_names() {
    assert!(is_test_source(Path::new("src/app_test.go")));
    assert!(is_test_source(Path::new("src/app.test.ts")));
    assert!(is_test_source(Path::new("src/app.spec.tsx")));
    assert!(is_test_source(Path::new("tests/app.rs")));
    assert!(is_test_source(Path::new("src/test_app.py")));
    assert!(is_test_source(Path::new("src/app_tests.rs")));
    assert!(!is_test_source(Path::new("src/app.go")));
}

#[test]
fn writer_progress_outputs_messages() {
    let mut progress = WriterProgress::new(Vec::new());

    progress.report("Scanning example");
    progress.report("Finished scan");

    let output = String::from_utf8(progress.into_inner()).unwrap();
    assert!(output.contains("Scanning example"));
    assert!(output.contains("Finished scan"));
}

#[test]
fn churn_auto_degrades_outside_git_repository() -> Result<()> {
    let root = test_root("churn-auto-no-git");
    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("src/main.rs"), "fn main() {}\n")?;

    let mut args = scan_args(root.clone(), false);
    args.churn = Some(crate::cli::ChurnMode::Auto);
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;

    fs::remove_dir_all(root)?;

    assert!(!report.summary.churn.enabled);
    assert_eq!(report.summary.churn.status, "unavailable");
    Ok(())
}

#[test]
fn churn_on_errors_outside_git_repository() -> Result<()> {
    let root = test_root("churn-on-no-git");
    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("src/main.rs"), "fn main() {}\n")?;

    let mut args = scan_args(root.clone(), false);
    args.churn = Some(crate::cli::ChurnMode::On);
    let mut progress = NoopProgress;
    let result = scan_report(&args, &mut progress);

    fs::remove_dir_all(root)?;

    assert!(result.is_err());
    Ok(())
}

#[test]
fn loads_config_and_cli_overrides_configured_values() -> Result<()> {
    let root = test_root("config");
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("reforge.toml"),
        "max-file-lines = 2\nchurn = \"off\"\nhotspot-model = \"static\"\nchurn-window-days = 30\n",
    )?;
    fs::write(
        root.join("src/main.rs"),
        "fn main() {\n  let value = 1;\n  dbg!(value);\n}\n",
    )?;

    let mut args = scan_args(root.clone(), false);
    args.hotspot_model = Some(crate::cli::HotspotModel::Churn);
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;

    fs::remove_dir_all(root)?;

    assert_eq!(
        report.summary.hotspot_model,
        crate::cli::HotspotModel::Churn
    );
    assert_eq!(report.summary.churn.mode, crate::cli::ChurnMode::Off);
    assert_eq!(report.summary.churn.window_days, 30);
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.kind == FindingKind::LargeFile)
    );
    Ok(())
}

#[test]
fn config_ignore_paths_skip_matching_subtrees() -> Result<()> {
    let root = test_root("config-ignore-paths");
    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join("vendor"))?;
    fs::write(root.join("reforge.toml"), "ignore-paths = [\"vendor\"]\n")?;
    fs::write(root.join("src/main.rs"), "// TODO: reported\n")?;
    fs::write(root.join("vendor/ignored.rs"), "// TODO: ignored\n")?;

    let args = scan_args(root.clone(), false);
    let findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    assert_eq!(findings.len(), 1);
    assert!(findings[0].path.ends_with("src/main.rs"));
    Ok(())
}

#[test]
fn reports_missing_project_documentation_for_project_roots() -> Result<()> {
    let root = test_root("missing-project-docs");
    write_project_marker(&root)?;
    fs::write(root.join("README.md"), "Reforge fixture\n")?;

    let mut args = scan_args(root.clone(), false);
    args.churn = Some(crate::cli::ChurnMode::Off);
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;

    fs::remove_dir_all(root)?;

    assert!(has_kind(
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
fn complete_project_documentation_suppresses_documentation_findings() -> Result<()> {
    let root = test_root("complete-project-docs");
    write_complete_docs(&root)?;

    let mut args = scan_args(root.clone(), false);
    args.churn = Some(crate::cli::ChurnMode::Off);
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;

    fs::remove_dir_all(root)?;

    assert!(report.findings.iter().all(|finding| {
        finding
            .metrics
            .iter()
            .all(|metric| metric.dimension != crate::model::MetricDimension::Documentation)
    }));
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

    let mut args = scan_args(root.clone(), false);
    args.churn = Some(crate::cli::ChurnMode::Off);
    let findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    assert!(has_kind(&findings, FindingKind::MissingDocumentationSet));
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

    let mut args = scan_args(root.clone(), false);
    args.churn = Some(crate::cli::ChurnMode::Off);
    let findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    let stale = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::StaleCliDocumentation)
        .expect("stale CLI docs should be reported");
    assert_eq!(metric_value(stale, "missing_cli_flags"), Some(26));
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

    let mut args = scan_args(root.clone(), false);
    args.churn = Some(crate::cli::ChurnMode::Off);
    let findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    let stale = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::StaleSchemaDocumentation)
        .expect("stale schema docs should be reported");
    assert!(metric_value(stale, "missing_schema_fields").unwrap() > 0);
    assert!(stale.message.contains("hotspots"));
    Ok(())
}

#[test]
fn metrics_summary_uses_all_raw_metrics_not_only_findings() -> Result<()> {
    let root = test_root("raw-percentiles");
    fs::create_dir_all(root.join("src"))?;
    for index in 0..6 {
        fs::write(
            root.join("src").join(format!("file_{index}.rs")),
            format!("fn f_{index}() {{}}\n"),
        )?;
    }

    let mut args = scan_args(root.clone(), false);
    args.max_file_lines = 100;
    args.churn = Some(crate::cli::ChurnMode::Off);
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;

    fs::remove_dir_all(root)?;

    assert!(report.findings.is_empty());
    assert_eq!(report.raw_metrics.files.len(), 6);
    assert_eq!(report.metrics_summary.files["loc"].p50, 1);
    Ok(())
}

#[test]
fn hotspot_models_sort_differently() {
    let raw_metrics = RawMetrics {
        files: vec![
            FileRawMetric {
                path: "src/static.rs".to_string(),
                loc: 900,
                imports: 1,
                public_items: 1,
                directory_source_files: 2,
                is_test: false,
                churn: ChurnFileMetric::default(),
            },
            FileRawMetric {
                path: "src/churn.rs".to_string(),
                loc: 10,
                imports: 1,
                public_items: 1,
                directory_source_files: 2,
                is_test: false,
                churn: ChurnFileMetric {
                    commits_touched: 12,
                    lines_added: 400,
                    lines_deleted: 100,
                    authors_count: 3,
                    recent_weighted_churn: 500,
                },
            },
        ],
        functions: Vec::new(),
        types: Vec::new(),
    };
    let summary = summarize_raw_metrics(&raw_metrics);

    let static_hotspots = rank_hotspots(&raw_metrics, &summary, crate::cli::HotspotModel::Static);
    let churn_hotspots = rank_hotspots(&raw_metrics, &summary, crate::cli::HotspotModel::Churn);

    assert_eq!(static_hotspots[0].path, "src/static.rs");
    assert_eq!(churn_hotspots[0].path, "src/churn.rs");
}

#[test]
fn file_level_hotspot_only_weakly_influences_line_findings() {
    let mut findings = vec![finding(FindingInput::new(
        FindingKind::RepeatedLiteral,
        "src/big.rs",
        Some(42),
        "literal is repeated",
        vec![FindingMetric::threshold("group_size", 4, 4, "occurrences")],
    ))];
    let base_priority = findings[0].priority;
    let hotspots = vec![Hotspot {
        level: HotspotLevel::File,
        path: "src/big.rs".to_string(),
        line: None,
        name: None,
        priority: 100,
        severity: Severity::Critical,
        static_risk: 100.0,
        churn_risk: 100.0,
        reason: "file churn".to_string(),
    }];

    finalize_scoring(&mut findings, &RawMetrics::default(), &hotspots);

    assert_eq!(findings[0].priority_factors.change_pressure, 50.0);
    assert!(findings[0].priority > base_priority);
    assert!(findings[0].priority < hotspots[0].priority);
}

#[test]
fn function_hotspot_takes_precedence_over_file_hotspot_for_same_line_finding() {
    let mut findings = vec![finding(FindingInput::new(
        FindingKind::LongFunction,
        "src/hot.rs",
        Some(10),
        "function is long",
        vec![FindingMetric::threshold("function_lines", 120, 80, "lines")],
    ))];
    let hotspots = vec![
        Hotspot {
            level: HotspotLevel::File,
            path: "src/hot.rs".to_string(),
            line: None,
            name: None,
            priority: 100,
            severity: Severity::Critical,
            static_risk: 100.0,
            churn_risk: 100.0,
            reason: "file churn".to_string(),
        },
        Hotspot {
            level: HotspotLevel::Function,
            path: "src/hot.rs".to_string(),
            line: Some(10),
            name: Some("hot".to_string()),
            priority: 80,
            severity: Severity::Critical,
            static_risk: 80.0,
            churn_risk: 80.0,
            reason: "function churn".to_string(),
        },
    ];

    finalize_scoring(&mut findings, &RawMetrics::default(), &hotspots);

    assert_eq!(findings[0].priority_factors.change_pressure, 80.0);
    assert!(findings[0].rank_explanation.contains("high churn pressure"));
}

#[test]
fn churn_parser_filters_binary_outside_scan_root_and_large_commits() {
    let output = "\
commit:one\tAda
10\t2\tsrc/a.rs
-\t-\tsrc/binary.bin
3\t1\tother/outside.rs
commit:two\tGrace
200\t0\tsrc/a.rs
commit:three\tAda
1\t1\tsrc/a.rs
";

    let churn = parse_git_numstat_churn(output, "src", 50);
    let metric = churn.get("src/a.rs").expect("src/a.rs should be counted");

    assert_eq!(metric.commits_touched, 2);
    assert_eq!(metric.lines_added, 11);
    assert_eq!(metric.lines_deleted, 3);
    assert_eq!(metric.authors_count, 1);
    assert!(!churn.contains_key("src/binary.bin"));
    assert!(!churn.contains_key("other/outside.rs"));
}

#[test]
fn scan_report_outputs_percent_progress_when_enabled() -> Result<()> {
    let root = test_root("percent-progress");
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("src/one.rs"),
        "fn one() -> i32 { let value = 1; value }\n",
    )?;
    fs::write(
        root.join("src/two.rs"),
        "fn two() -> i32 { let value = 2; value }\n",
    )?;

    let mut progress = WriterProgress::new(Vec::new());
    let mut args = scan_args(root.clone(), false);
    args.min_function_tokens = 1;
    let _ = scan_report(&args, &mut progress)?;

    fs::remove_dir_all(root)?;

    let output = String::from_utf8(progress.into_inner()).unwrap();
    assert!(output.contains("[ 50%] Scanning source files (1/2)"));
    assert!(output.contains("[100%] Scanning source files (2/2)"));
    assert!(output.contains("[ 50%] Analyzing similar functions: extracting candidates (1/2)"));
    assert!(output.contains("[100%] Analyzing similar functions: extracting candidates (2/2)"));
    assert!(output.contains("[100%] Analyzing similar functions: comparing candidates (1/1)"));
    Ok(())
}
