fn scan_function_metrics(
    file: &SourceFile,
    functions: &[FunctionMetric],
    options: &StructureOptions,
    signals: &mut FileSignals,
) {
    for function in functions {
        let mut readability_signals = Vec::new();
        for signal in FUNCTION_FINDING_SIGNALS {
            if signal.exceeds(function, options) {
                readability_signals.push(signal);
            }
            push_function_threshold_finding(file, function, options, signals, signal);
        }
        collect_data_clumps(file, function, options, signals);
    }
}

const FUNCTION_FINDING_SIGNALS: [FunctionFindingSignal; 4] = [
    FunctionFindingSignal::LongFunction,
    FunctionFindingSignal::ComplexFunction,
    FunctionFindingSignal::DeepNesting,
    FunctionFindingSignal::ManyParameters,
];

#[derive(Debug, Clone, Copy)]
enum FunctionFindingSignal {
    LongFunction,
    ComplexFunction,
    DeepNesting,
    ManyParameters,
}

fn push_function_threshold_finding(
    file: &SourceFile,
    function: &FunctionMetric,
    options: &StructureOptions,
    signals: &mut FileSignals,
    signal: FunctionFindingSignal,
) {
    if !signal.exceeds(function, options) {
        return;
    }
    let value = signal.value(function);
    let threshold = signal.threshold(options);

    signals
        .findings
        .push(crate::scanner::Finding::from(FindingInput::new(
            signal.kind(),
            file.display_path.clone(),
            Some(function.line),
            signal.message(function),
            vec![FindingMetric::threshold(
                signal.metric_name(),
                value,
                threshold,
                signal.unit(),
            )],
        )));
}

impl FunctionFindingSignal {
    fn exceeds(self, function: &FunctionMetric, options: &StructureOptions) -> bool {
        self.value(function) > self.threshold(options)
    }

    fn kind(self) -> FindingKind {
        match self {
            Self::LongFunction => FindingKind::LongFunction,
            Self::ComplexFunction => FindingKind::ComplexFunction,
            Self::DeepNesting => FindingKind::DeepNesting,
            Self::ManyParameters => FindingKind::ManyParameters,
        }
    }

    fn metric_name(self) -> MetricId {
        match self {
            Self::LongFunction => MetricId::FunctionLoc,
            Self::ComplexFunction => MetricId::FunctionComplexity,
            Self::DeepNesting => MetricId::FunctionNestingDepth,
            Self::ManyParameters => MetricId::FunctionParameterCount,
        }
    }

    fn unit(self) -> &'static str {
        match self {
            Self::LongFunction => "lines",
            Self::ComplexFunction => "complexity",
            Self::DeepNesting => "levels",
            Self::ManyParameters => "parameters",
        }
    }

    fn value(self, function: &FunctionMetric) -> usize {
        match self {
            Self::LongFunction => function.lines,
            Self::ComplexFunction => function.complexity,
            Self::DeepNesting => function.nesting_depth,
            Self::ManyParameters => function.parameter_count,
        }
    }

    fn threshold(self, options: &StructureOptions) -> usize {
        match self {
            Self::LongFunction => options.max_function_lines,
            Self::ComplexFunction => options.max_function_complexity,
            Self::DeepNesting => options.max_nesting_depth,
            Self::ManyParameters => options.max_function_parameters,
        }
    }

    fn message(self, function: &FunctionMetric) -> String {
        match self {
            Self::LongFunction => format!(
                "function `{}` spans {} lines; consider extracting smaller steps",
                function.name, function.lines
            ),
            Self::ComplexFunction => format!(
                "function `{}` has estimated complexity {}; consider reducing branches",
                function.name, function.complexity
            ),
            Self::DeepNesting => format!(
                "function `{}` nests control flow {} levels deep",
                function.name, function.nesting_depth
            ),
            Self::ManyParameters => format!(
                "function `{}` has {} parameters; consider grouping related data",
                function.name, function.parameter_count
            ),
        }
    }
}

