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
fn rust_match_arms_do_not_inflate_branch_complexity() -> Result<()> {
    let source = r#"
fn label(value: u8) -> &'static str {
    match value {
        0 => "zero",
        1 => "one",
        2 => "two",
        3 => "three",
        _ => "many",
    }
}
"#;
    let parsed = parse_source_files(&[source_file("src/lib.rs", source)])?;
    let metrics = collect_raw_structure_metrics(&parsed);

    assert_eq!(metrics[0].functions[0].complexity, 2);
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
