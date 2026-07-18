use super::*;
use crate::scanner::Severity;

fn source_file(path: &str, source: &str) -> SourceFile {
    SourceFile {
        path: PathBuf::from(path),
        display_path: path.to_string(),
        source: source.into(),
    }
}

fn options() -> StructureOptions {
    StructureOptions {
        max_function_lines: 6,
        max_function_complexity: 3,
        max_nesting_depth: 2,
        max_function_parameters: 3,
        max_type_lines: 6,
        max_type_members: 3,
        max_imports: 2,
        max_public_items: 2,
        max_functions_per_file: 40,
        max_functions_per_100_lines: 12,
        max_small_function_ratio: 70,
        min_repeated_literal_occurrences: 3,
        min_data_clump_occurrences: 3,
        max_dir_files: 3,
        include_test_structure: false,
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
fn reports_rust_function_level_signals() -> Result<()> {
    let source = r#"
pub fn process(a: i32, b: i32, c: i32, d: i32) -> i32 {
    if a > 0 {
        for value in [b, c] {
            if value > 1 {
                return value;
            }
        }
    }
    d
}
"#;

    let findings = scan_structure(&[source_file("src/lib.rs", source)], &options())?;

    assert!(
        findings
            .iter()
            .any(|finding| finding.kind == FindingKind::LongFunction)
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.kind == FindingKind::ComplexFunction)
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.kind == FindingKind::DeepNesting)
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.kind == FindingKind::ManyParameters)
    );
    Ok(())
}

#[test]
fn keeps_combined_function_signals_as_constituent_evidence() -> Result<()> {
    let source = r#"
pub fn process(a: i32, b: i32, c: i32, d: i32) -> i32 {
    if a > 0 {
        for value in [b, c] {
            if value > 1 {
                return value;
            }
        }
    }
    d
}
"#;

    let findings = scan_structure(&[source_file("src/lib.rs", source)], &options())?;
    assert!(
        !findings
            .iter()
            .any(|finding| finding.kind == FindingKind::ReadabilityRisk)
    );
    for kind in [
        FindingKind::LongFunction,
        FindingKind::ComplexFunction,
        FindingKind::DeepNesting,
        FindingKind::ManyParameters,
    ] {
        assert!(findings.iter().any(|finding| finding.kind == kind));
    }
    Ok(())
}

#[test]
fn counts_rust_parameter_patterns_without_type_identifiers() -> Result<()> {
    let source = r#"
fn collect_named_functions(
    node: Node<'_>,
    extraction: CandidateExtraction<'_>,
    interner: &mut TokenInterner,
    candidates: &mut Vec<FunctionCandidate>,
) {
}
"#;
    let mut opts = options();
    opts.max_function_parameters = 4;

    let findings = scan_structure(&[source_file("src/lib.rs", source)], &opts)?;

    assert!(
        !findings
            .iter()
            .any(|finding| finding.kind == FindingKind::ManyParameters),
        "{findings:#?}"
    );
    Ok(())
}

#[test]
fn counts_typescript_parameters_without_type_annotation_identifiers() -> Result<()> {
    let source = r#"
export function transportEndpointsFromExpression(
  node: t.Expression | t.ObjectMethod,
  path: NodePath<t.Node>,
  ctx: FrameworkAdapterContext,
  depth = 0,
): FrameworkTransportEndpointInfo[] {
  return [];
}

function importBindingFromSpecifier<TParsed extends ParsedModuleFile>(
  ctx: ModuleCollectionContext<TParsed>,
  stmt: t.ImportDeclaration,
  specifier: t.ImportDeclaration['specifiers'][number],
  sourcePath: string | null,
): ModuleImport | null {
  return null;
}
"#;

    let parsed = parse_source_files(&[source_file("src/app.ts", source)])?;
    let metrics = collect_raw_structure_metrics(&parsed);
    let functions = &metrics[0].functions;

    assert_eq!(
        functions
            .iter()
            .find(|function| function.name == "transportEndpointsFromExpression")
            .map(|function| function.parameter_count),
        Some(4)
    );
    assert_eq!(
        functions
            .iter()
            .find(|function| function.name == "importBindingFromSpecifier")
            .map(|function| function.parameter_count),
        Some(4)
    );

    let mut opts = options();
    opts.max_function_parameters = 4;
    let findings = scan_structure(&[source_file("src/app.ts", source)], &opts)?;

    assert!(
        !findings
            .iter()
            .any(|finding| finding.kind == FindingKind::ManyParameters),
        "{findings:#?}"
    );
    Ok(())
}

