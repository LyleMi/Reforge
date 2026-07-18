pub const DEFAULT_MAX_FILE_LINES: usize = 800;
pub const DEFAULT_MAX_DIR_FILES: usize = 40;
pub const DEFAULT_MIN_SIMILAR_FUNCTIONS: usize = 3;
pub const DEFAULT_MIN_FUNCTION_TOKENS: usize = 80;
pub const DEFAULT_FUNCTION_SIMILARITY: f64 = 0.85;
pub const DEFAULT_MAX_FUNCTION_LINES: usize = 80;
pub const DEFAULT_MAX_FUNCTION_COMPLEXITY: usize = 15;
pub const DEFAULT_MAX_NESTING_DEPTH: usize = 4;
pub const DEFAULT_MAX_FUNCTION_PARAMETERS: usize = 5;
pub const DEFAULT_MAX_TYPE_LINES: usize = 250;
pub const DEFAULT_MAX_TYPE_MEMBERS: usize = 30;
pub const DEFAULT_MAX_IMPORTS: usize = 35;
pub const DEFAULT_MAX_PUBLIC_ITEMS: usize = 30;
pub const DEFAULT_MAX_FUNCTIONS_PER_FILE: usize = 40;
pub const DEFAULT_MAX_FUNCTIONS_PER_100_LINES: usize = 12;
pub const DEFAULT_MAX_SMALL_FUNCTION_RATIO: usize = 70;
pub const DEFAULT_MIN_REPEATED_LITERAL_OCCURRENCES: usize = 12;
pub const DEFAULT_MIN_DATA_CLUMP_OCCURRENCES: usize = 4;
pub const DEFAULT_CHURN_WINDOW_DAYS: usize = 180;
pub const DEFAULT_CHURN_MAX_COMMIT_LINES: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThresholdSettings {
    pub file: FileThresholdSettings,
    pub similarity: SimilarityThresholdSettings,
    pub structure: StructureThresholdSettings,
    pub repetition: RepetitionThresholdSettings,
    pub unity: UnityThresholdSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnityThresholdSettings {
    pub max_assembly_dependencies: usize,
    pub max_scene_objects: usize,
    pub max_prefab_objects: usize,
    pub max_serialized_fields: usize,
    pub max_lifecycle_methods: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileThresholdSettings {
    pub max_file_lines: usize,
    pub max_dir_files: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimilarityThresholdSettings {
    pub min_similar_functions: usize,
    pub min_function_tokens: usize,
    pub function_similarity: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructureThresholdSettings {
    pub max_function_lines: usize,
    pub max_function_complexity: usize,
    pub max_nesting_depth: usize,
    pub max_function_parameters: usize,
    pub max_type_lines: usize,
    pub max_type_members: usize,
    pub max_imports: usize,
    pub max_public_items: usize,
    pub max_functions_per_file: usize,
    pub max_functions_per_100_lines: usize,
    pub max_small_function_ratio: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepetitionThresholdSettings {
    pub min_repeated_literal_occurrences: usize,
    pub min_data_clump_occurrences: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ThresholdOverrideFlags {
    pub unity: bool,
    pub max_file_lines: bool,
    pub max_dir_files: bool,
    pub min_similar_functions: bool,
    pub min_function_tokens: bool,
    pub function_similarity: bool,
    pub max_function_lines: bool,
    pub max_function_complexity: bool,
    pub max_nesting_depth: bool,
    pub max_function_parameters: bool,
    pub max_type_lines: bool,
    pub max_type_members: bool,
    pub max_imports: bool,
    pub max_public_items: bool,
    pub max_functions_per_file: bool,
    pub max_functions_per_100_lines: bool,
    pub max_small_function_ratio: bool,
    pub min_repeated_literal_occurrences: bool,
    pub min_data_clump_occurrences: bool,
    pub max_unity_assembly_dependencies: bool,
    pub max_unity_scene_objects: bool,
    pub max_unity_prefab_objects: bool,
    pub max_unity_serialized_fields: bool,
    pub max_unity_lifecycle_methods: bool,
}

impl ThresholdSettings {
    pub const BALANCED: Self = Self {
        file: FileThresholdSettings {
            max_file_lines: DEFAULT_MAX_FILE_LINES,
            max_dir_files: DEFAULT_MAX_DIR_FILES,
        },
        similarity: SimilarityThresholdSettings {
            min_similar_functions: DEFAULT_MIN_SIMILAR_FUNCTIONS,
            min_function_tokens: DEFAULT_MIN_FUNCTION_TOKENS,
            function_similarity: DEFAULT_FUNCTION_SIMILARITY,
        },
        structure: StructureThresholdSettings {
            max_function_lines: DEFAULT_MAX_FUNCTION_LINES,
            max_function_complexity: DEFAULT_MAX_FUNCTION_COMPLEXITY,
            max_nesting_depth: DEFAULT_MAX_NESTING_DEPTH,
            max_function_parameters: DEFAULT_MAX_FUNCTION_PARAMETERS,
            max_type_lines: DEFAULT_MAX_TYPE_LINES,
            max_type_members: DEFAULT_MAX_TYPE_MEMBERS,
            max_imports: DEFAULT_MAX_IMPORTS,
            max_public_items: DEFAULT_MAX_PUBLIC_ITEMS,
            max_functions_per_file: DEFAULT_MAX_FUNCTIONS_PER_FILE,
            max_functions_per_100_lines: DEFAULT_MAX_FUNCTIONS_PER_100_LINES,
            max_small_function_ratio: DEFAULT_MAX_SMALL_FUNCTION_RATIO,
        },
        repetition: RepetitionThresholdSettings {
            min_repeated_literal_occurrences: DEFAULT_MIN_REPEATED_LITERAL_OCCURRENCES,
            min_data_clump_occurrences: DEFAULT_MIN_DATA_CLUMP_OCCURRENCES,
        },
        unity: UnityThresholdSettings {
            max_assembly_dependencies: 8,
            max_scene_objects: 1_000,
            max_prefab_objects: 250,
            max_serialized_fields: 16,
            max_lifecycle_methods: 7,
        },
    };

    pub const STRICT: Self = Self {
        file: FileThresholdSettings {
            max_file_lines: 600,
            max_dir_files: 30,
        },
        similarity: SimilarityThresholdSettings {
            min_similar_functions: 2,
            min_function_tokens: 60,
            function_similarity: 0.88,
        },
        structure: StructureThresholdSettings {
            max_function_lines: 60,
            max_function_complexity: 12,
            max_nesting_depth: 3,
            max_function_parameters: 4,
            max_type_lines: 200,
            max_type_members: 25,
            max_imports: 25,
            max_public_items: 20,
            max_functions_per_file: 35,
            max_functions_per_100_lines: 10,
            max_small_function_ratio: 65,
        },
        repetition: RepetitionThresholdSettings {
            min_repeated_literal_occurrences: 8,
            min_data_clump_occurrences: 3,
        },
        unity: UnityThresholdSettings {
            max_assembly_dependencies: 5,
            max_scene_objects: 500,
            max_prefab_objects: 100,
            max_serialized_fields: 10,
            max_lifecycle_methods: 5,
        },
    };

    pub const RELAXED: Self = Self {
        file: FileThresholdSettings {
            max_file_lines: 1_200,
            max_dir_files: 60,
        },
        similarity: SimilarityThresholdSettings {
            min_similar_functions: 4,
            min_function_tokens: 120,
            function_similarity: 0.90,
        },
        structure: StructureThresholdSettings {
            max_function_lines: 120,
            max_function_complexity: 20,
            max_nesting_depth: 5,
            max_function_parameters: 6,
            max_type_lines: 400,
            max_type_members: 45,
            max_imports: 50,
            max_public_items: 45,
            max_functions_per_file: 60,
            max_functions_per_100_lines: 18,
            max_small_function_ratio: 80,
        },
        repetition: RepetitionThresholdSettings {
            min_repeated_literal_occurrences: 20,
            min_data_clump_occurrences: 6,
        },
        unity: UnityThresholdSettings {
            max_assembly_dependencies: 12,
            max_scene_objects: 2_000,
            max_prefab_objects: 500,
            max_serialized_fields: 24,
            max_lifecycle_methods: 10,
        },
    };
}
