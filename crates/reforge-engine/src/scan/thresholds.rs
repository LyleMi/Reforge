use crate::execution::{EffectiveConfig, ThresholdPreset, ThresholdSettings};

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ConfigThresholdDefaults {
    pub preset: Option<ThresholdPreset>,
    pub file: ConfigFileThresholdDefaults,
    pub similarity: ConfigSimilarityThresholdDefaults,
    pub structure: ConfigStructureThresholdDefaults,
    pub repetition: ConfigRepetitionThresholdDefaults,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ConfigFileThresholdDefaults {
    pub max_file_lines: Option<usize>,
    pub max_dir_files: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ConfigSimilarityThresholdDefaults {
    pub min_similar_functions: Option<usize>,
    pub min_function_tokens: Option<usize>,
    pub function_similarity: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ConfigStructureThresholdDefaults {
    pub max_function_lines: Option<usize>,
    pub max_function_complexity: Option<usize>,
    pub max_nesting_depth: Option<usize>,
    pub max_function_parameters: Option<usize>,
    pub max_type_lines: Option<usize>,
    pub max_type_members: Option<usize>,
    pub max_imports: Option<usize>,
    pub max_public_items: Option<usize>,
    pub max_functions_per_file: Option<usize>,
    pub max_functions_per_100_lines: Option<usize>,
    pub max_small_function_ratio: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ConfigRepetitionThresholdDefaults {
    pub min_repeated_literal_occurrences: Option<usize>,
    pub min_data_clump_occurrences: Option<usize>,
}

pub(super) fn apply_threshold_defaults(
    config: &mut EffectiveConfig,
    configured: Option<ConfigThresholdDefaults>,
) {
    let configured = configured.unwrap_or_default();
    let preset = configured.preset.unwrap_or_default();
    apply_threshold_settings(config, preset.thresholds());
    apply_configured_thresholds(config, configured);
    config.preset = Some(preset);
}

fn apply_threshold_settings(config: &mut EffectiveConfig, settings: ThresholdSettings) {
    config.max_file_lines = settings.file.max_file_lines;
    config.max_dir_files = settings.file.max_dir_files;
    config.min_similar_functions = settings.similarity.min_similar_functions;
    config.min_function_tokens = settings.similarity.min_function_tokens;
    config.function_similarity = settings.similarity.function_similarity;
    config.max_function_lines = settings.structure.max_function_lines;
    config.max_function_complexity = settings.structure.max_function_complexity;
    config.max_nesting_depth = settings.structure.max_nesting_depth;
    config.max_function_parameters = settings.structure.max_function_parameters;
    config.max_type_lines = settings.structure.max_type_lines;
    config.max_type_members = settings.structure.max_type_members;
    config.max_imports = settings.structure.max_imports;
    config.max_public_items = settings.structure.max_public_items;
    config.function_proliferation.max_functions_per_file =
        settings.structure.max_functions_per_file;
    config.function_proliferation.max_functions_per_100_lines =
        settings.structure.max_functions_per_100_lines;
    config.function_proliferation.max_small_function_ratio =
        settings.structure.max_small_function_ratio;
    config.min_repeated_literal_occurrences = settings.repetition.min_repeated_literal_occurrences;
    config.min_data_clump_occurrences = settings.repetition.min_data_clump_occurrences;
}

fn apply_configured_thresholds(config: &mut EffectiveConfig, configured: ConfigThresholdDefaults) {
    apply_optional(&mut config.max_file_lines, configured.file.max_file_lines);
    apply_optional(&mut config.max_dir_files, configured.file.max_dir_files);
    apply_optional(
        &mut config.min_similar_functions,
        configured.similarity.min_similar_functions,
    );
    apply_optional(
        &mut config.min_function_tokens,
        configured.similarity.min_function_tokens,
    );
    apply_optional(
        &mut config.function_similarity,
        configured.similarity.function_similarity,
    );
    apply_structure_thresholds(config, configured.structure);
    apply_optional(
        &mut config.min_repeated_literal_occurrences,
        configured.repetition.min_repeated_literal_occurrences,
    );
    apply_optional(
        &mut config.min_data_clump_occurrences,
        configured.repetition.min_data_clump_occurrences,
    );
}

fn apply_structure_thresholds(
    config: &mut EffectiveConfig,
    configured: ConfigStructureThresholdDefaults,
) {
    apply_optional(
        &mut config.max_function_lines,
        configured.max_function_lines,
    );
    apply_optional(
        &mut config.max_function_complexity,
        configured.max_function_complexity,
    );
    apply_optional(&mut config.max_nesting_depth, configured.max_nesting_depth);
    apply_optional(
        &mut config.max_function_parameters,
        configured.max_function_parameters,
    );
    apply_optional(&mut config.max_type_lines, configured.max_type_lines);
    apply_optional(&mut config.max_type_members, configured.max_type_members);
    apply_optional(&mut config.max_imports, configured.max_imports);
    apply_optional(&mut config.max_public_items, configured.max_public_items);
    apply_optional(
        &mut config.function_proliferation.max_functions_per_file,
        configured.max_functions_per_file,
    );
    apply_optional(
        &mut config.function_proliferation.max_functions_per_100_lines,
        configured.max_functions_per_100_lines,
    );
    apply_optional(
        &mut config.function_proliferation.max_small_function_ratio,
        configured.max_small_function_ratio,
    );
}

fn apply_optional<T>(target: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *target = value;
    }
}
