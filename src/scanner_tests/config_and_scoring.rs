#[test]
fn churn_auto_degrades_outside_git_repository() -> Result<()> {
    let root = test_root("churn-auto-no-git");
    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("src/main.rs"), "fn main() {}\n")?;

    let mut args = scan_args(root.clone(), false);
    args.churn = Some(crate::cli::ChurnMode::Auto);
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;

    fs::remove_dir_all(root)?;

    assert!(!report.summary.churn.enabled);
    assert_eq!(report.summary.churn.status, "unavailable");
    Ok(())
}

#[test]
fn churn_on_errors_outside_git_repository() -> Result<()> {
    let root = test_root("churn-on-no-git");
    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("src/main.rs"), "fn main() {}\n")?;

    let mut args = scan_args(root.clone(), false);
    args.churn = Some(crate::cli::ChurnMode::On);
    let mut progress = NoopProgress;
    let result = scan_report(&args, &mut progress);

    fs::remove_dir_all(root)?;

    assert!(result.is_err());
    Ok(())
}

#[test]
fn loads_config_and_cli_overrides_configured_values() -> Result<()> {
    let root = test_root("config");
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("reforge.toml"),
        "max-file-lines = 2\nmax-functions-per-file = 3\nmax-functions-per-100-lines = 10\nmax-small-function-ratio = 60\nchurn = \"off\"\nhotspot-model = \"static\"\nchurn-window-days = 30\n",
    )?;
    fs::write(
        root.join("src/main.rs"),
        "fn one() {}\nfn two() {}\nfn three() {}\nfn four() {}\nfn five() {}\n",
    )?;

    let mut args = scan_args(root.clone(), false);
    args.hotspot_model = Some(crate::cli::HotspotModel::Churn);
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;

    fs::remove_dir_all(root)?;

    assert_eq!(
        report.summary.hotspot_model,
        crate::cli::HotspotModel::Churn
    );
    assert_eq!(report.summary.churn.mode, crate::cli::ChurnMode::Off);
    assert_eq!(report.summary.churn.window_days, 30);
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.kind == FindingKind::LargeFile)
    );
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.kind == FindingKind::FunctionProliferation)
    );
    Ok(())
}

#[test]
fn config_ignore_paths_skip_matching_subtrees() -> Result<()> {
    let root = test_root("config-ignore-paths");
    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join("vendor"))?;
    fs::write(root.join("reforge.toml"), "ignore-paths = [\"vendor\"]\n")?;
    fs::write(root.join("src/main.rs"), "// TODO: reported\n")?;
    fs::write(root.join("vendor/ignored.rs"), "// TODO: ignored\n")?;

    let args = scan_args(root.clone(), false);
    let findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    assert_eq!(findings.len(), 1);
    assert!(findings[0].path.ends_with("src/main.rs"));
    Ok(())
}

#[test]
fn cli_ignore_paths_are_added_to_config_ignore_paths() -> Result<()> {
    let root = test_root("cli-and-config-ignore-paths");
    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join("vendor"))?;
    fs::create_dir_all(root.join("fixtures"))?;
    fs::write(root.join("reforge.toml"), "ignore-paths = [\"vendor\"]\n")?;
    fs::write(root.join("src/main.rs"), "// TODO: reported\n")?;
    fs::write(root.join("vendor/ignored.rs"), "// TODO: ignored\n")?;
    fs::write(root.join("fixtures/ignored.rs"), "// TODO: ignored\n")?;

    let mut args = scan_args(root.clone(), false);
    args.filters.ignore_paths.push("fixtures".to_string());
    let findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    assert_eq!(findings.len(), 1);
    assert!(findings[0].path.ends_with("src/main.rs"));
    Ok(())
}

