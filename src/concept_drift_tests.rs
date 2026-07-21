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

fn has_kind(findings: &[Finding], kind: FindingKind) -> bool {
    findings.iter().any(|finding| finding.kind == kind)
}

fn metric_value(finding: &Finding, name: &str) -> Option<usize> {
    finding
        .metrics
        .iter()
        .find(|metric| metric.name.as_str() == name)
        .map(|metric| metric.value)
}

#[test]
fn detects_parallel_implementations_and_shadowed_helpers() {
    let files = vec![
        source_file(
            "src/feature_a/helpers.ts",
            "export function normalizePattern(input: string) { return input.trim().toLowerCase(); }",
        ),
        source_file(
            "src/feature_b/helpers.ts",
            "export function normalizePattern(value: string) { return value.trim().toLowerCase(); }",
        ),
        source_file(
            "src/feature_c/helpers.ts",
            "export function normalizePattern(text: string) { return text.trim().toLowerCase(); }",
        ),
    ];

    let findings = scan_concept_drift(&files, &options());

    let parallel = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::ParallelImplementation)
        .expect("parallel implementation finding");
    assert_eq!(metric_value(parallel, "group.size"), Some(3));
    assert_eq!(parallel.related_locations.len(), 3);
    assert!(has_kind(&findings, FindingKind::ShadowedAbstraction));
}

#[test]
fn ignores_function_like_text_inside_string_literals() {
    let files = vec![
        source_file(
            "src/examples.rs",
            r#"
fn example() {
    let source = "export function normalizePattern(input: string) { return input; }";
}
"#,
        ),
        source_file(
            "src/normalizer.rs",
            "fn normalize_pattern(input: &str) -> &str { input }",
        ),
    ];

    let findings = scan_concept_drift(&files, &options());

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::ParallelImplementation),
        "{findings:#?}"
    );
}

#[test]
fn skips_two_cross_file_parallel_implementations_at_default_thresholds() {
    let files = vec![
        source_file(
            "src/similar_functions.rs",
            "fn adapter_for_path(path: &Path) -> Option<LanguageAdapter> { None }",
        ),
        source_file(
            "src/structural.rs",
            "fn adapter_for_path(path: &Path) -> Option<LanguageAdapter> { None }",
        ),
    ];
    let mut opts = options();
    opts.min_repeated_occurrences = 4;

    let findings = scan_concept_drift(&files, &opts);

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::ParallelImplementation),
        "{findings:#?}"
    );
}

#[test]
fn detects_duplicate_type_shapes() {
    let files = vec![
        source_file(
            "src/api/user.ts",
            "interface UserPayload {\n  id: string;\n  email: string;\n  name: string;\n  status: string;\n}",
        ),
        source_file(
            "src/jobs/user.rs",
            "struct UserRecord {\n    id: String,\n    email: String,\n    name: String,\n    status: String,\n}",
        ),
    ];

    let findings = scan_concept_drift(&files, &options());

    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::DuplicateTypeShape)
        .expect("duplicate type shape finding");
    assert_eq!(metric_value(finding, "group.size"), Some(2));
    assert!(finding.message.contains("email"));
}

#[test]
fn detects_duplicate_single_line_type_shapes() {
    let files = vec![
        source_file(
            "src/api/user.ts",
            "interface UserPayload { id: string; email: string; name: string; status: string }",
        ),
        source_file(
            "src/jobs/user.rs",
            "struct UserRecord { id: String, email: String, name: String, status: String }",
        ),
    ];

    let findings = scan_concept_drift(&files, &options());

    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::DuplicateTypeShape)
        .expect("duplicate type shape finding");
    assert_eq!(metric_value(finding, "group.size"), Some(2));
    assert!(finding.message.contains("email"));
}

