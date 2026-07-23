use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThresholdPreset {
    Strict,
    #[default]
    Balanced,
    Relaxed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChurnMode {
    Auto,
    On,
    Off,
}

impl EffectiveConfig {
    pub fn defaults_for_path(path: PathBuf) -> Self {
        let thresholds = ThresholdSettings::BALANCED;
        Self {
            path,
            preset: None,
            max_file_lines: thresholds.file.max_file_lines,
            max_dir_files: thresholds.file.max_dir_files,
            filters: ScanFilterArgs::default(),
            codebase_thresholds: CodebaseThresholds {
                min_similar_functions: thresholds.similarity.min_similar_functions,
                min_function_tokens: thresholds.similarity.min_function_tokens,
                function_similarity: thresholds.similarity.function_similarity,
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
            },
            churn: None,
            churn_window_days: None,
            churn_max_commit_lines: None,
            reproducible: false,
            progress: ProgressMode::Auto,
        }
    }
}

impl Default for EffectiveConfig {
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

impl ThresholdPreset {
    pub fn thresholds(self) -> ThresholdSettings {
        match self {
            Self::Strict => ThresholdSettings::STRICT,
            Self::Balanced => ThresholdSettings::BALANCED,
            Self::Relaxed => ThresholdSettings::RELAXED,
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