#[test]
fn suppression_summary_counts_reportable_suppressed_findings() -> Result<()> {
    let root = test_root("suppression-summary");
    let debt_marker = ["TO", "DO"].concat();
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("reforge.toml"),
        "\
[[suppressions]]
kind = \"debt_marker\"
path = \"src/main.rs\"
line = 1
reason = \"tracked elsewhere\"
",
    )?;
    fs::write(
        root.join("src/main.rs"),
        format!(
            "\
// {debt_marker}: config suppressed
// {directive}:ignore-next-line debt_marker accepted generated marker
// {debt_marker}: inline suppressed
// {debt_marker}: reported
",
            directive = "reforge"
        ),
    )?;

    let mut progress = NoopProgress;
    let report = scan_report(&scan_args(root.clone(), false), &mut progress)?;

    fs::remove_dir_all(root)?;

    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].line, Some(4));
    assert_eq!(report.suppression_summary.suppressed_count, 2);
    assert_eq!(
        report
            .suppression_summary
            .suppressed_by_kind
            .get(&FindingKind::DebtMarker),
        Some(&2)
    );
    assert_eq!(
        report
            .suppression_summary
            .suppressed_by_severity
            .get(&Severity::Info),
        Some(&2)
    );
    assert!(
        report
            .suppression_summary
            .highest_suppressed_priority
            .is_some()
    );
    Ok(())
}

#[test]
fn ci_gate_uses_unsuppressed_findings_not_hotspots_or_suppressed_findings() -> Result<()> {
    let root = test_root("suppressed-gate");
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("reforge.toml"),
        "\
[[suppressions]]
kind = \"large_file\"
path = \"src/large.rs\"
line = 1
reason = \"legacy file tracked separately\"
",
    )?;
    fs::write(root.join("src/large.rs"), "// filler\n".repeat(900))?;

    let mut args = scan_args(root.clone(), false);
    args.churn = Some(crate::cli::ChurnMode::Off);
    args.hotspot_model = Some(crate::cli::HotspotModel::Static);
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;

    fs::remove_dir_all(root)?;

    assert!(report.findings.is_empty());
    assert_eq!(report.suppression_summary.suppressed_count, 1);
    assert_eq!(
        report
            .suppression_summary
            .suppressed_by_severity
            .get(&Severity::Warning),
        Some(&1)
    );
    assert!(!report.hotspots.is_empty());
    assert!(
        crate::baseline::gate_failures(report.issues.iter(), crate::cli::FailOnSeverity::Warning,)
            .is_empty()
    );
    Ok(())
}

#[test]
fn gitignore_paths_are_skipped_by_default() -> Result<()> {
    let root = test_root("gitignore-paths");
    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join("vendor"))?;
    fs::write(root.join(".gitignore"), "vendor/\n")?;
    fs::write(root.join("src/main.rs"), "// TODO: reported\n")?;
    fs::write(root.join("vendor/ignored.rs"), "// TODO: ignored\n")?;

    let args = scan_args(root.clone(), false);
    let findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    assert_eq!(findings.len(), 1);
    assert!(findings[0].path.ends_with("src/main.rs"));
    Ok(())
}

#[test]
fn can_disable_gitignore_filtering() -> Result<()> {
    let root = test_root("no-gitignore");
    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join("vendor"))?;
    fs::write(root.join(".gitignore"), "vendor/\n")?;
    fs::write(root.join("src/main.rs"), "// TODO: reported\n")?;
    fs::write(root.join("vendor/included.rs"), "// TODO: reported\n")?;

    let mut args = scan_args(root.clone(), false);
    args.filters.no_gitignore = true;
    let findings = scan_path(&args)?;

    fs::remove_dir_all(root)?;

    assert_eq!(findings.len(), 2);
    assert!(
        findings
            .iter()
            .any(|finding| finding.path.ends_with("vendor/included.rs"))
    );
    Ok(())
}

#[test]
fn metrics_summary_uses_all_raw_metrics_not_only_findings() -> Result<()> {
    let root = test_root("raw-percentiles");
    fs::create_dir_all(root.join("src"))?;
    for index in 0..6 {
        fs::write(
            root.join("src").join(format!("file_{index}.rs")),
            format!("pub fn f_{index}() {{}}\n"),
        )?;
    }

    let mut args = scan_args(root.clone(), false);
    args.max_file_lines = 100;
    args.churn = Some(crate::cli::ChurnMode::Off);
    let mut progress = NoopProgress;
    let report = scan_report(&args, &mut progress)?;

    fs::remove_dir_all(root)?;

    assert!(report.findings.is_empty());
    assert_eq!(report.raw_metrics.files.len(), 6);
    assert_eq!(report.raw_metrics.directories.len(), 1);
    assert_eq!(report.raw_metrics.directories[0].source_files, 6);
    assert_eq!(report.metrics_summary.directories["source_files"].p50, 6);
    assert_eq!(report.metrics_summary.files["loc"].p50, 1);
    Ok(())
}