#[test]
fn detects_config_key_drift() {
    let files = vec![
        source_file(
            "src/auth.ts",
            "const AUTH_TOKEN_URL = \"AUTH_TOKEN_URL\";\nconst route = \"/api/login\";",
        ),
        source_file(
            "src/client.ts",
            "const tokenUrl = process.env.AUTH_TOKEN_URL;\nfetch(\"/api/login\");",
        ),
        source_file(
            "src/job.ts",
            "let key = \"AUTH_TOKEN_URL\";\nlet route = \"/api/login\";",
        ),
    ];

    let findings = scan_concept_drift(&files, &options());

    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::ConfigKeyDrift)
        .expect("config key drift finding");
    assert_eq!(metric_value(finding, "group.size"), Some(3));
    assert!(finding.related_locations.len() >= 3);
}

#[test]
fn ignores_config_keys_inside_comments() {
    let files = vec![
        source_file("src/auth.ts", "// const token = \"AUTH_TOKEN_URL\";"),
        source_file("src/client.py", "# route = \"/api/login\""),
        source_file("src/job.ts", "// fetch(\"/api/login\");"),
    ];

    let findings = scan_concept_drift(&files, &options());

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::ConfigKeyDrift),
        "{findings:#?}"
    );
}

#[test]
fn detects_fixture_factory_drift_in_tests() {
    let files = vec![
        source_file(
            "tests/user_a.test.ts",
            "function makeUserFixture() { return { id: \"1\" }; }",
        ),
        source_file(
            "tests/user_b.test.ts",
            "function makeUserFixture() { return { id: \"2\" }; }",
        ),
    ];

    let findings = scan_concept_drift(&files, &options());

    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::FixtureFactoryDrift)
        .expect("fixture factory drift finding");
    assert_eq!(metric_value(finding, "group.size"), Some(2));
    assert_eq!(finding.related_locations.len(), 2);
}

#[test]
fn shared_fixture_factory_definition_with_import_only_callers_does_not_drift() {
    let files = vec![
        source_file(
            "tests/support/user_fixture.ts",
            "export function makeUserFixture() { return { id: 'shared' }; }",
        ),
        source_file(
            "tests/user_a.test.ts",
            "import { makeUserFixture } from './support/user_fixture'; test('a', () => makeUserFixture());",
        ),
        source_file(
            "tests/user_b.test.ts",
            "import { makeUserFixture } from './support/user_fixture'; test('b', () => makeUserFixture());",
        ),
    ];

    let findings = scan_concept_drift(&files, &options());

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::FixtureFactoryDrift),
        "{findings:#?}"
    );
}

#[test]
fn detects_generic_bucket_directories() {
    let files = vec![
        source_file(
            "src/utils/auth_token.ts",
            "export function parseAuthToken() {}",
        ),
        source_file(
            "src/utils/cache_store.ts",
            "export function buildCacheStore() {}",
        ),
        source_file(
            "src/utils/retry_policy.ts",
            "export function validateRetryPolicy() {}",
        ),
        source_file(
            "src/utils/route_mapper.ts",
            "export function mapRoutePattern() {}",
        ),
        source_file(
            "src/utils/audit_sink.ts",
            "export function writeAuditSink() {}",
        ),
    ];

    let findings = scan_concept_drift(&files, &options());

    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::GenericBucketDrift)
        .expect("generic bucket finding");
    assert_eq!(finding.path, "src/utils");
    assert!(metric_value(finding, "group.size").unwrap_or_default() >= 4);
    assert_eq!(finding.related_locations.len(), 5);
}

#[test]
fn skips_small_generic_files_with_only_a_few_concepts() {
    let files = vec![source_file(
        "src/shared/utils.ts",
        r#"
export function parseAuth() {}
export function buildAuth() {}
export function validateAuth() {}
"#,
    )];

    let findings = scan_concept_drift(&files, &options());

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::GenericBucketDrift),
        "{findings:#?}"
    );
}

