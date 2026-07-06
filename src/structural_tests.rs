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
        .find(|metric| metric.name == name)
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

    assert_eq!(metric_value(finding, "group_size"), Some(3));
    assert!(finding.confidence >= 0.80);
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
    assert_eq!(metric_value(finding, "group_size"), Some(3));
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
    assert_eq!(metric_value(finding, "group_size"), Some(3));
    assert_eq!(finding.related_locations.len(), 2);
    Ok(())
}
