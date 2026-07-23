use super::*;

pub(super) struct ScanSignalContext<'a> {
    pub(super) root: &'a Path,
    pub(super) args: &'a EffectiveConfig,
    pub(super) data_flow: &'a config::DataFlowConfig,
    pub(super) plan: ExecutionPlan,
    pub(super) progress: &'a mut dyn ProgressSink,
    pub(super) scan: &'a mut WorkspaceIndex,
}

impl ScanSignalContext<'_> {
    pub(super) fn run(&mut self) -> Result<()> {
        if self.plan.codebase {
            self.scan_structural_signals()?;
            self.scan_unused_function_signals();
            self.scan_dependency_graph_signals();
            self.scan_concept_drift_signals();
            self.scan_similarity_signals()?;
        }
        if self.plan.dataflow {
            self.scan_data_flow_signals()?;
        }
        Ok(())
    }

    fn scan_data_flow_signals(&mut self) -> Result<()> {
        self.progress.report(&format!(
            "Analyzing exact and conservative Dataflow paths in {} parsed files",
            self.scan.parsed_sources.len()
        ));
        let mut flow = crate::detectors::data_flow::scan_data_flow_with_ir(
            self.root,
            &self.scan.parsed_sources,
            &self.scan.parse_failures,
            self.data_flow,
            self.plan.materialize_flow_ir,
        )?;
        self.scan.detections.append(&mut flow.detections);
        self.scan.flow_analysis = flow.summary;
        Ok(())
    }

    fn scan_dependency_graph_signals(&mut self) {
        self.progress.report(&format!(
            "Analyzing dependency graph in {} files",
            self.scan.codebase_sources.len()
        ));
        let dependency_scan = scan_dependency_graph_report(&self.scan.codebase_sources, self.root);
        self.scan.unresolved_dependency_edges = dependency_scan.unresolved_edges;
        self.scan.unresolved_dependency_edges_by_file = dependency_scan.unresolved_by_file;
        self.scan.dependency_graph = dependency_scan.snapshot;
        self.scan.detections.extend(dependency_scan.detections);
    }

    fn scan_concept_drift_signals(&mut self) {
        self.progress.report(&format!(
            "Analyzing concept drift signals in {} files",
            self.scan.codebase_sources.len()
        ));
        let options = ConceptDriftOptions {
            min_repeated_occurrences: self.args.min_repeated_literal_occurrences,
            min_data_shape_occurrences: self.args.min_data_clump_occurrences,
            max_dir_files: self.args.max_dir_files,
            include_test_structure: false,
        };
        self.scan
            .detections
            .extend(scan_concept_drift(&self.scan.codebase_sources, &options));
    }

    fn scan_structural_signals(&mut self) -> Result<()> {
        self.progress.report(&format!(
            "Analyzing structural signals in {} files",
            self.scan.parsed_sources.len()
        ));
        let structure_options = StructureOptions {
            max_function_lines: self.args.max_function_lines,
            max_function_complexity: self.args.max_function_complexity,
            max_nesting_depth: self.args.max_nesting_depth,
            max_function_parameters: self.args.max_function_parameters,
            max_type_lines: self.args.max_type_lines,
            max_type_members: self.args.max_type_members,
            max_imports: self.args.max_imports,
            max_public_items: self.args.max_public_items,
            max_functions_per_file: self.args.function_proliferation.max_functions_per_file,
            max_functions_per_100_lines: self
                .args
                .function_proliferation
                .max_functions_per_100_lines,
            max_small_function_ratio: self.args.function_proliferation.max_small_function_ratio,
            min_repeated_literal_occurrences: self.args.min_repeated_literal_occurrences,
            min_data_clump_occurrences: self.args.min_data_clump_occurrences,
            max_dir_files: self.args.max_dir_files,
            include_test_structure: false,
        };
        self.scan
            .detections
            .extend(crate::detectors::structure::scan_parsed_structure(
                &self.scan.parsed_sources,
                &structure_options,
            )?);
        self.scan
            .detections
            .extend(crate::detectors::documentation::scan_documentation(
                self.root,
            )?);
        Ok(())
    }

    fn scan_unused_function_signals(&mut self) {
        self.progress.report(&format!(
            "Analyzing unused functions in {} files",
            self.scan.parsed_sources.len()
        ));
        let options = UnusedFunctionOptions {
            include_tests: false,
        };
        self.scan.detections.extend(scan_parsed_unused_functions(
            &self.scan.parsed_sources,
            &options,
        ));
    }

    fn scan_similarity_signals(&mut self) -> Result<usize> {
        self.progress.report(&format!(
            "Analyzing similar functions in {} files",
            self.scan.parsed_sources.len()
        ));
        let similarity_options = SimilarFunctionOptions {
            min_group_size: self.args.min_similar_functions,
            min_tokens: self.args.min_function_tokens,
            threshold: self.args.function_similarity,
            include_test_similarity: false,
        };
        let mut similarity_progress = ScanSimilarityProgress {
            progress: self.progress,
        };
        let similarity_scan = scan_parsed_similar_functions_report_with_progress(
            &self.scan.parsed_sources,
            &similarity_options,
            &mut similarity_progress,
        )?;
        self.scan.stats.function_candidates = similarity_scan.candidate_count;
        self.scan.similarity_comparisons = similarity_scan.comparison_stats;
        self.scan.detections.extend(similarity_scan.detections);
        Ok(self
            .scan
            .detections
            .iter()
            .filter(|detection| detection.kind == Rule::SimilarFunctions)
            .count())
    }
}