#[test]
fn skips_generic_bucket_drift_in_tests_by_default() {
    let files = vec![
        source_file(
            "tests/utils/auth_token.ts",
            "export function parseAuthToken() {}",
        ),
        source_file(
            "tests/utils/cache_store.ts",
            "export function buildCacheStore() {}",
        ),
        source_file(
            "tests/utils/retry_policy.ts",
            "export function validateRetryPolicy() {}",
        ),
        source_file(
            "tests/utils/route_mapper.ts",
            "export function mapRoutePattern() {}",
        ),
    ];

    let default_findings = scan_concept_drift(&files, &options());
    let mut included_options = options();
    included_options.include_test_structure = true;
    let included_findings = scan_concept_drift(&files, &included_options);

    assert!(
        default_findings
            .iter()
            .all(|finding| finding.kind != FindingKind::GenericBucketDrift),
        "{default_findings:#?}"
    );
    assert!(has_kind(
        &included_findings,
        FindingKind::GenericBucketDrift
    ));
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

    let findings = scan_concept_drift(&files, &options());

    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::AdapterBoundaryBypass)
        .expect("adapter bypass finding");
    assert_eq!(metric_value(finding, "group.size"), Some(4));
    assert_eq!(finding.related_locations.len(), 4);
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

    let findings = scan_concept_drift(&files, &options());

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::AdapterBoundaryBypass),
        "{findings:#?}"
    );
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

    let findings = scan_concept_drift(&files, &options());

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::AdapterBoundaryBypass),
        "{findings:#?}"
    );
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

    let findings = scan_concept_drift(&files, &options());

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::AdapterBoundaryBypass),
        "{findings:#?}"
    );
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

    let findings = scan_concept_drift(&files, &options());

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::AdapterBoundaryBypass),
        "{findings:#?}"
    );
}

#[test]
fn detects_stale_compatibility_paths_without_exit_boundary() {
    let files = vec![source_file(
        "src/api/user_legacy.ts",
        r#"
export function mapLegacyUser(payload: LegacyUser) {
  if (payload.v1) {
    return fallbackUserMapper(payload);
  }
  if (payload.v2) {
    return legacyUserMapper(payload);
  }
  return mapCurrentUser(payload);
}
"#,
    )];

    let findings = scan_concept_drift(&files, &options());

    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::StaleCompatibilityPath)
        .expect("stale compatibility path finding");
    assert_eq!(finding.path, "src/api/user_legacy.ts");
    assert_eq!(metric_value(finding, "group.size"), Some(3));
    assert_eq!(finding.related_locations.len(), 3);
}

#[test]
fn skips_plain_fallback_helpers_as_stale_compatibility_paths() {
    let files = vec![source_file(
        "src/api/user_mapper.ts",
        r#"
export function fallbackUserMapper(payload: User) {
  return mapDefaultUser(payload);
}

export function loadFallbackUser(payload: User) {
  return fallbackUserMapper(payload);
}
"#,
    )];

    let findings = scan_concept_drift(&files, &options());

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::StaleCompatibilityPath),
        "{findings:#?}"
    );
}

#[test]
fn skips_compatibility_paths_with_exit_boundary() {
    let files = vec![source_file(
        "src/api/user_legacy.ts",
        r#"
// remove after mobile clients migrate to v3
export function mapLegacyUser(payload: LegacyUser) {
  return fallbackUserMapper(payload);
}
"#,
    )];

    let findings = scan_concept_drift(&files, &options());

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::StaleCompatibilityPath),
        "{findings:#?}"
    );
}

#[test]
fn skips_stale_compatibility_paths_in_tests_by_default() {
    let files = vec![source_file(
        "tests/api/user_legacy.test.ts",
        r#"
export function mapLegacyUserFixture(payload: LegacyUser) {
  return fallbackUserMapper(payload);
}
"#,
    )];

    let findings = scan_concept_drift(&files, &options());

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::StaleCompatibilityPath),
        "{findings:#?}"
    );
}