#[test]
fn ignores_rust_enum_variants_for_large_type_member_count() -> Result<()> {
    let source = r#"
enum FindingKind {
    LargeFile,
    LargeDirectory,
    DebtMarker,
    SimilarFunctions,
    LongFunction,
}
"#;
    let mut opts = options();
    opts.max_type_lines = 20;
    opts.max_type_members = 3;

    let findings = scan_structure(&[source_file("src/model.rs", source)], &opts)?;

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::LargeType),
        "{findings:#?}"
    );
    Ok(())
}

#[test]
fn reports_typescript_module_level_signals() -> Result<()> {
    let source = r#"
import a from "a";
import b from "b";
import c from "c";
export function one() {}
export function two() {}
export function three() {}
export class BigThing {
  one() {}
  two() {}
  three() {}
  four() {}
}
"#;

    let findings = scan_structure(&[source_file("src/app.ts", source)], &options())?;

    assert!(
        findings
            .iter()
            .any(|finding| finding.kind == FindingKind::ImportHeavyFile)
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.kind == FindingKind::LargePublicSurface)
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.kind == FindingKind::LargeType)
    );
    Ok(())
}

#[test]
fn reports_rust_public_surface() -> Result<()> {
    let source = r#"
pub use crate::other::Thing;
pub const LIMIT: usize = 10;
pub struct One;
pub(crate) enum Two { A }
fn private_helper() {}
"#;

    let findings = scan_structure(&[source_file("src/lib.rs", source)], &options())?;

    let public_surface = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::LargePublicSurface)
        .expect("public Rust items should be counted");
    assert_eq!(
        metric_value(public_surface, MetricId::FilePublicItems.as_str()),
        Some(4)
    );
    Ok(())
}

#[test]
fn reports_function_proliferation_for_dense_small_functions() -> Result<()> {
    let source = r#"
fn one() -> i32 { 1 }
fn two() -> i32 { 2 }
fn three() -> i32 { 3 }
fn four() -> i32 { 4 }
fn five() -> i32 { 5 }
"#;
    let mut opts = options();
    opts.max_functions_per_file = 3;
    opts.max_functions_per_100_lines = 50;
    opts.max_small_function_ratio = 60;

    let findings = scan_structure(&[source_file("src/lib.rs", source)], &opts)?;
    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::FunctionProliferation)
        .expect("dense small functions should be reported");

    assert_eq!(metric_value(finding, "file.function_count"), Some(5));
    assert_eq!(
        metric_value(finding, "file.small_function_ratio"),
        Some(100)
    );
    Ok(())
}

#[test]
fn skips_function_proliferation_when_small_function_ratio_is_low() -> Result<()> {
    let source = r#"
fn one() -> i32 { 1 }
fn two() -> i32 { 2 }
fn three() -> i32 { 3 }
fn four() -> i32 {
    if true {
        return 4;
    }
    0
}
fn five() -> i32 {
    if true {
        return 5;
    }
    0
}
"#;
    let mut opts = options();
    opts.max_functions_per_file = 3;
    opts.max_functions_per_100_lines = 20;
    opts.max_small_function_ratio = 70;

    let findings = scan_structure(&[source_file("src/lib.rs", source)], &opts)?;

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::FunctionProliferation),
        "{findings:#?}"
    );
    Ok(())
}

#[test]
fn reports_python_repeated_literals_and_data_clumps() -> Result<()> {
    let source = r#"
def one(customer_id, account_id, region_id):
    return "shared literal"

def two(customer_id, account_id, region_id):
    return "shared literal"

def three(customer_id, account_id, region_id):
    return "shared literal"
"#;

    let findings = scan_structure(&[source_file("src/app.py", source)], &options())?;

    assert!(
        findings
            .iter()
            .any(|finding| finding.kind == FindingKind::RepeatedLiteral)
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.kind == FindingKind::DataClump)
    );
    Ok(())
}

#[test]
fn skips_report_label_repeated_literals() -> Result<()> {
    let source = r#"
def one():
    return "group_size"

def two():
    return "group_size"

def three():
    return "group_size"
"#;

    let findings = scan_structure(&[source_file("src/app.py", source)], &options())?;

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::RepeatedLiteral),
        "{findings:#?}"
    );
    Ok(())
}

