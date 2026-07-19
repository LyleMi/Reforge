use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cli::{
    ChurnMode, DEFAULT_CHURN_MAX_COMMIT_LINES, DEFAULT_CHURN_WINDOW_DAYS, ScanArgs,
    ThresholdPreset, UnityMode,
};

use super::thresholds::{
    ConfigFileThresholdDefaults, ConfigRepetitionThresholdDefaults,
    ConfigSimilarityThresholdDefaults, ConfigStructureThresholdDefaults, ConfigThresholdDefaults,
    ConfigUnityThresholdDefaults, apply_threshold_defaults,
};

pub(crate) const CONFIG_FILE_NAME: &str = "reforge.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
struct ReforgeConfig {
    preset: Option<ThresholdPreset>,
    max_file_lines: Option<usize>,
    max_dir_files: Option<usize>,
    min_similar_functions: Option<usize>,
    min_function_tokens: Option<usize>,
    function_similarity: Option<f64>,
    max_function_lines: Option<usize>,
    max_function_complexity: Option<usize>,
    max_nesting_depth: Option<usize>,
    max_function_parameters: Option<usize>,
    max_type_lines: Option<usize>,
    max_type_members: Option<usize>,
    max_imports: Option<usize>,
    max_public_items: Option<usize>,
    max_functions_per_file: Option<usize>,
    max_functions_per_100_lines: Option<usize>,
    max_small_function_ratio: Option<usize>,
    min_repeated_literal_occurrences: Option<usize>,
    min_data_clump_occurrences: Option<usize>,
    churn: Option<ChurnMode>,
    churn_window_days: Option<usize>,
    churn_max_commit_lines: Option<usize>,
    ignore_paths: Vec<String>,
    suppressions: Vec<ConfigSuppression>,
    unity: UnityConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
struct UnityConfig {
    mode: Option<UnityMode>,
    max_assembly_dependencies: Option<usize>,
    max_scene_objects: Option<usize>,
    max_prefab_objects: Option<usize>,
    max_serialized_fields: Option<usize>,
    max_lifecycle_methods: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ConfigSuppression {
    pub kind: Option<String>,
    pub path: String,
    pub line: Option<usize>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub(super) struct EffectiveScanConfig {
    pub args: ScanArgs,
    pub suppressions: Vec<ConfigSuppression>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct EffectiveConfigOutput {
    preset: ThresholdPreset,
    max_file_lines: usize,
    max_dir_files: usize,
    include_hidden: bool,
    include_generated: bool,
    no_gitignore: bool,
    exclude_tests: bool,
    ignore_paths: Vec<String>,
    #[serde(flatten)]
    analysis: EffectiveAnalysisConfigOutput,
    churn: ChurnMode,
    churn_window_days: usize,
    churn_max_commit_lines: usize,
    unity: UnityMode,
    max_unity_assembly_dependencies: usize,
    max_unity_scene_objects: usize,
    max_unity_prefab_objects: usize,
    max_unity_serialized_fields: usize,
    max_unity_lifecycle_methods: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct EffectiveAnalysisConfigOutput {
    min_similar_functions: usize,
    min_function_tokens: usize,
    function_similarity: f64,
    include_test_similarity: bool,
    max_function_lines: usize,
    max_function_complexity: usize,
    max_nesting_depth: usize,
    max_function_parameters: usize,
    max_type_lines: usize,
    max_type_members: usize,
    max_imports: usize,
    max_public_items: usize,
    max_functions_per_file: usize,
    max_functions_per_100_lines: usize,
    max_small_function_ratio: usize,
    min_repeated_literal_occurrences: usize,
    min_data_clump_occurrences: usize,
    include_test_structure: bool,
}

impl std::ops::Deref for EffectiveConfigOutput {
    type Target = EffectiveAnalysisConfigOutput;

    fn deref(&self) -> &Self::Target {
        &self.analysis
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct ReforgeConfigTemplate {
    preset: ThresholdPreset,
    max_file_lines: usize,
    max_dir_files: usize,
    min_similar_functions: usize,
    min_function_tokens: usize,
    function_similarity: f64,
    max_function_lines: usize,
    max_function_complexity: usize,
    max_nesting_depth: usize,
    max_function_parameters: usize,
    max_type_lines: usize,
    max_type_members: usize,
    max_imports: usize,
    max_public_items: usize,
    max_functions_per_file: usize,
    max_functions_per_100_lines: usize,
    max_small_function_ratio: usize,
    min_repeated_literal_occurrences: usize,
    min_data_clump_occurrences: usize,
    churn: ChurnMode,
    churn_window_days: usize,
    churn_max_commit_lines: usize,
    ignore_paths: Vec<String>,
    unity: UnityConfigTemplate,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct UnityConfigTemplate {
    mode: UnityMode,
    max_assembly_dependencies: usize,
    max_scene_objects: usize,
    max_prefab_objects: usize,
    max_serialized_fields: usize,
    max_lifecycle_methods: usize,
}

impl From<&ReforgeConfig> for ConfigThresholdDefaults {
    fn from(config: &ReforgeConfig) -> Self {
        Self {
            preset: config.preset,
            file: ConfigFileThresholdDefaults {
                max_file_lines: config.max_file_lines,
                max_dir_files: config.max_dir_files,
            },
            similarity: ConfigSimilarityThresholdDefaults {
                min_similar_functions: config.min_similar_functions,
                min_function_tokens: config.min_function_tokens,
                function_similarity: config.function_similarity,
            },
            structure: ConfigStructureThresholdDefaults {
                max_function_lines: config.max_function_lines,
                max_function_complexity: config.max_function_complexity,
                max_nesting_depth: config.max_nesting_depth,
                max_function_parameters: config.max_function_parameters,
                max_type_lines: config.max_type_lines,
                max_type_members: config.max_type_members,
                max_imports: config.max_imports,
                max_public_items: config.max_public_items,
                max_functions_per_file: config.max_functions_per_file,
                max_functions_per_100_lines: config.max_functions_per_100_lines,
                max_small_function_ratio: config.max_small_function_ratio,
            },
            repetition: ConfigRepetitionThresholdDefaults {
                min_repeated_literal_occurrences: config.min_repeated_literal_occurrences,
                min_data_clump_occurrences: config.min_data_clump_occurrences,
            },
            unity: ConfigUnityThresholdDefaults {
                max_assembly_dependencies: config.unity.max_assembly_dependencies,
                max_scene_objects: config.unity.max_scene_objects,
                max_prefab_objects: config.unity.max_prefab_objects,
                max_serialized_fields: config.unity.max_serialized_fields,
                max_lifecycle_methods: config.unity.max_lifecycle_methods,
            },
        }
    }
}

pub(crate) fn effective_scan_config(args: &ScanArgs, root: &Path) -> Result<EffectiveScanConfig> {
    let mut effective = args.clone();
    let config_path = resolve_config_path(args.config.as_deref(), root);
    let config = match &config_path {
        Some(path) => Some(parse_config_file(path)?),
        None => None,
    };
    let suppressions = config
        .as_ref()
        .map(|config| config.suppressions.clone())
        .unwrap_or_default();

    apply_config_defaults(&mut effective, args, config.as_ref());

    effective.churn = Some(
        args.churn
            .unwrap_or(effective.churn.unwrap_or(ChurnMode::Auto)),
    );
    effective.churn_window_days = Some(
        args.churn_window_days.unwrap_or(
            effective
                .churn_window_days
                .unwrap_or(DEFAULT_CHURN_WINDOW_DAYS),
        ),
    );
    effective.churn_max_commit_lines = Some(
        args.churn_max_commit_lines.unwrap_or(
            effective
                .churn_max_commit_lines
                .unwrap_or(DEFAULT_CHURN_MAX_COMMIT_LINES),
        ),
    );

    Ok(EffectiveScanConfig {
        args: effective,
        suppressions,
    })
}

pub(crate) fn validate_config(config_path: Option<&Path>, root: &Path) -> Result<Option<PathBuf>> {
    let Some(path) = resolve_config_path(config_path, root) else {
        return Ok(None);
    };

    parse_config_file(&path)?;
    Ok(Some(path))
}

pub(crate) fn effective_config_output(
    args: &ScanArgs,
    root: &Path,
) -> Result<EffectiveConfigOutput> {
    let effective = effective_scan_config(args, root)?;
    Ok(EffectiveConfigOutput::from(&effective.args))
}

pub(crate) fn default_config_toml() -> Result<String> {
    let defaults = ScanArgs::default();
    let template = ReforgeConfigTemplate::from(&defaults);
    let mut output =
        toml::to_string_pretty(&template).context("failed to serialize default configuration")?;
    if !output.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

fn resolve_config_path(config_path: Option<&Path>, root: &Path) -> Option<PathBuf> {
    config_path
        .map(Path::to_path_buf)
        .or_else(|| discover_config_path(root))
}

fn parse_config_file(path: &Path) -> Result<ReforgeConfig> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;
    let config = toml::from_str(&contents)
        .with_context(|| format!("failed to parse config file {}", path.display()))?;
    Ok(config)
}

fn discover_config_path(root: &Path) -> Option<PathBuf> {
    let mut current = if root.is_file() {
        root.parent()?.to_path_buf()
    } else {
        root.to_path_buf()
    };

    loop {
        let candidate = current.join(CONFIG_FILE_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn apply_config_defaults(args: &mut ScanArgs, cli_args: &ScanArgs, config: Option<&ReforgeConfig>) {
    apply_threshold_defaults(args, cli_args, config.map(ConfigThresholdDefaults::from));
    if let Some(config) = config {
        apply_churn_config_defaults(args, config);
        apply_ignore_path_defaults(args, config);
        if !args.threshold_overrides.unity {
            args.unity = config.unity.mode.unwrap_or(UnityMode::Auto);
        }
    }
}

fn apply_churn_config_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    args.churn = args.churn.or(config.churn);
    args.churn_window_days = args.churn_window_days.or(config.churn_window_days);
    args.churn_max_commit_lines = args
        .churn_max_commit_lines
        .or(config.churn_max_commit_lines);
}

fn apply_ignore_path_defaults(args: &mut ScanArgs, config: &ReforgeConfig) {
    if config.ignore_paths.is_empty() {
        return;
    }

    let mut ignore_paths = config.ignore_paths.clone();
    for path in &args.filters.ignore_paths {
        if !ignore_paths.contains(path) {
            ignore_paths.push(path.clone());
        }
    }
    args.filters.ignore_paths = ignore_paths;
}

impl From<&ScanArgs> for EffectiveConfigOutput {
    fn from(args: &ScanArgs) -> Self {
        Self {
            preset: args.preset.unwrap_or_default(),
            max_file_lines: args.max_file_lines,
            max_dir_files: args.max_dir_files,
            include_hidden: args.filters.include_hidden,
            include_generated: args.filters.include_generated,
            no_gitignore: args.filters.no_gitignore,
            exclude_tests: args.filters.exclude_tests,
            ignore_paths: args.filters.ignore_paths.clone(),
            analysis: EffectiveAnalysisConfigOutput {
                min_similar_functions: args.min_similar_functions,
                min_function_tokens: args.min_function_tokens,
                function_similarity: args.function_similarity,
                include_test_similarity: args.include_test_similarity,
                max_function_lines: args.max_function_lines,
                max_function_complexity: args.max_function_complexity,
                max_nesting_depth: args.max_nesting_depth,
                max_function_parameters: args.max_function_parameters,
                max_type_lines: args.max_type_lines,
                max_type_members: args.max_type_members,
                max_imports: args.max_imports,
                max_public_items: args.max_public_items,
                max_functions_per_file: args.function_proliferation.max_functions_per_file,
                max_functions_per_100_lines: args
                    .function_proliferation
                    .max_functions_per_100_lines,
                max_small_function_ratio: args.function_proliferation.max_small_function_ratio,
                min_repeated_literal_occurrences: args.min_repeated_literal_occurrences,
                min_data_clump_occurrences: args.min_data_clump_occurrences,
                include_test_structure: args.include_test_structure,
            },
            churn: args.churn.expect("effective args should set churn mode"),
            churn_window_days: args
                .churn_window_days
                .expect("effective args should set churn window"),
            churn_max_commit_lines: args
                .churn_max_commit_lines
                .expect("effective args should set churn max commit lines"),
            unity: args.unity,
            max_unity_assembly_dependencies: args.max_unity_assembly_dependencies,
            max_unity_scene_objects: args.max_unity_scene_objects,
            max_unity_prefab_objects: args.max_unity_prefab_objects,
            max_unity_serialized_fields: args.max_unity_serialized_fields,
            max_unity_lifecycle_methods: args.max_unity_lifecycle_methods,
        }
    }
}

impl From<&ScanArgs> for ReforgeConfigTemplate {
    fn from(args: &ScanArgs) -> Self {
        Self {
            preset: args.preset.unwrap_or_default(),
            max_file_lines: args.max_file_lines,
            max_dir_files: args.max_dir_files,
            min_similar_functions: args.min_similar_functions,
            min_function_tokens: args.min_function_tokens,
            function_similarity: args.function_similarity,
            max_function_lines: args.max_function_lines,
            max_function_complexity: args.max_function_complexity,
            max_nesting_depth: args.max_nesting_depth,
            max_function_parameters: args.max_function_parameters,
            max_type_lines: args.max_type_lines,
            max_type_members: args.max_type_members,
            max_imports: args.max_imports,
            max_public_items: args.max_public_items,
            max_functions_per_file: args.function_proliferation.max_functions_per_file,
            max_functions_per_100_lines: args.function_proliferation.max_functions_per_100_lines,
            max_small_function_ratio: args.function_proliferation.max_small_function_ratio,
            min_repeated_literal_occurrences: args.min_repeated_literal_occurrences,
            min_data_clump_occurrences: args.min_data_clump_occurrences,
            churn: ChurnMode::Auto,
            churn_window_days: DEFAULT_CHURN_WINDOW_DAYS,
            churn_max_commit_lines: DEFAULT_CHURN_MAX_COMMIT_LINES,
            ignore_paths: args.filters.ignore_paths.clone(),
            unity: UnityConfigTemplate {
                mode: args.unity,
                max_assembly_dependencies: args.max_unity_assembly_dependencies,
                max_scene_objects: args.max_unity_scene_objects,
                max_prefab_objects: args.max_unity_prefab_objects,
                max_serialized_fields: args.max_unity_serialized_fields,
                max_lifecycle_methods: args.max_unity_lifecycle_methods,
            },
        }
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command, ThresholdSettings};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("reforge-config-{name}-{suffix}"))
    }

    #[test]
    fn default_config_toml_is_valid() -> Result<()> {
        let config = default_config_toml()?;
        let parsed: ReforgeConfig = toml::from_str(&config)?;

        assert_eq!(parsed.preset, Some(ThresholdPreset::Balanced));
        assert_eq!(
            parsed.max_file_lines,
            Some(ThresholdSettings::BALANCED.file.max_file_lines)
        );
        assert_eq!(
            parsed.min_repeated_literal_occurrences,
            Some(
                ThresholdSettings::BALANCED
                    .repetition
                    .min_repeated_literal_occurrences
            )
        );
        assert_eq!(
            parsed.min_data_clump_occurrences,
            Some(
                ThresholdSettings::BALANCED
                    .repetition
                    .min_data_clump_occurrences
            )
        );
        assert_eq!(parsed.churn, Some(ChurnMode::Auto));
        assert_eq!(parsed.hotspot_model, Some(HotspotModel::Hybrid));
        Ok(())
    }

    #[test]
    fn generated_config_preset_can_change_without_deleting_balanced_thresholds() -> Result<()> {
        let root = test_root("generated-config-preset");
        fs::create_dir_all(&root)?;
        let config =
            default_config_toml()?.replacen("preset = \"balanced\"", "preset = \"strict\"", 1);
        fs::write(root.join(CONFIG_FILE_NAME), config)?;
        let args = ScanArgs::defaults_for_path(root.clone());

        let output = effective_config_output(&args, &root)?;

        fs::remove_dir_all(root)?;

        assert_eq!(output.preset, ThresholdPreset::Strict);
        assert_eq!(
            output.max_file_lines,
            ThresholdSettings::STRICT.file.max_file_lines
        );
        assert_eq!(
            output.max_function_lines,
            ThresholdSettings::STRICT.structure.max_function_lines
        );
        Ok(())
    }

    #[test]
    fn validate_config_rejects_unknown_keys() -> Result<()> {
        let root = test_root("unknown-key");
        fs::create_dir_all(&root)?;
        let config_path = root.join(CONFIG_FILE_NAME);
        fs::write(&config_path, "unknown-key = true\n")?;

        let result = validate_config(None, &root);

        fs::remove_dir_all(root)?;

        assert!(result.is_err());
        assert!(format!("{:#}", result.unwrap_err()).contains("unknown field"));
        Ok(())
    }

    #[test]
    fn effective_config_output_applies_discovered_config() -> Result<()> {
        let root = test_root("effective");
        fs::create_dir_all(&root)?;
        fs::write(
            root.join(CONFIG_FILE_NAME),
            "max-file-lines = 600\nchurn = \"off\"\nhotspot-model = \"static\"\nignore-paths = [\"vendor\"]\n",
        )?;
        let args = ScanArgs::defaults_for_path(root.clone());

        let output = effective_config_output(&args, &root)?;

        fs::remove_dir_all(root)?;

        assert_eq!(output.max_file_lines, 600);
        assert_eq!(output.churn, ChurnMode::Off);
        assert_eq!(output.hotspot_model, HotspotModel::Static);
        assert_eq!(output.ignore_paths, ["vendor"]);
        Ok(())
    }

    #[test]
    fn effective_config_output_applies_config_preset() -> Result<()> {
        let root = test_root("config-preset");
        fs::create_dir_all(&root)?;
        fs::write(root.join(CONFIG_FILE_NAME), "preset = \"strict\"\n")?;
        let args = ScanArgs::defaults_for_path(root.clone());

        let output = effective_config_output(&args, &root)?;

        fs::remove_dir_all(root)?;

        assert_eq!(output.preset, ThresholdPreset::Strict);
        assert_eq!(
            output.max_file_lines,
            ThresholdSettings::STRICT.file.max_file_lines
        );
        assert_eq!(
            output.max_function_lines,
            ThresholdSettings::STRICT.structure.max_function_lines
        );
        assert_eq!(
            output.min_similar_functions,
            ThresholdSettings::STRICT.similarity.min_similar_functions
        );
        assert_eq!(
            output.function_similarity,
            ThresholdSettings::STRICT.similarity.function_similarity
        );
        Ok(())
    }

    #[test]
    fn unity_config_and_cli_thresholds_follow_precedence() -> Result<()> {
        let root = test_root("unity-precedence");
        fs::create_dir_all(&root)?;
        fs::write(
            root.join(CONFIG_FILE_NAME),
            "preset = \"strict\"\n[unity]\nmode = \"on\"\nmax-scene-objects = 700\n",
        )?;
        let cli = Cli::parse_from_with_explicit_overrides([
            "reforge",
            "scan",
            root.to_str().unwrap(),
            "--max-unity-scene-objects",
            "900",
        ]);
        let Command::Scan(args) = cli.command else {
            unreachable!()
        };

        let output = effective_config_output(&args, &root)?;

        fs::remove_dir_all(root)?;
        assert_eq!(output.unity, UnityMode::On);
        assert_eq!(output.max_unity_scene_objects, 900);
        assert_eq!(output.max_unity_prefab_objects, 100);
        Ok(())
    }

    #[test]
    fn config_threshold_overrides_config_preset() -> Result<()> {
        let root = test_root("config-threshold-over-preset");
        fs::create_dir_all(&root)?;
        fs::write(
            root.join(CONFIG_FILE_NAME),
            "preset = \"strict\"\nmax-file-lines = 700\n",
        )?;
        let args = ScanArgs::defaults_for_path(root.clone());

        let output = effective_config_output(&args, &root)?;

        fs::remove_dir_all(root)?;

        assert_eq!(output.preset, ThresholdPreset::Strict);
        assert_eq!(output.max_file_lines, 700);
        assert_eq!(
            output.max_function_lines,
            ThresholdSettings::STRICT.structure.max_function_lines
        );
        Ok(())
    }

    #[test]
    fn cli_preset_overrides_config_thresholds() -> Result<()> {
        let root = test_root("cli-preset-over-config");
        fs::create_dir_all(&root)?;
        fs::write(
            root.join(CONFIG_FILE_NAME),
            "preset = \"strict\"\nmax-file-lines = 700\n",
        )?;
        let mut args = ScanArgs::defaults_for_path(root.clone());
        args.preset = Some(ThresholdPreset::Relaxed);

        let output = effective_config_output(&args, &root)?;

        fs::remove_dir_all(root)?;

        assert_eq!(output.preset, ThresholdPreset::Relaxed);
        assert_eq!(
            output.max_file_lines,
            ThresholdSettings::RELAXED.file.max_file_lines
        );
        Ok(())
    }

    #[test]
    fn cli_threshold_overrides_cli_preset() -> Result<()> {
        let root = test_root("cli-threshold-over-preset");
        fs::create_dir_all(&root)?;
        let mut args = ScanArgs::defaults_for_path(root.clone());
        args.preset = Some(ThresholdPreset::Strict);
        args.max_file_lines = 700;

        let output = effective_config_output(&args, &root)?;

        fs::remove_dir_all(root)?;

        assert_eq!(output.preset, ThresholdPreset::Strict);
        assert_eq!(output.max_file_lines, 700);
        assert_eq!(
            output.max_function_lines,
            ThresholdSettings::STRICT.structure.max_function_lines
        );
        Ok(())
    }

    #[test]
    fn explicit_cli_balanced_threshold_overrides_cli_preset() -> Result<()> {
        let root = test_root("cli-balanced-threshold-over-preset");
        fs::create_dir_all(&root)?;
        let root_arg = root.display().to_string();
        let cli = Cli::parse_from_with_explicit_overrides([
            "reforge",
            "scan",
            &root_arg,
            "--preset",
            "strict",
            "--max-file-lines",
            "800",
        ]);
        let Command::Scan(args) = cli.command else {
            panic!("expected scan command");
        };

        let output = effective_config_output(&args, &root)?;

        fs::remove_dir_all(root)?;

        assert_eq!(output.preset, ThresholdPreset::Strict);
        assert_eq!(
            output.max_file_lines,
            ThresholdSettings::BALANCED.file.max_file_lines
        );
        assert_eq!(
            output.max_function_lines,
            ThresholdSettings::STRICT.structure.max_function_lines
        );
        Ok(())
    }

    #[test]
    fn scoring_policy_cli_path_takes_precedence_and_resolves_from_cwd() -> Result<()> {
        let args = ScanArgs {
            scoring_policy: Some(PathBuf::from("cli-policy.json")),
            ..Default::default()
        };
        let config = ReforgeConfig {
            scoring_policy: Some(PathBuf::from("config-policy.json")),
            ..Default::default()
        };
        let resolved = resolve_scoring_policy_path(
            &args,
            Some(&config),
            Some(Path::new("project/reforge.toml")),
        )?
        .unwrap();
        assert_eq!(resolved, std::env::current_dir()?.join("cli-policy.json"));
        Ok(())
    }

    #[test]
    fn scoring_policy_config_path_resolves_from_config_directory() -> Result<()> {
        let args = ScanArgs::default();
        let config = ReforgeConfig {
            scoring_policy: Some(PathBuf::from("policies/accepted.json")),
            ..Default::default()
        };
        let resolved = resolve_scoring_policy_path(
            &args,
            Some(&config),
            Some(Path::new("project/reforge.toml")),
        )?
        .unwrap();
        assert_eq!(resolved, PathBuf::from("project/policies/accepted.json"));
        Ok(())
    }
}
