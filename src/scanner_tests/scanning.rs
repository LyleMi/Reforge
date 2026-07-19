use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use crate::model::{Hotspot, HotspotLevel, MetricId, Severity};

#[test]
fn coverage_ontology_has_exactly_42_static_cells() {
    let targets = coverage_targets();
    assert_eq!(targets.len(), 42);
    let unique = targets
        .iter()
        .map(|(mechanism, scope, _)| (*mechanism, *scope))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), 42);
    assert_eq!(
        targets
            .iter()
            .filter(
                |(_, _, expectation)| *expectation == crate::model::CoverageExpectation::Required
            )
            .count(),
        12
    );
    assert_eq!(
        targets
            .iter()
            .filter(|(_, _, expectation)| *expectation
                == crate::model::CoverageExpectation::IntentionallyOutOfScope)
            .count(),
        30
    );
}

#[test]
fn every_required_coverage_cell_has_a_detector() {
    let manifest = crate::detectors::manifest::detector_manifest();
    for (mechanism, scope, expectation) in coverage_targets() {
        if expectation == crate::model::CoverageExpectation::Required {
            assert!(
                manifest
                    .iter()
                    .any(|entry| entry.mechanism == mechanism && entry.entity_scope == scope),
                "missing detector for {mechanism:?}/{scope:?}"
            );
        }
    }
}

#[test]
fn syntax_failures_make_parse_dependent_coverage_partial() -> anyhow::Result<()> {
    let root = test_root("coverage-parse-failure");
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join("broken.rs"), "fn broken( {\n")?;
    let args = scan_args(root.clone(), false);
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;
    std::fs::remove_dir_all(root)?;
    assert_eq!(report.coverage_summary.parse_failures.len(), 1);
    assert!(
        !report.coverage_summary.parse_failures[0]
            .path
            .contains('\\')
    );
    assert!(report.coverage_manifest.iter().any(|cell| cell.mechanism
        == crate::model::SignalMechanism::CognitiveLoad
        && cell.entity_scope == crate::model::EntityScope::Function
        && cell.status == crate::model::CoverageStatus::PartiallyObserved));
    Ok(())
}

#[test]
fn unsupported_language_specific_detectors_make_coverage_partial() -> anyhow::Result<()> {
    let root = test_root("coverage-partial-language-support");
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join("App.java"), "public class App {}\n")?;
    let args = scan_args(root.clone(), false);
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;
    std::fs::remove_dir_all(root)?;

    let dependency_files = report
        .coverage_manifest
        .iter()
        .find(|cell| {
            cell.mechanism == crate::model::SignalMechanism::DependencyPropagation
                && cell.entity_scope == crate::model::EntityScope::File
        })
        .expect("dependency file coverage should be present");
    assert_eq!(
        dependency_files.status,
        crate::model::CoverageStatus::PartiallyObserved
    );
    assert!(
        dependency_files
            .unobservable_reasons
            .iter()
            .any(|reason| reason.contains("dependency_hub"))
    );
    Ok(())
}

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
        threshold_overrides: crate::cli::ThresholdOverrideFlags::default(),
        preset: None,
        unity: crate::cli::UnityMode::Auto,
        max_unity_assembly_dependencies: 8,
        max_unity_scene_objects: 1_000,
        max_unity_prefab_objects: 250,
        max_unity_serialized_fields: 16,
        max_unity_lifecycle_methods: 7,
        max_file_lines: 800,
        max_dir_files: 40,
        filters: crate::cli::ScanFilterArgs {
            include_hidden: false,
            include_generated,
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
        analysis_thresholds: crate::cli::AnalysisThresholdArgs {
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
        },
        config: None,
        scoring_policy: None,
        ci: crate::cli::CiArgs {
            baseline: None,
            baseline_mode: crate::cli::BaselineMode::NewOrWorse,
            show: crate::cli::BaselineShow::All,
            fail_on: None,
        },
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
        .find(|metric| metric.name.as_str() == name)
        .map(|metric| metric.value)
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
fn skips_unity_generated_directories_by_default() -> Result<()> {
    let root = test_root("skip-unity-generated");
    for directory in ["Library", "Temp", "Logs", "UserSettings", "obj"] {
        let generated = root.join(directory);
        fs::create_dir_all(&generated)?;
        fs::write(generated.join("Generated.cs"), "// TODO: ignored\n")?;
    }
    fs::create_dir_all(root.join("Assets"))?;
    fs::write(root.join("Assets/Game.cs"), "// TODO: reported\n")?;

    let findings = scan_path(&scan_args(root.clone(), false))?;
    fs::remove_dir_all(root)?;

    assert_eq!(findings.len(), 1);
    assert!(findings[0].path.ends_with("Assets/Game.cs"));
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
fn scans_and_analyzes_php_sources() -> Result<()> {
    let root = test_root("php-source");
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("src/app.php"),
        r#"<?php
function calculate_total(array $items): int {
    $total = 0;
    foreach ($items as $item) {
        $total += $item;
    }
    return $total;
}
"#,
    )?;

    let mut progress = NoopProgress;
    let report = scan_report(&scan_args(root.clone(), false), &mut progress)?;

    fs::remove_dir_all(root)?;

    assert_eq!(report.stats.source_files_scanned, 1);
    assert_eq!(report.raw_metrics.files.len(), 1);
    assert!(report.raw_metrics.files[0].path.ends_with("src/app.php"));
    assert!(report.raw_metrics.functions.iter().any(|function| {
        function.path.ends_with("src/app.php") && function.name == "calculate_total"
    }));
    Ok(())
}

#[test]
fn can_exclude_test_sources_from_scan() -> Result<()> {
    let root = test_root("exclude-tests");
    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join("__tests__"))?;
    fs::create_dir_all(root.join("tests"))?;
    fs::write(root.join("src/app.ts"), "// TODO: reported\n")?;
    fs::write(root.join("src/app.test.ts"), "// TODO: ignored\n")?;
    fs::write(root.join("__tests__/app.ts"), "// TODO: ignored\n")?;
    fs::write(root.join("tests/helper.rs"), "// TODO: ignored\n")?;

    let mut args = scan_args(root.clone(), false);
    args.filters.exclude_tests = true;
    args.include_test_similarity = true;
    args.include_test_structure = true;
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;

    fs::remove_dir_all(root)?;

    assert_eq!(report.stats.source_files_scanned, 1);
    assert_eq!(report.raw_metrics.files.len(), 1);
    assert!(report.raw_metrics.files[0].path.ends_with("src/app.ts"));
    assert!(!report.findings.iter().any(|finding| {
        finding.path.contains("__tests__")
            || finding.path.contains("/tests/")
            || finding.path.contains(".test.")
    }));
    Ok(())
}

