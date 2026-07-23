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

fn has_kind(detections: &[DetectedEvidence], kind: Rule) -> bool {
    detections.iter().any(|detection| detection.kind == kind)
}

fn metric_value(detection: &DetectedEvidence, name: &str) -> Option<usize> {
    detection
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

    let detections = scan_concept_drift(&files, &options());

    let parallel = detections
        .iter()
        .find(|detection| detection.kind == Rule::ParallelImplementation)
        .expect("parallel implementation detection");
    assert_eq!(metric_value(parallel, "group.size"), Some(3));
    assert_eq!(parallel.related_locations.len(), 3);
    assert!(has_kind(&detections, Rule::ShadowedAbstraction));
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

    let detections = scan_concept_drift(&files, &options());

    assert!(
        detections
            .iter()
            .all(|detection| detection.kind != Rule::ParallelImplementation),
        "{detections:#?}"
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

    let detections = scan_concept_drift(&files, &opts);

    assert!(
        detections
            .iter()
            .all(|detection| detection.kind != Rule::ParallelImplementation),
        "{detections:#?}"
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

    let detections = scan_concept_drift(&files, &options());

    let detection = detections
        .iter()
        .find(|detection| detection.kind == Rule::DuplicateTypeShape)
        .expect("duplicate type shape detection");
    assert_eq!(metric_value(detection, "group.size"), Some(2));
    assert!(detection.message.contains("email"));
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

    let detections = scan_concept_drift(&files, &options());

    let detection = detections
        .iter()
        .find(|detection| detection.kind == Rule::DuplicateTypeShape)
        .expect("duplicate type shape detection");
    assert_eq!(metric_value(detection, "group.size"), Some(2));
    assert!(detection.message.contains("email"));
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

    let detections = scan_concept_drift(&files, &options());

    let detection = detections
        .iter()
        .find(|detection| detection.kind == Rule::ConfigKeyDrift)
        .expect("config key drift detection");
    assert_eq!(metric_value(detection, "group.size"), Some(3));
    assert!(detection.related_locations.len() >= 3);
}

#[test]
fn ignores_config_keys_inside_comments() {
    let files = vec![
        source_file("src/auth.ts", "// const token = \"AUTH_TOKEN_URL\";"),
        source_file("src/client.py", "# route = \"/api/login\""),
        source_file("src/job.ts", "// fetch(\"/api/login\");"),
    ];

    let detections = scan_concept_drift(&files, &options());

    assert!(
        detections
            .iter()
            .all(|detection| detection.kind != Rule::ConfigKeyDrift),
        "{detections:#?}"
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

    let detections = scan_concept_drift(&files, &options());

    let detection = detections
        .iter()
        .find(|detection| detection.kind == Rule::FixtureFactoryDrift)
        .expect("fixture factory drift detection");
    assert_eq!(metric_value(detection, "group.size"), Some(2));
    assert_eq!(detection.related_locations.len(), 2);
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

    let detections = scan_concept_drift(&files, &options());

    assert!(
        detections
            .iter()
            .all(|detection| detection.kind != Rule::FixtureFactoryDrift),
        "{detections:#?}"
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

    let detections = scan_concept_drift(&files, &options());

    let detection = detections
        .iter()
        .find(|detection| detection.kind == Rule::GenericBucketDrift)
        .expect("generic bucket detection");
    assert_eq!(detection.path, "src/utils");
    assert!(metric_value(detection, "group.size").unwrap_or_default() >= 4);
    assert_eq!(detection.related_locations.len(), 5);
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

    let detections = scan_concept_drift(&files, &options());

    assert!(
        detections
            .iter()
            .all(|detection| detection.kind != Rule::GenericBucketDrift),
        "{detections:#?}"
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

    let default_detections = scan_concept_drift(&files, &options());
    let mut included_options = options();
    included_options.include_test_structure = true;
    let included_detections = scan_concept_drift(&files, &included_options);

    assert!(
        default_detections
            .iter()
            .all(|detection| detection.kind != Rule::GenericBucketDrift),
        "{default_detections:#?}"
    );
    assert!(has_kind(&included_detections, Rule::GenericBucketDrift));
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

    let detections = scan_concept_drift(&files, &options());

    let detection = detections
        .iter()
        .find(|detection| detection.kind == Rule::StaleCompatibilityPath)
        .expect("stale compatibility path detection");
    assert_eq!(detection.path, "src/api/user_legacy.ts");
    assert_eq!(metric_value(detection, "group.size"), Some(3));
    assert_eq!(detection.related_locations.len(), 3);
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

    let detections = scan_concept_drift(&files, &options());

    assert!(
        detections
            .iter()
            .all(|detection| detection.kind != Rule::StaleCompatibilityPath),
        "{detections:#?}"
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

    let detections = scan_concept_drift(&files, &options());

    assert!(
        detections
            .iter()
            .all(|detection| detection.kind != Rule::StaleCompatibilityPath),
        "{detections:#?}"
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

    let detections = scan_concept_drift(&files, &options());

    assert!(
        detections
            .iter()
            .all(|detection| detection.kind != Rule::StaleCompatibilityPath),
        "{detections:#?}"
    );
}
