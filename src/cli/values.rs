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
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BaselineShow {
    New,
    All,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnityMode {
    #[default]
    Auto,
    On,
    Off,
}

impl ScanArgs {
    pub fn defaults_for_path(path: PathBuf) -> Self {
        let thresholds = ThresholdSettings::BALANCED;
        Self {
            path,
            threshold_overrides: ThresholdOverrideFlags::default(),
            preset: None,
            unity: UnityMode::Auto,
            max_unity_assembly_dependencies: thresholds.unity.max_assembly_dependencies,
            max_unity_scene_objects: thresholds.unity.max_scene_objects,
            max_unity_prefab_objects: thresholds.unity.max_prefab_objects,
            max_unity_serialized_fields: thresholds.unity.max_serialized_fields,
            max_unity_lifecycle_methods: thresholds.unity.max_lifecycle_methods,
            max_file_lines: thresholds.file.max_file_lines,
            max_dir_files: thresholds.file.max_dir_files,
            filters: ScanFilterArgs::default(),
            finding_controls: FindingControlArgs::default(),
            analysis_thresholds: AnalysisThresholdArgs {
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
            },
            config: None,
            ci: CiArgs::default(),
            churn: None,
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

    pub(crate) fn try_parse_from_with_explicit_overrides<I, T>(itr: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let matches = Self::command().try_get_matches_from(itr)?;
        let mut cli = Self::from_arg_matches(&matches)?;
        Self::apply_explicit_overrides(&mut cli, &matches);
        Ok(cli)
    }

    fn from_arg_matches_with_explicit_overrides(matches: &ArgMatches) -> Self {
        let mut cli = Self::from_arg_matches(matches).unwrap_or_else(|error| error.exit());
        Self::apply_explicit_overrides(&mut cli, matches);
        cli
    }

    fn apply_explicit_overrides(cli: &mut Self, matches: &ArgMatches) {
        match (&mut cli.command, matches.subcommand()) {
            (Command::Scan(args), Some(("scan", scan_matches))) => {
                args.threshold_overrides = ThresholdOverrideFlags::from_arg_matches(scan_matches);
            }
            (Command::Workflow(workflow), Some(("workflow", workflow_matches))) => {
                if let WorkflowCommand::Start(args) = &mut workflow.command
                    && let Some(("start", start_matches)) = workflow_matches.subcommand()
                {
                    args.scan.threshold_overrides =
                        ThresholdOverrideFlags::from_arg_matches(start_matches);
                }
            }
            _ => {}
        }
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
            baseline_mode: BaselineMode::New,
            show: BaselineShow::All,
            fail_on_findings: false,
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
            unity: was_command_line_value(matches, "unity"),
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
            max_unity_assembly_dependencies: was_command_line_value(
                matches,
                "max_unity_assembly_dependencies",
            ),
            max_unity_scene_objects: was_command_line_value(matches, "max_unity_scene_objects"),
            max_unity_prefab_objects: was_command_line_value(matches, "max_unity_prefab_objects"),
            max_unity_serialized_fields: was_command_line_value(
                matches,
                "max_unity_serialized_fields",
            ),
            max_unity_lifecycle_methods: was_command_line_value(
                matches,
                "max_unity_lifecycle_methods",
            ),
        }
    }
}

fn was_command_line_value(matches: &ArgMatches, id: &str) -> bool {
    matches.value_source(id) == Some(ValueSource::CommandLine)
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
