use crate::cli::{ScanArgs, ThresholdOverrideFlags, ThresholdPreset, ThresholdSettings};

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ConfigThresholdDefaults {
    pub preset: Option<ThresholdPreset>,
    pub file: ConfigFileThresholdDefaults,
    pub similarity: ConfigSimilarityThresholdDefaults,
    pub structure: ConfigStructureThresholdDefaults,
    pub repetition: ConfigRepetitionThresholdDefaults,
    pub unity: ConfigUnityThresholdDefaults,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ConfigUnityThresholdDefaults {
    pub max_assembly_dependencies: Option<usize>,
    pub max_scene_objects: Option<usize>,
    pub max_prefab_objects: Option<usize>,
    pub max_serialized_fields: Option<usize>,
    pub max_lifecycle_methods: Option<usize>,
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
    args: &mut ScanArgs,
    cli_args: &ScanArgs,
    config: Option<ConfigThresholdDefaults>,
) {
    let cli_overrides = ThresholdOverrides::from_cli(cli_args);
    let config_overrides = config
        .map(ThresholdOverrides::from_config)
        .unwrap_or_default();
    let preset = cli_args
        .preset
        .or_else(|| config.and_then(|config| config.preset))
        .unwrap_or_default();

    apply_threshold_settings(args, preset.thresholds());
    if cli_args.preset.is_none() {
        config_overrides.apply_unless(args, &cli_overrides);
    }
    cli_overrides.apply(args);
    args.preset = Some(preset);
}

fn apply_threshold_settings(args: &mut ScanArgs, settings: ThresholdSettings) {
    args.max_file_lines = settings.file.max_file_lines;
    args.max_dir_files = settings.file.max_dir_files;
    args.min_similar_functions = settings.similarity.min_similar_functions;
    args.min_function_tokens = settings.similarity.min_function_tokens;
    args.function_similarity = settings.similarity.function_similarity;
    args.max_function_lines = settings.structure.max_function_lines;
    args.max_function_complexity = settings.structure.max_function_complexity;
    args.max_nesting_depth = settings.structure.max_nesting_depth;
    args.max_function_parameters = settings.structure.max_function_parameters;
    args.max_type_lines = settings.structure.max_type_lines;
    args.max_type_members = settings.structure.max_type_members;
    args.max_imports = settings.structure.max_imports;
    args.max_public_items = settings.structure.max_public_items;
    args.function_proliferation.max_functions_per_file = settings.structure.max_functions_per_file;
    args.function_proliferation.max_functions_per_100_lines =
        settings.structure.max_functions_per_100_lines;
    args.function_proliferation.max_small_function_ratio =
        settings.structure.max_small_function_ratio;
    args.min_repeated_literal_occurrences = settings.repetition.min_repeated_literal_occurrences;
    args.min_data_clump_occurrences = settings.repetition.min_data_clump_occurrences;
    args.max_unity_assembly_dependencies = settings.unity.max_assembly_dependencies;
    args.max_unity_scene_objects = settings.unity.max_scene_objects;
    args.max_unity_prefab_objects = settings.unity.max_prefab_objects;
    args.max_unity_serialized_fields = settings.unity.max_serialized_fields;
    args.max_unity_lifecycle_methods = settings.unity.max_lifecycle_methods;
}

#[derive(Debug, Clone, Copy, Default)]
struct ThresholdOverrides {
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
    max_unity_assembly_dependencies: Option<usize>,
    max_unity_scene_objects: Option<usize>,
    max_unity_prefab_objects: Option<usize>,
    max_unity_serialized_fields: Option<usize>,
    max_unity_lifecycle_methods: Option<usize>,
}

#[derive(Clone, Copy)]
struct CliThresholdContext<'a> {
    args: &'a ScanArgs,
    defaults: ThresholdSettings,
    explicit: ThresholdOverrideFlags,
}

impl ThresholdOverrides {
    fn from_cli(args: &ScanArgs) -> Self {
        let context = CliThresholdContext {
            args,
            defaults: ThresholdSettings::BALANCED,
            explicit: args.threshold_overrides,
        };
        let mut overrides = Self::default();
        overrides.set_cli_file_thresholds(context);
        overrides.set_cli_similarity_thresholds(context);
        overrides.set_cli_structure_thresholds(context);
        overrides.set_cli_repetition_thresholds(context);
        overrides.set_cli_unity_thresholds(context);
        overrides
    }