#[test]
fn skips_code_convention_repeated_literals() -> Result<()> {
    let source = r#"
#[serde(rename_all = "snake_case")]
enum One {
    Value,
}

#[serde(rename_all = "snake_case")]
enum Two {
    Value,
}

#[serde(rename_all = "snake_case")]
enum Three {
    Value,
}
"#;

    let findings = scan_structure(&[source_file("src/lib.rs", source)], &options())?;

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::RepeatedLiteral),
        "{findings:#?}"
    );
    Ok(())
}

#[test]
fn skips_import_and_type_metadata_repeated_literals() -> Result<()> {
    let source = r#"
import type { NodePath } from "@babel/traverse";
import * as t from "@babel/types";

export function one(value: unknown) {
    if (typeof value === "string") return "string";
    return "object";
}

export function two(value: unknown) {
    if (typeof value === "string") return "string";
    return "object";
}

export function three(value: unknown) {
    if (typeof value === "string") return "string";
    return "object";
}

export function four(value: unknown) {
    if (typeof value === "string") return "string";
    return "object";
}
"#;

    let findings = scan_structure(&[source_file("src/app.ts", source)], &options())?;

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::RepeatedLiteral),
        "{findings:#?}"
    );
    Ok(())
}

#[test]
fn keeps_cross_file_domain_repeated_literals() -> Result<()> {
    let files = [
        source_file(
            "src/billing/a.py",
            "def one():\n    return \"tenant_enterprise_plan\"\n",
        ),
        source_file(
            "src/billing/b.py",
            "def two():\n    return \"tenant_enterprise_plan\"\n",
        ),
        source_file(
            "src/billing/c.py",
            "def three():\n    return \"tenant_enterprise_plan\"\n",
        ),
    ];

    let findings = scan_structure(&files, &options())?;
    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::RepeatedLiteral)
        .expect("domain repeated literal should be reported");

    assert_eq!(metric_value(finding, "group.size"), Some(3));
    assert!(finding.detection_reliability >= 0.80);
    Ok(())
}

#[test]
fn reports_go_repeated_error_patterns() -> Result<()> {
    let source = r#"
package app

func One() error {
    value, err := load()
    if err != nil {
        return err
    }
    return value.Close()
}

func Two() error {
    value, err := load()
    if err != nil {
        return err
    }
    return value.Close()
}

func Three() error {
    value, err := load()
    if err != nil {
        return err
    }
    return value.Close()
}
"#;

    let findings = scan_structure(&[source_file("src/app.go", source)], &options())?;

    assert!(
        findings
            .iter()
            .any(|finding| finding.kind == FindingKind::RepeatedErrorPattern)
    );
    Ok(())
}

#[test]
fn reports_new_language_function_metrics_and_long_functions() -> Result<()> {
    let cases = [
        (
            "src/App.java",
            "calculate",
            "import java.util.List;\npublic class App {\n  public int calculate(List<Item> items) {\n    int total = 0;\n    for (Item item : items) {\n      if (item.score > 10) {\n        total += item.score * 2;\n      } else {\n        total += item.score;\n      }\n    }\n    return total;\n  }\n}\n",
        ),
        (
            "src/App.cs",
            "Calculate",
            "using System.Collections.Generic;\npublic class App {\n  public int Calculate(List<Item> items) {\n    var total = 0;\n    foreach (var item in items) {\n      if (item.Score > 10) {\n        total += item.Score * 2;\n      } else {\n        total += item.Score;\n      }\n    }\n    return total;\n  }\n}\n",
        ),
        (
            "src/App.kt",
            "calculate",
            "import app.Item\nfun calculate(items: List<Item>): Int {\n  var total = 0\n  for (item in items) {\n    if (item.score > 10) {\n      total += item.score * 2\n    } else {\n      total += item.score\n    }\n  }\n  return total\n}\n",
        ),
        (
            "src/app.php",
            "calculate",
            "<?php\nuse App\\Item;\nfunction calculate(array $items): int {\n  $total = 0;\n  foreach ($items as $item) {\n    if ($item->score > 10) {\n      $total += $item->score * 2;\n    } else {\n      $total += $item->score;\n    }\n  }\n  return $total;\n}\n",
        ),
        (
            "src/app.rb",
            "calculate",
            "class App\n  def calculate(items)\n    total = 0\n    items.each do |item|\n      if item.score > 10\n        total += item.score * 2\n      else\n        total += item.score\n      end\n    end\n    total\n  end\nend\n",
        ),
    ];

    for (path, name, source) in cases {
        let file = source_file(path, source);
        let parsed = parse_source_files(std::slice::from_ref(&file))?;
        let metrics = collect_raw_structure_metrics(&parsed);
        assert_eq!(
            metrics[0]
                .functions
                .iter()
                .find(|function| function.name == name)
                .map(|function| function.parameter_count),
            Some(1),
            "{path}"
        );

        let findings = scan_structure(&[file], &options())?;
        assert!(
            findings
                .iter()
                .any(|finding| finding.kind == FindingKind::LongFunction),
            "{path}: {findings:#?}"
        );
    }

    Ok(())
}

