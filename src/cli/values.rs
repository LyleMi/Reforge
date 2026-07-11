use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Human,
    Html,
    Json,
    Sarif,
    Yaml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ConfigOutputFormat {
    Human,
    Json,
    Yaml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BaselineMode {
    New,
    NewOrWorse,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BaselineShow {
    New,
    NewOrWorse,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum FailOnSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum FindingSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ProgressMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThresholdPreset {
    Strict,
    #[default]
    Balanced,
    Relaxed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChurnMode {
    Auto,
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotspotModel {
    Static,
    Churn,
    Hybrid,
}

impl ScanArgs {
    pub fn defaults_for_path(path: PathBuf) -> Self {
        let thresholds = ThresholdSettings::BALANCED;
        Self {
            path,
            threshold_overrides: ThresholdOverrideFlags::default(),
            preset: None,
            max_file_lines: thresholds.file.max_file_lines,
            max_dir_files: thresholds.file.max_dir_files,
            filters: ScanFilterArgs::default(),
            finding_controls: FindingControlArgs::default(),
            min_similar_functions: thresholds.similarity.min_similar_functions,
            min_function_tokens: thresholds.similarity.min_function_tokens,
            function_similarity: thresholds.similarity.function_similarity,
            include_test_similarity: false,
            max_function_lines: thresholds.structure.max_function_lines,
            max_function_complexity: thresholds.structure.max_function_complexity,
            max_nesting_depth: thresholds.structure.max_nesting_depth,
            max_function_parameters: thresholds.structure.max_function_parameters,
            max_type_lines: thresholds.structure.max_type_lines,
            max_type_members: thresholds.structure.max_type_members,
            max_imports: thresholds.structure.max_imports,
            max_public_items: thresholds.structure.max_public_items,
            function_proliferation: FunctionProliferationArgs::default(),
            min_repeated_literal_occurrences: thresholds
                .repetition
                .min_repeated_literal_occurrences,
            min_data_clump_occurrences: thresholds.repetition.min_data_clump_occurrences,
            include_test_structure: false,
            config: None,
            ci: CiArgs::default(),
            churn: None,
            hotspot_model: None,
            churn_window_days: None,
            churn_max_commit_lines: None,
            output: None,
            output_file: None,
            progress: ProgressMode::Auto,
            color: ColorMode::Auto,
        }
    }

    pub fn output_format(&self) -> OutputFormat {
        self.output
            .unwrap_or_else(|| match self.output_file_extension() {
                Some(extension) if extension.eq_ignore_ascii_case("html") => OutputFormat::Html,
                Some(extension) if extension.eq_ignore_ascii_case("htm") => OutputFormat::Html,
                Some(extension) if extension.eq_ignore_ascii_case("json") => OutputFormat::Json,
                Some(extension) if extension.eq_ignore_ascii_case("sarif") => OutputFormat::Sarif,
                Some(extension)
                    if extension.eq_ignore_ascii_case("yaml")
                        || extension.eq_ignore_ascii_case("yml") =>
                {
                    OutputFormat::Yaml
                }
                _ => OutputFormat::Human,
            })
    }

    fn output_file_extension(&self) -> Option<&str> {
        self.output_file
            .as_ref()
            .and_then(|path| path.extension())
            .and_then(|extension| extension.to_str())
    }
}

impl Cli {
    pub fn parse_with_explicit_overrides() -> Self {
        let matches = Self::command().get_matches();
        Self::from_arg_matches_with_explicit_overrides(&matches)
    }

    #[cfg(test)]
    pub fn parse_from_with_explicit_overrides<I, T>(itr: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let matches = Self::command().get_matches_from(itr);
        Self::from_arg_matches_with_explicit_overrides(&matches)
    }

    fn from_arg_matches_with_explicit_overrides(matches: &ArgMatches) -> Self {
        let mut cli = Self::from_arg_matches(matches).unwrap_or_else(|error| error.exit());
        if let Command::Scan(args) = &mut cli.command
            && let Some(("scan", scan_matches)) = matches.subcommand()
        {
            args.threshold_overrides = ThresholdOverrideFlags::from_arg_matches(scan_matches);
        }
        cli
    }
}

impl Default for ScanArgs {
    fn default() -> Self {
        Self::defaults_for_path(PathBuf::from("."))
    }
}

impl Default for FunctionProliferationArgs {
    fn default() -> Self {
        let thresholds = ThresholdSettings::BALANCED;
        Self {
            max_functions_per_file: thresholds.structure.max_functions_per_file,
            max_functions_per_100_lines: thresholds.structure.max_functions_per_100_lines,
            max_small_function_ratio: thresholds.structure.max_small_function_ratio,
        }
    }
}

impl Default for CiArgs {
    fn default() -> Self {
        Self {
            baseline: None,
            baseline_mode: BaselineMode::NewOrWorse,
            show: BaselineShow::All,
            fail_on: None,
        }
    }
}

impl FailOnSeverity {
    pub fn matches(self, severity: crate::model::Severity) -> bool {
        match self {
            Self::Info => true,
            Self::Warning => matches!(
                severity,
                crate::model::Severity::Warning | crate::model::Severity::Critical
            ),
            Self::Critical => severity == crate::model::Severity::Critical,
        }
    }
}

impl ThresholdPreset {
    pub fn thresholds(self) -> ThresholdSettings {
        match self {
            Self::Strict => ThresholdSettings::STRICT,
            Self::Balanced => ThresholdSettings::BALANCED,
            Self::Relaxed => ThresholdSettings::RELAXED,
        }
    }
}

impl ThresholdOverrideFlags {
    fn from_arg_matches(matches: &ArgMatches) -> Self {
        Self {
            max_file_lines: was_command_line_value(matches, "max_file_lines"),
            max_dir_files: was_command_line_value(matches, "max_dir_files"),
            min_similar_functions: was_command_line_value(matches, "min_similar_functions"),
            min_function_tokens: was_command_line_value(matches, "min_function_tokens"),
            function_similarity: was_command_line_value(matches, "function_similarity"),
            max_function_lines: was_command_line_value(matches, "max_function_lines"),
            max_function_complexity: was_command_line_value(matches, "max_function_complexity"),
            max_nesting_depth: was_command_line_value(matches, "max_nesting_depth"),
            max_function_parameters: was_command_line_value(matches, "max_function_parameters"),
            max_type_lines: was_command_line_value(matches, "max_type_lines"),
            max_type_members: was_command_line_value(matches, "max_type_members"),
            max_imports: was_command_line_value(matches, "max_imports"),
            max_public_items: was_command_line_value(matches, "max_public_items"),
            max_functions_per_file: was_command_line_value(matches, "max_functions_per_file"),
            max_functions_per_100_lines: was_command_line_value(
                matches,
                "max_functions_per_100_lines",
            ),
            max_small_function_ratio: was_command_line_value(matches, "max_small_function_ratio"),
            min_repeated_literal_occurrences: was_command_line_value(
                matches,
                "min_repeated_literal_occurrences",
            ),
            min_data_clump_occurrences: was_command_line_value(
                matches,
                "min_data_clump_occurrences",
            ),
        }
    }
}

fn was_command_line_value(matches: &ArgMatches, id: &str) -> bool {
    matches.value_source(id) == Some(ValueSource::CommandLine)
}

impl FindingSeverity {
    pub fn matches(self, severity: crate::model::Severity) -> bool {
        match self {
            Self::Info => true,
            Self::Warning => matches!(
                severity,
                crate::model::Severity::Warning | crate::model::Severity::Critical
            ),
            Self::Critical => severity == crate::model::Severity::Critical,
        }
    }
}

impl ProgressMode {
    pub fn enabled(self, stderr_is_tty: bool) -> bool {
        match self {
            ProgressMode::Auto => stderr_is_tty,
            ProgressMode::Always => true,
            ProgressMode::Never => false,
        }
    }
}

impl ColorMode {
    pub fn enabled(self, stdout_is_tty: bool) -> bool {
        match self {
            ColorMode::Auto => stdout_is_tty,
            ColorMode::Always => true,
            ColorMode::Never => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_args_from(cli: Cli) -> ScanArgs {
        match cli.command {
            Command::Scan(args) => *args,
            other => panic!("expected scan command, got {other:?}"),
        }
    }

    #[test]
    fn parses_init_command() {
        let cli = Cli::parse_from(["reforge", "init", "config/reforge.toml", "--force"]);

        match cli.command {
            Command::Init(args) => {
                assert_eq!(args.path, PathBuf::from("config/reforge.toml"));
                assert!(args.force);
            }
            other => panic!("expected init command, got {other:?}"),
        }
    }

    #[test]
    fn parses_config_validate_command() {
        let cli = Cli::parse_from([
            "reforge",
            "config",
            "validate",
            "src",
            "--config",
            "reforge.toml",
        ]);

        match cli.command {
            Command::Config(args) => match args.command {
                ConfigCommand::Validate(validate) => {
                    assert_eq!(validate.path, PathBuf::from("src"));
                    assert_eq!(validate.config, Some(PathBuf::from("reforge.toml")));
                }
                other => panic!("expected config validate command, got {other:?}"),
            },
            other => panic!("expected config command, got {other:?}"),
        }
    }

    #[test]
    fn parses_config_show_command() {
        let cli = Cli::parse_from(["reforge", "config", "show", ".", "--output", "json"]);

        match cli.command {
            Command::Config(args) => match args.command {
                ConfigCommand::Show(show) => {
                    assert_eq!(show.path, PathBuf::from("."));
                    assert_eq!(show.output, ConfigOutputFormat::Json);
                }
                other => panic!("expected config show command, got {other:?}"),
            },
            other => panic!("expected config command, got {other:?}"),
        }
    }

    #[test]
    fn parses_threshold_preset() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--preset", "strict"]);

        let args = scan_args_from(cli);
        assert_eq!(args.preset, Some(ThresholdPreset::Strict));
    }

    #[test]
    fn parses_similar_function_thresholds() {
        let cli = Cli::parse_from([
            "reforge",
            "scan",
            ".",
            "--min-similar-functions",
            "4",
            "--min-function-tokens",
            "25",
            "--function-similarity",
            "0.9",
        ]);

        let args = scan_args_from(cli);
        assert_eq!(args.min_similar_functions, 4);
        assert_eq!(args.min_function_tokens, 25);
        assert_eq!(args.function_similarity, 0.9);
    }

    #[test]
    fn uses_stricter_default_similarity_thresholds() {
        let cli = Cli::parse_from(["reforge", "scan", "."]);

        let args = scan_args_from(cli);
        assert_eq!(args.min_function_tokens, 80);
        assert_eq!(args.function_similarity, 0.85);
        assert!(!args.include_test_similarity);
    }

    #[test]
    fn parses_test_similarity_flag() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--include-test-similarity"]);

        let args = scan_args_from(cli);
        assert!(args.include_test_similarity);
    }

    #[test]
    fn parses_structure_thresholds() {
        let cli = Cli::parse_from([
            "reforge",
            "scan",
            ".",
            "--max-function-lines",
            "60",
            "--max-function-complexity",
            "10",
            "--max-nesting-depth",
            "3",
            "--max-function-parameters",
            "4",
            "--max-type-lines",
            "120",
            "--max-type-members",
            "20",
            "--max-imports",
            "12",
            "--max-public-items",
            "8",
            "--max-functions-per-file",
            "24",
            "--max-functions-per-100-lines",
            "10",
            "--max-small-function-ratio",
            "65",
            "--min-repeated-literal-occurrences",
            "5",
            "--min-data-clump-occurrences",
            "4",
            "--include-test-structure",
        ]);

        let args = scan_args_from(cli);
        assert_eq!(args.max_function_lines, 60);
        assert_eq!(args.max_function_complexity, 10);
        assert_eq!(args.max_nesting_depth, 3);
        assert_eq!(args.max_function_parameters, 4);
        assert_eq!(args.max_type_lines, 120);
        assert_eq!(args.max_type_members, 20);
        assert_eq!(args.max_imports, 12);
        assert_eq!(args.max_public_items, 8);
        assert_eq!(args.function_proliferation.max_functions_per_file, 24);
        assert_eq!(args.function_proliferation.max_functions_per_100_lines, 10);
        assert_eq!(args.function_proliferation.max_small_function_ratio, 65);
        assert_eq!(args.min_repeated_literal_occurrences, 5);
        assert_eq!(args.min_data_clump_occurrences, 4);
        assert!(args.include_test_structure);
    }

    #[test]
    fn uses_default_structure_thresholds() {
        let cli = Cli::parse_from(["reforge", "scan", "."]);

        let args = scan_args_from(cli);
        assert_eq!(args.preset, None);
        assert_eq!(args.max_function_lines, 80);
        assert_eq!(args.max_function_complexity, 15);
        assert_eq!(args.max_nesting_depth, 4);
        assert_eq!(args.max_function_parameters, 5);
        assert_eq!(args.max_type_lines, 250);
        assert_eq!(args.max_type_members, 30);
        assert_eq!(args.max_imports, 35);
        assert_eq!(args.max_public_items, 30);
        assert_eq!(args.function_proliferation.max_functions_per_file, 40);
        assert_eq!(args.function_proliferation.max_functions_per_100_lines, 12);
        assert_eq!(args.function_proliferation.max_small_function_ratio, 70);
        assert_eq!(args.min_repeated_literal_occurrences, 12);
        assert_eq!(args.min_data_clump_occurrences, 4);
        assert!(!args.include_test_structure);
    }

    #[test]
    fn parses_scan_ignore_options() {
        let cli = Cli::parse_from([
            "reforge",
            "scan",
            ".",
            "--ignore-path",
            "vendor",
            "--ignore-path",
            "generated/snapshots",
            "--no-gitignore",
            "--exclude-tests",
        ]);

        let args = scan_args_from(cli);
        assert_eq!(args.filters.ignore_paths, ["vendor", "generated/snapshots"]);
        assert!(args.filters.no_gitignore);
        assert!(args.filters.exclude_tests);
    }

    #[test]
    fn parses_finding_control_options() {
        let cli = Cli::parse_from([
            "reforge",
            "scan",
            ".",
            "--only",
            "large_file,debt_marker",
            "--exclude-detector",
            "similar_functions",
            "--min-priority",
            "35",
            "--severity",
            "warning",
        ]);

        let args = scan_args_from(cli);
        assert_eq!(
            args.finding_controls.only,
            Some("large_file,debt_marker".to_string())
        );
        assert_eq!(
            args.finding_controls.exclude_detector,
            Some("similar_functions".to_string())
        );
        assert_eq!(args.finding_controls.min_priority, Some(35));
        assert_eq!(
            args.finding_controls.severity,
            Some(FindingSeverity::Warning)
        );
    }

    #[test]
    fn parses_output_format() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output", "json"]);

        let args = scan_args_from(cli);
        assert_eq!(args.output, Some(OutputFormat::Json));
        assert_eq!(args.output_format(), OutputFormat::Json);
    }

    #[test]
    fn parses_yaml_output_format() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output", "yaml"]);

        let args = scan_args_from(cli);
        assert_eq!(args.output, Some(OutputFormat::Yaml));
        assert_eq!(args.output_format(), OutputFormat::Yaml);
    }

    #[test]
    fn parses_sarif_output_format() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output", "sarif"]);

        let args = scan_args_from(cli);
        assert_eq!(args.output, Some(OutputFormat::Sarif));
        assert_eq!(args.output_format(), OutputFormat::Sarif);
    }

    #[test]
    fn parses_html_output_format() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output", "html"]);

        let args = scan_args_from(cli);
        assert_eq!(args.output, Some(OutputFormat::Html));
        assert_eq!(args.output_format(), OutputFormat::Html);
    }

    #[test]
    fn parses_output_file() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output-file", "report.json"]);

        let args = scan_args_from(cli);
        assert_eq!(args.output_file, Some(PathBuf::from("report.json")));
    }

    #[test]
    fn infers_json_output_format_from_output_file_extension() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output-file", "report.json"]);

        let args = scan_args_from(cli);
        assert_eq!(args.output_format(), OutputFormat::Json);
    }

    #[test]
    fn infers_json_output_format_from_uppercase_output_file_extension() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output-file", "REPORT.JSON"]);

        let args = scan_args_from(cli);
        assert_eq!(args.output_format(), OutputFormat::Json);
    }

    #[test]
    fn infers_yaml_output_format_from_output_file_extension() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output-file", "report.yaml"]);

        let args = scan_args_from(cli);
        assert_eq!(args.output_format(), OutputFormat::Yaml);
    }

    #[test]
    fn infers_yaml_output_format_from_yml_output_file_extension() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output-file", "REPORT.YML"]);

        let args = scan_args_from(cli);
        assert_eq!(args.output_format(), OutputFormat::Yaml);
    }

    #[test]
    fn infers_html_output_format_from_output_file_extension() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output-file", "report.html"]);

        let args = scan_args_from(cli);
        assert_eq!(args.output_format(), OutputFormat::Html);
    }

    #[test]
    fn infers_html_output_format_from_htm_output_file_extension() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output-file", "REPORT.HTM"]);

        let args = scan_args_from(cli);
        assert_eq!(args.output_format(), OutputFormat::Html);
    }

    #[test]
    fn infers_sarif_output_format_from_output_file_extension() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--output-file", "report.sarif"]);

        let args = scan_args_from(cli);
        assert_eq!(args.output_format(), OutputFormat::Sarif);
    }

    #[test]
    fn keeps_explicit_output_format_when_output_file_extension_is_json() {
        let cli = Cli::parse_from([
            "reforge",
            "scan",
            ".",
            "--output-file",
            "report.json",
            "--output",
            "human",
        ]);

        let args = scan_args_from(cli);
        assert_eq!(args.output_format(), OutputFormat::Human);
    }

    #[test]
    fn keeps_explicit_output_format_when_output_file_extension_is_yaml() {
        let cli = Cli::parse_from([
            "reforge",
            "scan",
            ".",
            "--output-file",
            "report.yaml",
            "--output",
            "json",
        ]);

        let args = scan_args_from(cli);
        assert_eq!(args.output_format(), OutputFormat::Json);
    }

    #[test]
    fn parses_progress_mode() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--progress", "never"]);

        let args = scan_args_from(cli);
        assert_eq!(args.progress, ProgressMode::Never);
    }

    #[test]
    fn resolves_progress_modes() {
        assert!(ProgressMode::Auto.enabled(true));
        assert!(!ProgressMode::Auto.enabled(false));
        assert!(ProgressMode::Always.enabled(false));
        assert!(!ProgressMode::Never.enabled(true));
    }

    #[test]
    fn parses_color_mode() {
        let cli = Cli::parse_from(["reforge", "scan", ".", "--color", "never"]);

        let args = scan_args_from(cli);
        assert_eq!(args.color, ColorMode::Never);
    }

    #[test]
    fn resolves_color_modes() {
        assert!(ColorMode::Auto.enabled(true));
        assert!(!ColorMode::Auto.enabled(false));
        assert!(ColorMode::Always.enabled(false));
        assert!(!ColorMode::Never.enabled(true));
    }

    #[test]
    fn parses_quality_model_options() {
        let cli = Cli::parse_from([
            "reforge",
            "scan",
            ".",
            "--churn",
            "on",
            "--hotspot-model",
            "static",
            "--churn-window-days",
            "90",
            "--churn-max-commit-lines",
            "1000",
        ]);

        let args = scan_args_from(cli);
        assert_eq!(args.churn, Some(ChurnMode::On));
        assert_eq!(args.hotspot_model, Some(HotspotModel::Static));
        assert_eq!(args.churn_window_days, Some(90));
        assert_eq!(args.churn_max_commit_lines, Some(1000));
    }

    #[test]
    fn parses_baseline_and_failure_gate_options() {
        let cli = Cli::parse_from([
            "reforge",
            "scan",
            ".",
            "--baseline",
            "baseline.json",
            "--baseline-mode",
            "new",
            "--show",
            "new-or-worse",
            "--fail-on",
            "warning",
        ]);

        let args = scan_args_from(cli);
        assert_eq!(args.ci.baseline, Some(PathBuf::from("baseline.json")));
        assert_eq!(args.ci.baseline_mode, BaselineMode::New);
        assert_eq!(args.ci.show, BaselineShow::NewOrWorse);
        assert_eq!(args.ci.fail_on, Some(FailOnSeverity::Warning));
    }
}