    fn set_cli_unity_thresholds(&mut self, context: CliThresholdContext<'_>) {
        self.max_unity_assembly_dependencies = cli_usize(
            context.args.max_unity_assembly_dependencies,
            context.defaults.unity.max_assembly_dependencies,
            context.explicit.max_unity_assembly_dependencies,
        );
        self.max_unity_scene_objects = cli_usize(
            context.args.max_unity_scene_objects,
            context.defaults.unity.max_scene_objects,
            context.explicit.max_unity_scene_objects,
        );
        self.max_unity_prefab_objects = cli_usize(
            context.args.max_unity_prefab_objects,
            context.defaults.unity.max_prefab_objects,
            context.explicit.max_unity_prefab_objects,
        );
        self.max_unity_serialized_fields = cli_usize(
            context.args.max_unity_serialized_fields,
            context.defaults.unity.max_serialized_fields,
            context.explicit.max_unity_serialized_fields,
        );
        self.max_unity_lifecycle_methods = cli_usize(
            context.args.max_unity_lifecycle_methods,
            context.defaults.unity.max_lifecycle_methods,
            context.explicit.max_unity_lifecycle_methods,
        );
    }

    fn set_cli_file_thresholds(&mut self, context: CliThresholdContext<'_>) {
        self.max_file_lines = cli_usize(
            context.args.max_file_lines,
            context.defaults.file.max_file_lines,
            context.explicit.max_file_lines,
        );
        self.max_dir_files = cli_usize(
            context.args.max_dir_files,
            context.defaults.file.max_dir_files,
            context.explicit.max_dir_files,
        );
    }

    fn set_cli_similarity_thresholds(&mut self, context: CliThresholdContext<'_>) {
        self.min_similar_functions = cli_usize(
            context.args.min_similar_functions,
            context.defaults.similarity.min_similar_functions,
            context.explicit.min_similar_functions,
        );
        self.min_function_tokens = cli_usize(
            context.args.min_function_tokens,
            context.defaults.similarity.min_function_tokens,
            context.explicit.min_function_tokens,
        );
        self.function_similarity = cli_f64(
            context.args.function_similarity,
            context.defaults.similarity.function_similarity,
            context.explicit.function_similarity,
        );
    }

    fn set_cli_structure_thresholds(&mut self, context: CliThresholdContext<'_>) {
        self.max_function_lines = cli_usize(
            context.args.max_function_lines,
            context.defaults.structure.max_function_lines,
            context.explicit.max_function_lines,
        );
        self.max_function_complexity = cli_usize(
            context.args.max_function_complexity,
            context.defaults.structure.max_function_complexity,
            context.explicit.max_function_complexity,
        );
        self.max_nesting_depth = cli_usize(
            context.args.max_nesting_depth,
            context.defaults.structure.max_nesting_depth,
            context.explicit.max_nesting_depth,
        );
        self.max_function_parameters = cli_usize(
            context.args.max_function_parameters,
            context.defaults.structure.max_function_parameters,
            context.explicit.max_function_parameters,
        );
        self.max_type_lines = cli_usize(
            context.args.max_type_lines,
            context.defaults.structure.max_type_lines,
            context.explicit.max_type_lines,
        );
        self.max_type_members = cli_usize(
            context.args.max_type_members,
            context.defaults.structure.max_type_members,
            context.explicit.max_type_members,
        );
        self.max_imports = cli_usize(
            context.args.max_imports,
            context.defaults.structure.max_imports,
            context.explicit.max_imports,
        );
        self.max_public_items = cli_usize(
            context.args.max_public_items,
            context.defaults.structure.max_public_items,
            context.explicit.max_public_items,
        );
        self.max_functions_per_file = cli_usize(
            context.args.function_proliferation.max_functions_per_file,
            context.defaults.structure.max_functions_per_file,
            context.explicit.max_functions_per_file,
        );
        self.max_functions_per_100_lines = cli_usize(
            context
                .args
                .function_proliferation
                .max_functions_per_100_lines,
            context.defaults.structure.max_functions_per_100_lines,
            context.explicit.max_functions_per_100_lines,
        );
        self.max_small_function_ratio = cli_usize(
            context.args.function_proliferation.max_small_function_ratio,
            context.defaults.structure.max_small_function_ratio,
            context.explicit.max_small_function_ratio,
        );
    }

    fn set_cli_repetition_thresholds(&mut self, context: CliThresholdContext<'_>) {
        self.min_repeated_literal_occurrences = cli_usize(
            context.args.min_repeated_literal_occurrences,
            context.defaults.repetition.min_repeated_literal_occurrences,
            context.explicit.min_repeated_literal_occurrences,
        );
        self.min_data_clump_occurrences = cli_usize(
            context.args.min_data_clump_occurrences,
            context.defaults.repetition.min_data_clump_occurrences,
            context.explicit.min_data_clump_occurrences,
        );
    }