fn scan_type_metrics(
    file: &SourceFile,
    types: &[TypeMetric],
    options: &StructureOptions,
    signals: &mut FileSignals,
) {
    for type_metric in types {
        if type_metric.lines > options.max_type_lines
            || type_metric.members > options.max_type_members
        {
            signals
                .findings
                .push(crate::scanner::Finding::from(FindingInput::new(
                FindingKind::LargeType,
                file.display_path.clone(),
                Some(type_metric.line),
                format!(
                    "type `{}` spans {} lines and has {} members; consider splitting responsibilities",
                    type_metric.name, type_metric.lines, type_metric.members
                ),
                vec![
                    FindingMetric::threshold(
                        MetricId::TypeLoc,
                        type_metric.lines,
                        options.max_type_lines,
                        "lines",
                    ),
                    FindingMetric::threshold(
                        MetricId::TypeMemberCount,
                        type_metric.members,
                        options.max_type_members,
                        "members",
                    ),
                ],
                )));
        }
    }
}

fn scan_file_metrics(
    file: &SourceFile,
    root: Node<'_>,
    traversal: StructureTraversal<'_>,
    options: &StructureOptions,
    signals: &mut FileSignals,
) {
    let imports = count_imports(root, traversal.family);
    if imports > options.max_imports {
        signals
            .findings
            .push(crate::scanner::Finding::from(FindingInput::new(
                FindingKind::ImportHeavyFile,
                file.display_path.clone(),
                Some(1),
                format!("file has {imports} imports; consider reducing module coupling"),
                vec![FindingMetric::threshold(
                    MetricId::FileImports,
                    imports,
                    options.max_imports,
                    "imports",
                )],
            )));
    }

    let public_items = count_public_items(root, traversal);
    if public_items > options.max_public_items {
        signals
            .findings
            .push(crate::scanner::Finding::from(FindingInput::new(
                FindingKind::LargePublicSurface,
                file.display_path.clone(),
                Some(1),
                format!("file exposes {public_items} public/exported items"),
                vec![FindingMetric::threshold(
                    MetricId::FilePublicItems,
                    public_items,
                    options.max_public_items,
                    "items",
                )],
            )));
    }
}

fn scan_function_proliferation(
    file: &SourceFile,
    root: Node<'_>,
    functions: &[FunctionMetric],
    options: &StructureOptions,
    signals: &mut FileSignals,
) {
    let function_count = functions.len();
    if function_count <= options.max_functions_per_file {
        return;
    }

    let file_lines = node_line_span(root).max(1);
    let functions_per_100_lines = function_count
        .saturating_mul(FUNCTION_DENSITY_LINE_UNIT)
        .div_ceil(file_lines);
    if functions_per_100_lines <= options.max_functions_per_100_lines {
        return;
    }

    let small_function_count = functions
        .iter()
        .filter(|function| is_small_simple_function(function))
        .count();
    let small_function_ratio =
        small_function_count.saturating_mul(FUNCTION_DENSITY_LINE_UNIT) / function_count;
    if small_function_ratio <= options.max_small_function_ratio {
        return;
    }

    signals
        .findings
        .push(crate::scanner::Finding::from(FindingInput::new(
            FindingKind::FunctionProliferation,
            file.display_path.clone(),
            Some(1),
            format!(
                "file defines {function_count} functions with {functions_per_100_lines} functions per 100 lines and {small_function_ratio}% small simple functions"
            ),
            vec![
                FindingMetric::threshold(
                    MetricId::FileFunctionCount,
                    function_count,
                    options.max_functions_per_file,
                    "functions",
                ),
                FindingMetric::threshold(
                    MetricId::FileFunctionsPerHundredLines,
                    functions_per_100_lines,
                    options.max_functions_per_100_lines,
                    "functions",
                ),
                FindingMetric::threshold(
                    MetricId::FileSmallFunctionRatio,
                    small_function_ratio,
                    options.max_small_function_ratio,
                    "percent",
                ),
            ],
        )));
}

fn is_small_simple_function(function: &FunctionMetric) -> bool {
    function.lines <= 5 && function.complexity <= 1 && function.parameter_count <= 3
}
