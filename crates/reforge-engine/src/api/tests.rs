use super::*;

#[test]
fn typed_config_defaults_and_rejects_unknown_or_removed_keys() {
    let config = Config::defaults();
    assert_eq!(config.enabled(), &BTreeSet::from([Analysis::Codebase]));
    let unknown = Config::parse_toml("version = 1\n[scope]\ninclude-hiddden = true\n").unwrap_err();
    assert!(unknown.to_string().contains("scope.include-hiddden"));
    let removed =
        Config::parse_toml("version = 1\n[analysis]\nlenses = [\"structure\"]\n").unwrap_err();
    assert_eq!(
        removed.to_string(),
        "`analysis.lenses` was removed; use `analysis.enabled`"
    );
    let removed = Config::parse_toml(
        "version = 1\n[analysis]\nenabled = [\"codebase\"]\n[structure]\nmax-file-lines = 10\n",
    )
    .unwrap_err();
    assert!(
        removed
            .to_string()
            .contains("unknown configuration key `structure`")
    );
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
        "version = 1\n[codebase]\nmax-file-lines = 5\nchurn = \"off\"\n[dataflow.search]\nmax-function-hops = 8\nmax-path-steps = 30\nmax-module-hops = 8\nmax-paths-per-source = 100\nmax-sinks-per-source = 100\nwork-budget = 10000\n[dataflow.relay]\nmin-function-hops = 4\nmin-module-hops = 2\nmin-relay-percent = 90\n[dataflow.fan-out]\nmin-sinks = 4\nmin-modules = 3\n",
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