    fn from_config(config: ConfigThresholdDefaults) -> Self {
        let defaults = ThresholdSettings::BALANCED;
        Self {
            max_file_lines: configured_usize(
                config.file.max_file_lines,
                defaults.file.max_file_lines,
            ),
            max_dir_files: configured_usize(config.file.max_dir_files, defaults.file.max_dir_files),
            min_similar_functions: configured_usize(
                config.similarity.min_similar_functions,
                defaults.similarity.min_similar_functions,
            ),
            min_function_tokens: configured_usize(
                config.similarity.min_function_tokens,
                defaults.similarity.min_function_tokens,
            ),
            function_similarity: configured_f64(
                config.similarity.function_similarity,
                defaults.similarity.function_similarity,
            ),
            max_function_lines: configured_usize(
                config.structure.max_function_lines,
                defaults.structure.max_function_lines,
            ),
            max_function_complexity: configured_usize(
                config.structure.max_function_complexity,
                defaults.structure.max_function_complexity,
            ),
            max_nesting_depth: configured_usize(
                config.structure.max_nesting_depth,
                defaults.structure.max_nesting_depth,
            ),
            max_function_parameters: configured_usize(
                config.structure.max_function_parameters,
                defaults.structure.max_function_parameters,
            ),
            max_type_lines: configured_usize(
                config.structure.max_type_lines,
                defaults.structure.max_type_lines,
            ),
            max_type_members: configured_usize(
                config.structure.max_type_members,
                defaults.structure.max_type_members,
            ),
            max_imports: configured_usize(
                config.structure.max_imports,
                defaults.structure.max_imports,
            ),
            max_public_items: configured_usize(
                config.structure.max_public_items,
                defaults.structure.max_public_items,
            ),
            max_functions_per_file: configured_usize(
                config.structure.max_functions_per_file,
                defaults.structure.max_functions_per_file,
            ),
            max_functions_per_100_lines: configured_usize(
                config.structure.max_functions_per_100_lines,
                defaults.structure.max_functions_per_100_lines,
            ),
            max_small_function_ratio: configured_usize(
                config.structure.max_small_function_ratio,
                defaults.structure.max_small_function_ratio,
            ),
            min_repeated_literal_occurrences: configured_usize(
                config.repetition.min_repeated_literal_occurrences,
                defaults.repetition.min_repeated_literal_occurrences,
            ),
            min_data_clump_occurrences: configured_usize(
                config.repetition.min_data_clump_occurrences,
                defaults.repetition.min_data_clump_occurrences,
            ),
            max_unity_assembly_dependencies: configured_usize(
                config.unity.max_assembly_dependencies,
                defaults.unity.max_assembly_dependencies,
            ),
            max_unity_scene_objects: configured_usize(
                config.unity.max_scene_objects,
                defaults.unity.max_scene_objects,
            ),
            max_unity_prefab_objects: configured_usize(
                config.unity.max_prefab_objects,
                defaults.unity.max_prefab_objects,
            ),
            max_unity_serialized_fields: configured_usize(
                config.unity.max_serialized_fields,
                defaults.unity.max_serialized_fields,
            ),
            max_unity_lifecycle_methods: configured_usize(
                config.unity.max_lifecycle_methods,
                defaults.unity.max_lifecycle_methods,
            ),
        }
    }

    fn apply(self, args: &mut ScanArgs) {
        apply_optional(&mut args.max_file_lines, self.max_file_lines);
        apply_optional(&mut args.max_dir_files, self.max_dir_files);
        apply_optional(&mut args.min_similar_functions, self.min_similar_functions);
        apply_optional(&mut args.min_function_tokens, self.min_function_tokens);
        apply_optional(&mut args.function_similarity, self.function_similarity);
        apply_optional(&mut args.max_function_lines, self.max_function_lines);
        apply_optional(
            &mut args.max_function_complexity,
            self.max_function_complexity,
        );
        apply_optional(&mut args.max_nesting_depth, self.max_nesting_depth);
        apply_optional(
            &mut args.max_function_parameters,
            self.max_function_parameters,
        );
        apply_optional(&mut args.max_type_lines, self.max_type_lines);
        apply_optional(&mut args.max_type_members, self.max_type_members);
        apply_optional(&mut args.max_imports, self.max_imports);
        apply_optional(&mut args.max_public_items, self.max_public_items);
        apply_optional(
            &mut args.function_proliferation.max_functions_per_file,
            self.max_functions_per_file,
        );
        apply_optional(
            &mut args.function_proliferation.max_functions_per_100_lines,
            self.max_functions_per_100_lines,
        );
        apply_optional(
            &mut args.function_proliferation.max_small_function_ratio,
            self.max_small_function_ratio,
        );
        apply_optional(
            &mut args.min_repeated_literal_occurrences,
            self.min_repeated_literal_occurrences,
        );
        apply_optional(
            &mut args.min_data_clump_occurrences,
            self.min_data_clump_occurrences,
        );
        apply_optional(
            &mut args.max_unity_assembly_dependencies,
            self.max_unity_assembly_dependencies,
        );
        apply_optional(
            &mut args.max_unity_scene_objects,
            self.max_unity_scene_objects,
        );
        apply_optional(
            &mut args.max_unity_prefab_objects,
            self.max_unity_prefab_objects,
        );
        apply_optional(
            &mut args.max_unity_serialized_fields,
            self.max_unity_serialized_fields,
        );
        apply_optional(
            &mut args.max_unity_lifecycle_methods,
            self.max_unity_lifecycle_methods,
        );
    }

