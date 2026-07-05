use super::*;

fn source_file(path: &str, source: &str) -> SourceFile {
    SourceFile {
        path: PathBuf::from(path),
        display_path: path.to_string(),
        source: source.to_string(),
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