#[test]
fn hotspot_models_sort_differently() {
    let raw_metrics = RawMetrics {
        directories: Vec::new(),
        files: vec![
            FileRawMetric {
                path: "src/static.rs".to_string(),
                loc: 900,
                imports: 1,
                public_items: 1,
                is_test: false,
                churn: ChurnFileMetric::default(),
            },
            FileRawMetric {
                path: "src/churn.rs".to_string(),
                loc: 10,
                imports: 1,
                public_items: 1,
                is_test: false,
                churn: ChurnFileMetric {
                    commits_touched: 12,
                    lines_added: 400,
                    lines_deleted: 100,
                    authors_count: 3,
                    recent_weighted_churn: 500,
                },
            },
        ],
        functions: Vec::new(),
        types: Vec::new(),
    };
    let summary = summarize_raw_metrics(&raw_metrics);

    let static_hotspots = rank_hotspots(
        &raw_metrics,
        &summary,
        crate::cli::HotspotModel::Static,
        StaticRiskThresholds::default(),
    );
    let churn_hotspots = rank_hotspots(
        &raw_metrics,
        &summary,
        crate::cli::HotspotModel::Churn,
        StaticRiskThresholds::default(),
    );

    assert_eq!(static_hotspots[0].path, "src/static.rs");
    assert_eq!(churn_hotspots[0].path, "src/churn.rs");
}

#[test]
fn test_metrics_do_not_enter_hotspot_leaderboard() {
    let raw_metrics = RawMetrics {
        directories: Vec::new(),
        files: vec![
            FileRawMetric {
                path: "tests/large_test.rs".to_string(),
                loc: 2_000,
                imports: 1,
                public_items: 1,
                is_test: true,
                churn: ChurnFileMetric {
                    commits_touched: 20,
                    lines_added: 2_000,
                    lines_deleted: 1_000,
                    authors_count: 2,
                    recent_weighted_churn: 3_000,
                },
            },
            FileRawMetric {
                path: "src/large.rs".to_string(),
                loc: 900,
                imports: 1,
                public_items: 1,
                is_test: false,
                churn: ChurnFileMetric::default(),
            },
        ],
        functions: vec![
            FunctionRawMetric {
                path: "tests/large_test.rs".to_string(),
                name: "large_test".to_string(),
                line: 1,
                loc: 1_500,
                complexity: 30,
                nesting_depth: 6,
                parameter_count: 0,
                is_test: true,
            },
            FunctionRawMetric {
                path: "src/large.rs".to_string(),
                name: "large".to_string(),
                line: 1,
                loc: 100,
                complexity: 1,
                nesting_depth: 0,
                parameter_count: 0,
                is_test: false,
            },
        ],
        types: Vec::new(),
    };
    let summary = summarize_raw_metrics(&raw_metrics);

    let hotspots = rank_hotspots(
        &raw_metrics,
        &summary,
        crate::cli::HotspotModel::Static,
        StaticRiskThresholds::default(),
    );

    assert!(!hotspots.is_empty());
    assert!(
        hotspots
            .iter()
            .all(|hotspot| !hotspot.path.contains("tests/"))
    );
}

#[test]
fn hotspot_static_risk_uses_effective_scan_thresholds() -> Result<()> {
    let root = test_root("hotspot-thresholds");
    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("src/lib.rs"), "fn main() {}\n".repeat(900))?;

    let mut default_args = scan_args(root.clone(), false);
    default_args.churn = Some(crate::cli::ChurnMode::Off);
    default_args.hotspot_model = Some(crate::cli::HotspotModel::Static);
    let mut progress = NoopProgress;
    let default_report = scan_report(&default_args, &mut progress)?;

    let mut loose_args = default_args.clone();
    loose_args.max_file_lines = 2_000;
    let mut progress = NoopProgress;
    let loose_report = scan_report(&loose_args, &mut progress)?;

    fs::remove_dir_all(root)?;

    assert!(
        default_report
            .hotspots
            .iter()
            .any(|hotspot| hotspot.path.ends_with("src/lib.rs"))
    );
    assert!(
        loose_report
            .hotspots
            .iter()
            .all(|hotspot| !hotspot.path.ends_with("src/lib.rs")),
        "{:#?}",
        loose_report.hotspots
    );
    Ok(())
}

