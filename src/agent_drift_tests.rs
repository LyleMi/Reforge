use std::path::PathBuf;

use super::*;

fn source_file(path: &str, source: &str) -> SourceFile {
    SourceFile {
        path: PathBuf::from(path),
        display_path: path.to_string(),
        source: source.into(),
    }
}

fn options() -> AgentDriftOptions {
    AgentDriftOptions {
        min_repeated_occurrences: 3,
        min_data_shape_occurrences: 2,
        max_dir_files: 16,
        include_test_structure: false,
    }
}

fn has_kind(findings: &[Finding], kind: FindingKind) -> bool {
    findings.iter().any(|finding| finding.kind == kind)
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
    ];

    let findings = scan_agent_drift(&files, &options());

    let parallel = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::ParallelImplementation)
        .expect("parallel implementation finding");
    assert_eq!(parallel.magnitude, Some(2));
    assert_eq!(parallel.related_locations.len(), 2);
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

    let findings = scan_agent_drift(&files, &options());

    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::ParallelImplementation),
        "{findings:#?}"
    );
}

#[test]
fn reports_two_cross_file_parallel_implementations_at_default_thresholds() {
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

    let findings = scan_agent_drift(&files, &opts);

    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::ParallelImplementation)
        .expect("parallel implementation finding");
    assert_eq!(finding.magnitude, Some(2));
    assert_eq!(finding.related_locations.len(), 2);
    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != FindingKind::ShadowedAbstraction),
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

    let findings = scan_agent_drift(&files, &options());

    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::DuplicateTypeShape)
        .expect("duplicate type shape finding");
    assert_eq!(finding.magnitude, Some(2));
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

    let findings = scan_agent_drift(&files, &options());

    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::DuplicateTypeShape)
        .expect("duplicate type shape finding");
    assert_eq!(finding.magnitude, Some(2));
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

    let findings = scan_agent_drift(&files, &options());

    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::ConfigKeyDrift)
        .expect("config key drift finding");
    assert_eq!(finding.magnitude, Some(3));
    assert!(finding.related_locations.len() >= 3);
}

#[test]
fn ignores_config_keys_inside_comments() {
    let files = vec![
        source_file("src/auth.ts", "// const token = \"AUTH_TOKEN_URL\";"),
        source_file("src/client.py", "# route = \"/api/login\""),
        source_file("src/job.ts", "// fetch(\"/api/login\");"),
    ];

    let findings = scan_agent_drift(&files, &options());

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

    let findings = scan_agent_drift(&files, &options());

    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::FixtureFactoryDrift)
        .expect("fixture factory drift finding");
    assert_eq!(finding.magnitude, Some(2));
    assert_eq!(finding.related_locations.len(), 2);
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
    ];

    let findings = scan_agent_drift(&files, &options());

    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::GenericBucketDrift)
        .expect("generic bucket finding");
    assert_eq!(finding.path, "src/utils");
    assert!(finding.magnitude.unwrap_or_default() >= 4);
    assert_eq!(finding.related_locations.len(), 4);
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

    let default_findings = scan_agent_drift(&files, &options());
    let mut included_options = options();
    included_options.include_test_structure = true;
    let included_findings = scan_agent_drift(&files, &included_options);

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
    ];

    let findings = scan_agent_drift(&files, &options());

    let finding = findings
        .iter()
        .find(|finding| finding.kind == FindingKind::AdapterBoundaryBypass)
        .expect("adapter bypass finding");
    assert_eq!(finding.magnitude, Some(2));
    assert_eq!(finding.related_locations.len(), 2);
}
