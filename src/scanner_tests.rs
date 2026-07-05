use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

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