#[test]
fn collects_csharp_constructors_and_local_functions() -> Result<()> {
    let source = r#"
class Worker {
    Worker(int seed) {
        int Normalize(int value) {
            return value + seed;
        }
        System.Console.WriteLine(Normalize(seed));
    }
}

"#;
    let file = source_file("src/Worker.cs", source);
    let parsed = parse_source_files(std::slice::from_ref(&file))?;
    let metrics = collect_raw_structure_metrics(&parsed);
    let names = metrics[0]
        .functions
        .iter()
        .map(|function| function.name.as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"Worker"), "{names:?}");
    assert!(names.contains(&"Normalize"), "{names:?}");
    Ok(())
}

#[test]
fn counts_public_csharp_types_inside_block_and_file_scoped_namespaces() -> Result<()> {
    let files = vec![
        source_file(
            "src/Cards.cs",
            "namespace Poker.Core { public enum Suit { Clubs } internal class Hidden {} }\n",
        ),
        source_file(
            "src/Runtime.cs",
            "namespace Poker.Runtime;\npublic class Controller {}\ninternal class Helper {}\n",
        ),
    ];
    let parsed = parse_source_files(&files)?;
    let metrics = collect_raw_structure_metrics(&parsed);

    assert_eq!(metrics[0].public_items, 1);
    assert_eq!(metrics[1].public_items, 1);
    Ok(())
}

#[test]
fn analyzes_vue_script_setup_with_original_line_numbers() -> Result<()> {
    let source = r#"<template>
  <button>{{ label }}</button>
</template>
<script setup lang="ts">
function buildLabel(value: string) {
    if (value.length > 10) {
        return value.trim();
    }
    return value;
}
</script>
<style scoped>button { color: red; }</style>
"#;
    let file = source_file("src/Widget.vue", source);
    let parsed = parse_source_files(std::slice::from_ref(&file))?;
    let metrics = collect_raw_structure_metrics(&parsed);
    let function = metrics[0]
        .functions
        .iter()
        .find(|function| function.name == "buildLabel")
        .expect("Vue script function should be analyzed");

    assert_eq!(function.line, 5);
    Ok(())
}

#[test]
fn skips_test_files_for_structure_by_default_but_reports_test_duplication() -> Result<()> {
    let source = r#"
test("one", () => {
  setupUserFixture();
  const label = "shared literal";
  expect(1).toBe(1);
});
test("two", () => {
  setupUserFixture();
  const label = "shared literal";
  expect(2).toBe(2);
});
test("three", () => {
  setupUserFixture();
  const label = "shared literal";
  expect(3).toBe(3);
});
test("four", () => {
  setupUserFixture();
  const label = "shared literal";
  expect(4).toBe(4);
});
test("five", () => {
  setupUserFixture();
  const label = "shared literal";
  expect(5).toBe(5);
});
"#;

    let mut opts = options();
    opts.max_imports = 0;
    let findings = scan_structure(&[source_file("tests/app.test.js", source)], &opts)?;

    assert!(
        findings
            .iter()
            .any(|finding| finding.kind == FindingKind::TestDuplication)
    );
    assert!(
        !findings
            .iter()
            .any(|finding| finding.kind == FindingKind::ImportHeavyFile)
    );

    opts.include_test_structure = true;
    let included = scan_structure(&[source_file("tests/app.test.js", source)], &opts)?;
    assert!(
        included
            .iter()
            .any(|finding| finding.kind == FindingKind::RepeatedLiteral)
    );
    Ok(())
}