#[test]
fn reports_directories_with_many_source_files() -> Result<()> {
    let root = test_root("large-directory");
    let source_dir = root.join("src");
    fs::create_dir_all(&source_dir)?;
    fs::write(source_dir.join("one.rs"), "pub fn one() {}\n")?;
    fs::write(source_dir.join("two.rs"), "pub fn two() {}\n")?;
    fs::write(source_dir.join("three.rs"), "pub fn three() {}\n")?;
    fs::write(source_dir.join("notes.md"), "not source\n")?;

    let mut args = scan_args(root.clone(), false);
    args.max_dir_files = 2;
    let findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, FindingKind::LargeDirectory);
    assert!(findings[0].path.ends_with("src"));
    assert_eq!(findings[0].line, None);
    assert_eq!(
        metric_value(&findings[0], "directory.source_files"),
        Some(3)
    );
    assert!(
        findings[0]
            .message
            .contains("directory contains 3 source files")
    );
    Ok(())
}

#[test]
fn reports_dependency_cycles_between_resolved_source_files() -> Result<()> {
    let root = test_root("dependency-cycle");
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("src/a.ts"),
        "import { b } from './b';\nexport const a = b;\n",
    )?;
    fs::write(
        root.join("src/b.ts"),
        "import { a } from './a';\nexport const b = a;\n",
    )?;

    let findings = scan_path(&scan_args(root.clone(), false))?;

    fs::remove_dir_all(root)?;

    let cycle = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::DependencyCycle)
        .expect("resolved dependency cycle should be reported");
    assert_eq!(metric_value(cycle, "dependency.cycle_files"), Some(2));
    assert_eq!(cycle.related_locations.len(), 2);
    assert!(
        cycle
            .related_locations
            .iter()
            .any(|location| location.path.ends_with("src/a.ts"))
    );
    assert!(
        cycle
            .related_locations
            .iter()
            .any(|location| location.path.ends_with("src/b.ts"))
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
    assert_eq!(metric_value(similar_findings[0], "group.size"), Some(3));
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
    assert!(is_test_source(Path::new("__tests__/app.ts")));
    assert!(is_test_source(Path::new("src/test_app.py")));
    assert!(is_test_source(Path::new("src/app_tests.rs")));
    assert!(is_test_source(Path::new("src/scanner_tests/scanning.rs")));
    assert!(!is_test_source(Path::new("src/app.go")));
}

#[test]
fn recognizes_frontend_module_vue_and_csharp_script_sources() {
    for path in [
        "src/app.mjs",
        "src/app.cjs",
        "src/app.mts",
        "src/app.cts",
        "src/App.vue",
        "scripts/setup.csx",
    ] {
        assert!(is_supported_source(Path::new(path)), "{path}");
    }
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