#[test]
fn file_level_hotspot_only_weakly_influences_line_findings() {
    let mut findings = vec![Finding::from(FindingInput::new(
        FindingKind::RepeatedLiteral,
        "src/big.rs",
        Some(42),
        "literal is repeated",
        vec![FindingMetric::threshold(
            MetricId::GroupSize,
            4,
            4,
            "occurrences",
        )],
    ))];
    let base_priority = findings[0].priority;
    let hotspots = vec![Hotspot {
        level: HotspotLevel::File,
        path: "src/big.rs".to_string(),
        line: None,
        name: None,
        priority: 100,
        severity: Severity::Critical,
        static_risk: 100.0,
        churn_risk: 100.0,
        reason: "file churn".to_string(),
    }];

    finalize_scoring(&mut findings, &RawMetrics::default(), &hotspots);

    assert_eq!(findings[0].priority_factors.change_pressure, 50.0);
    assert!(findings[0].priority > base_priority);
    assert!(findings[0].priority < hotspots[0].priority);
}

#[test]
fn function_hotspot_takes_precedence_over_file_hotspot_for_same_line_finding() {
    let mut findings = vec![Finding::from(FindingInput::new(
        FindingKind::LongFunction,
        "src/hot.rs",
        Some(10),
        "function is long",
        vec![FindingMetric::threshold(
            MetricId::FunctionLoc,
            120,
            80,
            "lines",
        )],
    ))];
    let hotspots = vec![
        Hotspot {
            level: HotspotLevel::File,
            path: "src/hot.rs".to_string(),
            line: None,
            name: None,
            priority: 100,
            severity: Severity::Critical,
            static_risk: 100.0,
            churn_risk: 100.0,
            reason: "file churn".to_string(),
        },
        Hotspot {
            level: HotspotLevel::Function,
            path: "src/hot.rs".to_string(),
            line: Some(10),
            name: Some("hot".to_string()),
            priority: 80,
            severity: Severity::Critical,
            static_risk: 80.0,
            churn_risk: 80.0,
            reason: "function churn".to_string(),
        },
    ];

    finalize_scoring(&mut findings, &RawMetrics::default(), &hotspots);

    assert_eq!(findings[0].priority_factors.change_pressure, 80.0);
    assert!(findings[0].rank_explanation.contains("high churn pressure"));
}

#[test]
fn churn_parser_filters_binary_outside_scan_root_and_large_commits() {
    let output = "\
commit:one\tAda
10\t2\tsrc/a.rs
-\t-\tsrc/binary.bin
3\t1\tother/outside.rs
commit:two\tGrace
200\t0\tsrc/a.rs
commit:three\tAda
1\t1\tsrc/a.rs
";

    let churn = parse_git_numstat_churn(output, "src", 50);
    let metric = churn.get("src/a.rs").expect("src/a.rs should be counted");

    assert_eq!(metric.commits_touched, 2);
    assert_eq!(metric.lines_added, 11);
    assert_eq!(metric.lines_deleted, 3);
    assert_eq!(metric.authors_count, 1);
    assert!(!churn.contains_key("src/binary.bin"));
    assert!(!churn.contains_key("other/outside.rs"));
}

#[test]
fn scan_report_outputs_percent_progress_when_enabled() -> Result<()> {
    let root = test_root("percent-progress");
    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("src/one.rs"),
        "fn one() -> i32 { let value = 1; value }\n",
    )?;
    fs::write(
        root.join("src/two.rs"),
        "fn two() -> i32 { let value = 2; value }\n",
    )?;

    let mut progress = WriterProgress::new(Vec::new());
    let mut args = scan_args(root.clone(), false);
    args.min_function_tokens = 1;
    let _ = scan_report(&args, &mut progress)?;

    fs::remove_dir_all(root)?;

    let output = String::from_utf8(progress.into_inner()).unwrap();
    assert!(output.contains("[ 50%] Scanning source files (1/2)"));
    assert!(output.contains("[100%] Scanning source files (2/2)"));
    assert!(output.contains("[ 50%] Analyzing similar functions: extracting candidates (1/2)"));
    assert!(output.contains("[100%] Analyzing similar functions: extracting candidates (2/2)"));
    assert!(output.contains("[100%] Analyzing similar functions: comparing candidates (1/1)"));
    Ok(())
}
