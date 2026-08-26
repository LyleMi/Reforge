use super::*;

#[test]
fn typed_config_defaults_and_rejects_unknown_or_removed_keys() {
    let config = Config::defaults();
    assert_eq!(config.enabled(), &BTreeSet::from([Analysis::Codebase]));
    let unknown = Config::parse_toml("version = 2\n[scope]\ninclude-hiddden = true\n").unwrap_err();
    assert!(unknown.to_string().contains("scope.include-hiddden"));
    let removed =
        Config::parse_toml("version = 2\n[analysis]\nlenses = [\"structure\"]\n").unwrap_err();
    assert_eq!(
        removed.to_string(),
        "`analysis.lenses` was removed; use `analysis.enabled`"
    );
    let removed = Config::parse_toml(
        "version = 2\n[analysis]\nenabled = [\"codebase\"]\n[structure]\nmax-file-lines = 10\n",
    )
    .unwrap_err();
    assert!(
        removed
            .to_string()
            .contains("unknown configuration key `structure`")
    );
}

#[test]
fn rule_selection_requires_complete_unique_non_conflicting_ids() {
    let enabled =
        Config::parse_toml("version = 2\n[rules]\nenable = [\"reforge.codebase.large_file\"]\n")
            .unwrap();
    assert!(
        enabled
            .rules
            .enabled
            .contains("reforge.codebase.large_file")
    );

    for (input, message) in [
        (
            "version = 2\n[rules]\nenable = [\"large_file\"]\n",
            "complete rule ID",
        ),
        (
            "version = 2\n[rules]\nenable = [\"reforge.codebase.not_a_rule\"]\n",
            "unknown rule",
        ),
        (
            "version = 2\n[rules]\nenable = [\"reforge.codebase.large_file\", \"reforge.codebase.large_file\"]\n",
            "duplicate rule",
        ),
        (
            "version = 2\n[rules]\nenable = [\"reforge.codebase.large_file\"]\ndisable = [\"reforge.codebase.large_file\"]\n",
            "both rules.enable and rules.disable",
        ),
        (
            "version = 2\n[rules]\nenforce = [\"reforge.codebase.large_file\"]\n",
            "only accepts stable rules",
        ),
    ] {
        assert!(
            Config::parse_toml(input)
                .unwrap_err()
                .to_string()
                .contains(message),
            "{input}"
        );
    }
}

#[test]
fn repository_dogfood_enables_every_preview_rule_without_changing_defaults() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reforge.toml");
    let dogfood = Config::parse_toml(&std::fs::read_to_string(path).unwrap()).unwrap();
    let registry = crate::detectors::manifest::rule_registry();

    assert_eq!(registry.len(), 34);
    assert_eq!(dogfood.rules.enabled.len(), registry.len());
    assert!(
        registry.iter().all(|rule| {
            rule.maturity == crate::model::RuleMaturity::Preview
                && !rule.default_enabled
                && dogfood.rules.enabled.contains(&rule.rule)
        }),
        "repository dogfood must opt into every preview/off rule"
    );
    assert!(Config::defaults().rules.enabled.is_empty());
}

#[test]
fn low_module_cohesion_thresholds_follow_presets_and_overrides() {
    for (preset, functions, percent) in [
        ("strict", 16, 40),
        ("balanced", 20, 50),
        ("relaxed", 30, 60),
    ] {
        let config =
            Config::parse_toml(&format!("version = 2\n[codebase]\npreset = \"{preset}\"\n"))
                .unwrap();
        let resolved = crate::scan::config::effective_scan_config_with(
            &crate::execution::EffectiveConfig::default(),
            Some(&config.engine),
        )
        .unwrap();
        assert_eq!(resolved.args.min_module_functions, functions);
        assert_eq!(resolved.args.min_clustered_function_percent, percent);
    }

    let config = Config::parse_toml(
        "version = 2\n[codebase]\npreset = \"strict\"\nmin-module-functions = 18\nmin-clustered-function-percent = 75\n",
    )
    .unwrap();
    let resolved = crate::scan::config::effective_scan_config_with(
        &crate::execution::EffectiveConfig::default(),
        Some(&config.engine),
    )
    .unwrap();
    assert_eq!(resolved.args.min_module_functions, 18);
    assert_eq!(resolved.args.min_clustered_function_percent, 75);
}