#[test]
fn reports_happy_path_only_test_risk() -> Result<()> {
    let source = r#"
test("creates user", () => {
  expect(createUser("Ada").name).toBe("Ada");
});
test("updates user", () => {
  expect(updateUser("Ada").name).toBe("Ada");
});
test("loads user", () => {
  expect(loadUser("Ada").name).toBe("Ada");
});
"#;

    let findings = scan_structure(&[source_file("tests/user.test.js", source)], &options())?;
    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::HappyPathOnlyTests)
        .expect("happy-path-only test risk should be reported");

    assert_eq!(finding.severity, Severity::Info);
    assert_eq!(metric_value(finding, "group.size"), Some(3));
    assert_eq!(finding.related_locations.len(), 3);
    Ok(())
}

#[test]
fn skips_happy_path_risk_when_negative_case_is_present() -> Result<()> {
    let source = r#"
test("creates user", () => {
  expect(createUser("Ada").name).toBe("Ada");
});
test("updates user", () => {
  expect(updateUser("Ada").name).toBe("Ada");
});
test("rejects invalid user", () => {
  expect(() => createUser("")).toThrow();
});
"#;

    let findings = scan_structure(&[source_file("tests/user.test.js", source)], &options())?;

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::HappyPathOnlyTests),
        "{findings:#?}"
    );
    Ok(())
}

#[test]
fn skips_happy_path_risk_when_test_names_describe_negative_cases() -> Result<()> {
    let source = r#"
test("creates user", () => {
  expect(createUser("Ada").name).toBe("Ada");
});
test("does not emit anonymous shorthand without client evidence", () => {
  expect(extractGraphql("{}")).toEqual([]);
});
test("ignores skipped runtime entries", () => {
  expect(importRuntimeEntries([])).toEqual([]);
});
"#;

    let findings = scan_structure(&[source_file("tests/user.test.js", source)], &options())?;

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::HappyPathOnlyTests),
        "{findings:#?}"
    );
    Ok(())
}

#[test]
fn skips_rust_cfg_test_modules_for_structure_by_default() -> Result<()> {
    let source = r#"
pub fn production() -> &'static str {
    "production"
}

#[cfg(test)]
mod tests {
    fn one(customer_id: i32, account_id: i32, region_id: i32) -> &'static str {
        "shared test literal"
    }

    fn two(customer_id: i32, account_id: i32, region_id: i32) -> &'static str {
        "shared test literal"
    }

    fn three(customer_id: i32, account_id: i32, region_id: i32) -> &'static str {
        "shared test literal"
    }
}
"#;

    let findings = scan_structure(&[source_file("src/lib.rs", source)], &options())?;

    assert!(
        !findings
            .iter()
            .any(|finding| finding.kind == FindingKind::RepeatedLiteral)
    );
    assert!(
        !findings
            .iter()
            .any(|finding| finding.kind == FindingKind::DataClump)
    );

    let mut opts = options();
    opts.include_test_structure = true;
    let included = scan_structure(&[source_file("src/lib.rs", source)], &opts)?;

    assert!(
        included
            .iter()
            .any(|finding| finding.kind == FindingKind::RepeatedLiteral)
    );
    assert!(
        included
            .iter()
            .any(|finding| finding.kind == FindingKind::DataClump)
    );
    Ok(())
}

#[test]
fn reports_directory_drift() -> Result<()> {
    let files = [
        source_file("src/payments/user_invoice.rs", "fn a() {}\n"),
        source_file("src/payments/cache_token.rs", "fn b() {}\n"),
        source_file("src/payments/report_export.rs", "fn c() {}\n"),
        source_file("src/payments/email_template.rs", "fn d() {}\n"),
    ];
    let mut opts = options();
    opts.max_dir_files = 2;

    let findings = scan_structure(&files, &opts)?;

    assert!(
        findings
            .iter()
            .any(|finding| finding.kind == FindingKind::DirectoryDrift)
    );
    Ok(())
}

#[test]
fn reports_file_naming_drift_within_directory() -> Result<()> {
    let files = [
        source_file("src/payments/user_profile.rs", "fn a() {}\n"),
        source_file("src/payments/account_settings.rs", "fn b() {}\n"),
        source_file("src/payments/billingPlan.rs", "fn c() {}\n"),
        source_file("src/payments/invoice-report.rs", "fn d() {}\n"),
    ];

    let findings = scan_structure(&files, &options())?;
    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::FileNamingDrift)
        .expect("file naming drift should be reported");

    assert_eq!(finding.severity, Severity::Info);
    assert_eq!(finding.path, "src/payments");
    assert_eq!(metric_value(finding, "group.size"), Some(3));
    assert_eq!(finding.related_locations.len(), 2);
    Ok(())
}
