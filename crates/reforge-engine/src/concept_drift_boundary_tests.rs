use std::path::PathBuf;

use super::*;

fn source_file(path: &str, source: &str) -> SourceFile {
    SourceFile {
        path: PathBuf::from(path),
        display_path: path.to_string(),
        source: source.into(),
    }
}

fn options() -> ConceptDriftOptions {
    ConceptDriftOptions {
        min_repeated_occurrences: 3,
        min_data_shape_occurrences: 2,
        max_dir_files: 16,
        include_test_structure: false,
    }
}

#[test]
fn detects_adapter_boundary_bypasses_when_boundary_exists() {
    let files = vec![
        source_file(
            "src/http/client.ts",
            "export function request() { return fetch('/api/users'); }",
        ),
        source_file(
            "src/features/users.ts",
            "export function loadUsers() { return fetch('/api/users'); }",
        ),
        source_file(
            "src/jobs/sync.ts",
            "export function syncUsers() { return axios.get('/api/users'); }",
        ),
        source_file(
            "src/reports/users.ts",
            "export function reportUsers() { return fetch('/api/users'); }",
        ),
        source_file(
            "src/workers/users.ts",
            "export function refreshUsers() { return fetch('/api/users'); }",
        ),
    ];

    let detections = scan_concept_drift(&files, &options());

    let detection = detections
        .iter()
        .find(|detection| detection.kind == Rule::AdapterBoundaryBypass)
        .expect("adapter bypass detection");
    assert_eq!(
        detection
            .metrics
            .iter()
            .find(|metric| metric.name.as_str() == "group.size")
            .map(|metric| metric.value),
        Some(4)
    );
    assert_eq!(detection.related_locations.len(), 4);
}

#[test]
fn test_only_boundary_does_not_enable_production_bypass_detection() {
    let files = vec![
        source_file(
            "src/structural_tests/file_signals.rs",
            "fn file_adapter() {}",
        ),
        source_file("src/a.rs", "fn a() { std::fs::read(\"a\"); }"),
        source_file("src/b.rs", "fn b() { std::fs::read(\"b\"); }"),
        source_file("src/c.rs", "fn c() { std::fs::read(\"c\"); }"),
        source_file("src/d.rs", "fn d() { std::fs::read(\"d\"); }"),
    ];

    assert_no_boundary_bypass(&files);
}

#[test]
fn boundary_in_another_workspace_package_does_not_create_bypasses() {
    let files = vec![
        source_file(
            "tools/workflow/src/storage.rs",
            "fn save() { std::fs::write(\"state\", \"value\"); }",
        ),
        source_file("crates/engine/src/a.rs", "fn a() { std::fs::read(\"a\"); }"),
        source_file("crates/engine/src/b.rs", "fn b() { std::fs::read(\"b\"); }"),
        source_file("crates/engine/src/c.rs", "fn c() { std::fs::read(\"c\"); }"),
        source_file("crates/engine/src/d.rs", "fn d() { std::fs::read(\"d\"); }"),
    ];

    assert_no_boundary_bypass(&files);
}

#[test]
fn skips_adapter_boundary_bypasses_in_support_scripts() {
    let files = vec![
        source_file(
            "src/shared/file-utils.ts",
            "export function readFile(path: string) { return path; }",
        ),
        source_file(
            "scripts/import-fixtures.ts",
            "export function importFixtures() { return fs.readFileSync('fixtures.json', 'utf-8'); }",
        ),
        source_file(
            "scripts/export-fixtures.ts",
            "export function exportFixtures() { return fs.writeFileSync('fixtures.json', '{}'); }",
        ),
    ];

    assert_no_boundary_bypass(&files);
}

#[test]
fn skips_differential_and_oracle_operational_harnesses() {
    let files = vec![
        source_file(
            "src/shared/file-utils.ts",
            "export function readFile(path: string) { return path; }",
        ),
        source_file(
            "tools/differential.sh",
            "function compare_outputs() { fs.readFileSync('actual.json'); }",
        ),
        source_file(
            "benches/oracle.ps1",
            "function Invoke-Oracle { fs.readFileSync('expected.json'); }",
        ),
        source_file(
            "benchmarks/differential.ts",
            "export function differentialRun() { return fs.readFileSync('result.json'); }",
        ),
        source_file(
            "scripts/oracle.py",
            "def oracle_result(): return fs.readFileSync('golden.json')",
        ),
    ];

    assert_no_boundary_bypass(&files);
}

#[test]
fn skips_adapter_boundary_bypasses_in_cli_entrypoints() {
    let files = vec![
        source_file(
            "src/shared/logger.ts",
            "export function logInfo(message: string) { return message; }",
        ),
        source_file(
            "src/cli/import.ts",
            "export function importData() { console.log('import'); }",
        ),
        source_file(
            "src/cli/export.ts",
            "export function exportData() { console.log('export'); }",
        ),
        source_file(
            "src/cli/check.ts",
            "export function checkData() { console.log('check'); }",
        ),
    ];

    assert_no_boundary_bypass(&files);
}

fn assert_no_boundary_bypass(files: &[SourceFile]) {
    let detections = scan_concept_drift(files, &options());
    assert!(
        detections
            .iter()
            .all(|detection| detection.kind != Rule::AdapterBoundaryBypass),
        "{detections:#?}"
    );
}
