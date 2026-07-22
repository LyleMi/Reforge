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
            data_flow: DataFlowConfig::default(),
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
            data_flow: DataFlowConfig::default(),
        }
    }
}
