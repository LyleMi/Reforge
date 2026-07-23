use super::*;

#[test]
fn repository_guide_uses_current_cli_vocabulary() {
    let guide = include_str!("../../../AGENTS.md");
    assert!(guide.contains("--analysis codebase"));
    assert!(guide.contains("`rules`"));
    assert!(!guide.contains("--analysis structure"));
    assert!(!guide.contains("`catalog`"));
}

#[test]
fn nested_override_updates_effective_value() {
    let mut value: toml::Value = toml::from_str(default_config()).unwrap();
    apply_override(&mut value, "dataflow.search.max-path-steps=24").unwrap();
    assert_eq!(
        value_at(&value, "dataflow.search.max-path-steps").and_then(toml::Value::as_integer),
        Some(24)
    );
}

#[test]
fn baseline_identity_includes_selected_analyses() {
    let status = reforge_schema::CoverageStatus::Observed;
    assert!(status.is_observable());
}

#[test]
fn config_uses_analysis_enabled_and_rejects_removed_lenses() {
    let defaults: toml::Value = toml::from_str(default_config()).unwrap();
    assert_eq!(
        value_at(&defaults, "analysis.enabled")
            .and_then(toml::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    let removed: toml::Value =
        toml::from_str("version = 1\n[analysis]\nlenses = ['codebase']\n").unwrap();
    let error = validate_config(&removed).unwrap_err().to_string();
    assert_eq!(
        error,
        "`analysis.lenses` was removed; use `analysis.enabled`"
    );
}

#[test]
fn config_rejects_nested_unknown_keys() {
    let mut value: toml::Value = toml::from_str(default_config()).unwrap();
    apply_override(&mut value, "dataflow.search.max-path-stepz=2").unwrap();
    let error = validate_config(&value).unwrap_err().to_string();
    assert!(error.contains("dataflow.search.max-path-stepz"));
}

#[test]
fn config_rejects_invalid_nested_types_and_ranges() {
    for (override_value, expected) in [
        ("scope.include-hidden='yes'", "scope.include-hidden"),
        ("scope.ignore-paths=[1]", "scope.ignore-paths[0]"),
        ("codebase.preset='fast'", "codebase.preset"),
        (
            "codebase.function-similarity=1.2",
            "codebase.function-similarity",
        ),
        ("dataflow.fan-out.min-sinks=0", "dataflow.fan-out.min-sinks"),
    ] {
        let mut value: toml::Value = toml::from_str(default_config()).unwrap();
        apply_override(&mut value, override_value).unwrap();
        let error = validate_config(&value).unwrap_err().to_string();
        assert!(error.contains(expected), "{override_value}: {error}");
    }
}

#[test]
fn config_rejects_unknown_suppression_rules_with_location() {
    let value: toml::Value = toml::from_str(
        r#"version = 1
[[suppressions]]
rule = "reforge.codebase.not_a_rule"
path = "src/**"
reason = "test"
"#,
    )
    .unwrap();
    let error = validate_config(&value).unwrap_err().to_string();
    assert!(error.contains("suppressions[0].rule"));
    assert!(error.contains("reforge.codebase.not_a_rule"));
}

#[test]
fn discovered_config_overlays_built_in_defaults() {
    let mut defaults: toml::Value = toml::from_str(default_config()).unwrap();
    let configured: toml::Value =
        toml::from_str("version = 1\n[dataflow.search]\nmax-path-steps = 22\n").unwrap();
    merge_config(&mut defaults, configured);
    assert_eq!(
        value_at(&defaults, "dataflow.search.max-path-steps").and_then(toml::Value::as_integer),
        Some(22)
    );
    assert_eq!(
        value_at(&defaults, "codebase.max-file-lines").and_then(toml::Value::as_integer),
        Some(600)
    );
}

#[test]
fn removed_mode_and_packs_are_rejected() {
    for input in [
        "version = 1\n[dataflow]\nmode = 'observe'\n",
        "version = 1\n[packs.unity]\nmode = 'on'\n",
    ] {
        let value: toml::Value = toml::from_str(input).unwrap();
        assert!(validate_config(&value).is_err());
    }
}