    fn apply_unless(self, args: &mut ScanArgs, blocked: &Self) {
        Self {
            max_file_lines: self
                .max_file_lines
                .filter(|_| blocked.max_file_lines.is_none()),
            max_dir_files: self
                .max_dir_files
                .filter(|_| blocked.max_dir_files.is_none()),
            min_similar_functions: self
                .min_similar_functions
                .filter(|_| blocked.min_similar_functions.is_none()),
            min_function_tokens: self
                .min_function_tokens
                .filter(|_| blocked.min_function_tokens.is_none()),
            function_similarity: self
                .function_similarity
                .filter(|_| blocked.function_similarity.is_none()),
            max_function_lines: self
                .max_function_lines
                .filter(|_| blocked.max_function_lines.is_none()),
            max_function_complexity: self
                .max_function_complexity
                .filter(|_| blocked.max_function_complexity.is_none()),
            max_nesting_depth: self
                .max_nesting_depth
                .filter(|_| blocked.max_nesting_depth.is_none()),
            max_function_parameters: self
                .max_function_parameters
                .filter(|_| blocked.max_function_parameters.is_none()),
            max_type_lines: self
                .max_type_lines
                .filter(|_| blocked.max_type_lines.is_none()),
            max_type_members: self
                .max_type_members
                .filter(|_| blocked.max_type_members.is_none()),
            max_imports: self.max_imports.filter(|_| blocked.max_imports.is_none()),
            max_public_items: self
                .max_public_items
                .filter(|_| blocked.max_public_items.is_none()),
            max_functions_per_file: self
                .max_functions_per_file
                .filter(|_| blocked.max_functions_per_file.is_none()),
            max_functions_per_100_lines: self
                .max_functions_per_100_lines
                .filter(|_| blocked.max_functions_per_100_lines.is_none()),
            max_small_function_ratio: self
                .max_small_function_ratio
                .filter(|_| blocked.max_small_function_ratio.is_none()),
            min_repeated_literal_occurrences: self
                .min_repeated_literal_occurrences
                .filter(|_| blocked.min_repeated_literal_occurrences.is_none()),
            min_data_clump_occurrences: self
                .min_data_clump_occurrences
                .filter(|_| blocked.min_data_clump_occurrences.is_none()),
            max_unity_assembly_dependencies: self
                .max_unity_assembly_dependencies
                .filter(|_| blocked.max_unity_assembly_dependencies.is_none()),
            max_unity_scene_objects: self
                .max_unity_scene_objects
                .filter(|_| blocked.max_unity_scene_objects.is_none()),
            max_unity_prefab_objects: self
                .max_unity_prefab_objects
                .filter(|_| blocked.max_unity_prefab_objects.is_none()),
            max_unity_serialized_fields: self
                .max_unity_serialized_fields
                .filter(|_| blocked.max_unity_serialized_fields.is_none()),
            max_unity_lifecycle_methods: self
                .max_unity_lifecycle_methods
                .filter(|_| blocked.max_unity_lifecycle_methods.is_none()),
        }
        .apply(args);
    }
}

fn cli_usize(value: usize, default: usize, explicit: bool) -> Option<usize> {
    (explicit || value != default).then_some(value)
}

fn cli_f64(value: f64, default: f64, explicit: bool) -> Option<f64> {
    (explicit || (value - default).abs() >= f64::EPSILON).then_some(value)
}

fn configured_usize(value: Option<usize>, default: usize) -> Option<usize> {
    value.filter(|value| *value != default)
}

fn configured_f64(value: Option<f64>, default: f64) -> Option<f64> {
    value.filter(|value| (*value - default).abs() >= f64::EPSILON)
}

fn apply_optional<T>(target: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *target = value;
    }
}