#[test]
fn low_module_cohesion_config_rejects_invalid_boundaries() {
    for (setting, expected) in [
        ("min-module-functions = 0", "positive integer"),
        ("min-clustered-function-percent = -1", "between 0 and 100"),
        ("min-clustered-function-percent = 101", "between 0 and 100"),
    ] {
        let error = Config::parse_toml(&format!("version = 2\n[codebase]\n{setting}\n"))
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
    }
    Config::parse_toml(
        "version = 2\n[codebase]\nmin-module-functions = 1\nmin-clustered-function-percent = 0\n",
    )
    .unwrap();
}

#[test]
fn low_module_cohesion_report_is_schema_stable_and_python_is_not_applicable() {
    let root = std::env::temp_dir().join(format!(
        "reforge-low-module-cohesion-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("monolith.js"),
        "function renderPage(){renderHeader();renderBody();}\nfunction renderHeader(){}\nfunction renderBody(){}\nfunction diffPage(){diffHeader();diffBody();}\nfunction diffHeader(){}\nfunction diffBody(){}\n",
    )
    .unwrap();
    let config = Config::parse_toml(
        "version = 2\n[rules]\nenable = [\"reforge.codebase.low_module_cohesion\"]\n[codebase]\nmin-module-functions = 6\nmin-clustered-function-percent = 100\nchurn = \"off\"\n",
    )
    .unwrap();
    let analyze_once = || {
        analyze(&AnalyzeOptions {
            root: root.clone(),
            config: config.clone(),
            reproducible: true,
            metrics_output: None,
            flow_ir_output: None,
        })
        .unwrap()
    };
    let first = analyze_once();
    let second = analyze_once();
    first.validate().unwrap();
    let serialized = serde_json::to_vec(&first).unwrap();
    let round_trip: Report = serde_json::from_slice(&serialized).unwrap();
    round_trip.validate().unwrap();
    assert_eq!(serialized, serde_json::to_vec(&second).unwrap());
    assert_eq!(first.issues.len(), 1);
    assert_eq!(first.issues[0].id, round_trip.issues[0].id);
    assert_eq!(
        first.issues[0].evidence[0].id,
        round_trip.issues[0].evidence[0].id
    );

    std::fs::remove_file(root.join("monolith.js")).unwrap();
    std::fs::write(root.join("monolith.py"), "def render_page():\n    pass\n").unwrap();
    let python = analyze_once();
    assert!(python.issues.is_empty());
    assert_eq!(
        python.coverage["codebase"].rules["reforge.codebase.low_module_cohesion"].status,
        CoverageStatus::NotApplicable
    );
    std::fs::remove_dir_all(root).unwrap();
}

fn options(root: &Path, config: &Path, enabled: BTreeSet<Analysis>) -> AnalyzeOptions {
    let mut config = Config::parse_toml(&std::fs::read_to_string(config).unwrap()).unwrap();
    config.set_enabled(enabled).unwrap();
    AnalyzeOptions {
        root: root.to_path_buf(),
        config,
        reproducible: true,
        metrics_output: None,
        flow_ir_output: None,
    }
}

#[test]
fn selected_analyses_are_isolated_and_combined_parse_is_shared() {
    let root = std::env::temp_dir().join(format!("reforge-analysis-set-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("lib.rs"),
        "fn first(x: String) { second(x); }\nfn second(x: String) { third(x); }\nfn third(x: String) { let consumed = x; }\n// filler\n// filler\n// filler\n// filler\n// filler\n// filler\n// filler\n// filler\n",
    )
    .unwrap();
    let config = root.join("engine.toml");
    std::fs::write(
        &config,
        "version = 2\n[codebase]\nmax-file-lines = 5\nchurn = \"off\"\n[dataflow.search]\nmax-function-hops = 8\nmax-path-steps = 30\nmax-module-hops = 8\nmax-paths-per-source = 100\nmax-sinks-per-source = 100\nwork-budget = 10000\n[dataflow.relay]\nmin-function-hops = 4\nmin-module-hops = 2\nmin-relay-percent = 90\n[dataflow.fan-out]\nmin-sinks = 4\nmin-modules = 3\n",
    )
    .unwrap();

    let codebase = analyze(&options(
        &root,
        &config,
        BTreeSet::from([Analysis::Codebase]),
    ))
    .unwrap();
    let dataflow = analyze(&options(
        &root,
        &config,
        BTreeSet::from([Analysis::Dataflow]),
    ))
    .unwrap();
    let combined = analyze(&options(
        &root,
        &config,
        BTreeSet::from([Analysis::Codebase, Analysis::Dataflow]),
    ))
    .unwrap();

    assert_eq!(codebase.coverage.keys().collect::<Vec<_>>(), ["codebase"]);
    assert_eq!(dataflow.coverage.keys().collect::<Vec<_>>(), ["dataflow"]);
    assert_eq!(
        combined.coverage.keys().collect::<Vec<_>>(),
        ["codebase", "dataflow"]
    );

    let independent = codebase
        .issues
        .iter()
        .chain(&dataflow.issues)
        .map(|issue| issue.id.clone())
        .collect::<BTreeSet<_>>();
    let combined_ids = combined
        .issues
        .iter()
        .map(|issue| issue.id.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(combined_ids, independent);
    assert_eq!(combined.summary.scanned_files, 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn every_registry_rule_has_exactly_one_analysis_owner() {
    let registry = rules(&BTreeSet::from([Analysis::Codebase, Analysis::Dataflow]));
    let mut rules = BTreeSet::new();
    for entry in registry {
        assert!(rules.insert(entry["rule"].as_str().unwrap().to_owned()));
        assert!(matches!(
            entry["analysis"].as_str(),
            Some("codebase" | "dataflow")
        ));
        assert!(entry["observation"]["source"].as_str().is_some());
        assert!(entry["observation"]["unit"].as_str().is_some());
        assert!(entry.get("evidence_guidance").is_none());
    }
}

#[test]
fn dataflow_retains_discovered_unsupported_languages() {
    let root =
        std::env::temp_dir().join(format!("reforge-dataflow-language-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("main.go"), "package main\nfunc main() {}\n").unwrap();

    let mut config = Config::defaults();
    config
        .set_enabled(BTreeSet::from([Analysis::Dataflow]))
        .unwrap();
    let report = analyze(&AnalyzeOptions {
        root: root.clone(),
        config,
        reproducible: true,
        metrics_output: None,
        flow_ir_output: None,
    })
    .unwrap();
    let language = &report.coverage["dataflow"].languages["go"];
    assert_eq!(language.status, CoverageStatus::Unsupported);
    assert_eq!(language.files, 1);
    assert_eq!(language.limitations[0].code, "language_unsupported");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn dataflow_unresolved_call_limitations_are_scoped_to_their_language_and_capability() {
    let root =
        std::env::temp_dir().join(format!("reforge-dataflow-receipts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("main.rs"), "fn main() { missing_function(); }\n").unwrap();
    std::fs::write(
        root.join("identity.py"),
        "def identity(value):\n    return value\n",
    )
    .unwrap();

    let mut config = Config::defaults();
    config
        .set_enabled(BTreeSet::from([Analysis::Dataflow]))
        .unwrap();
    let report = analyze(&AnalyzeOptions {
        root: root.clone(),
        config,
        reproducible: true,
        metrics_output: None,
        flow_ir_output: None,
    })
    .unwrap();
    let rust = &report.coverage["dataflow"].languages["rust"];
    let python = &report.coverage["dataflow"].languages["python"];

    assert_eq!(rust.capabilities["syntax"].status, CoverageStatus::Observed);
    assert!(rust.capabilities["syntax"].limitations.is_empty());
    assert_eq!(
        rust.capabilities["direct_calls"].status,
        CoverageStatus::Partial
    );
    assert_eq!(
        rust.capabilities["direct_calls"].limitations[0].code,
        "unresolved_direct_call"
    );
    assert_eq!(
        python.capabilities["direct_calls"].status,
        CoverageStatus::Observed
    );
    assert!(python.capabilities["direct_calls"].limitations.is_empty());
    assert!(python.capabilities["syntax"].limitations.is_empty());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn every_core_rule_has_a_specific_description() {
    let registry = rules(&BTreeSet::from([Analysis::Codebase, Analysis::Dataflow]));
    let descriptions = registry
        .iter()
        .map(|entry| entry["description"].as_str().unwrap())
        .collect::<BTreeSet<_>>();

    assert_eq!(descriptions.len(), registry.len());
    assert!(
        descriptions
            .iter()
            .all(|description| !description.contains("refactoring evidence"))
    );
}
