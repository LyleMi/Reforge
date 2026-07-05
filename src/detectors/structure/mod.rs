use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{
    ARROW_FUNCTION, BODY_FIELD, FUNCTION_DECLARATION, FUNCTION_DEFINITION, FUNCTION_ITEM,
    GENERATOR_FUNCTION_DECLARATION, LanguageFamily, METHOD_DECLARATION, METHOD_DEFINITION,
    NAME_FIELD, PARAMETERS_FIELD, adapter_for_path,
};
use crate::scanner::{Finding, FindingKind, FindingMetric, RelatedLocation, is_test_source};
use crate::similar_functions::{ParsedSourceFile, SourceFile, parse_source_files};

#[derive(Debug, Clone)]
pub struct StructureOptions {
    pub max_function_lines: usize,
    pub max_function_complexity: usize,
    pub max_nesting_depth: usize,
    pub max_function_parameters: usize,
    pub max_type_lines: usize,
    pub max_type_members: usize,
    pub max_imports: usize,
    pub max_public_items: usize,
    pub min_repeated_literal_occurrences: usize,
    pub min_data_clump_occurrences: usize,
    pub max_dir_files: usize,
    pub include_test_structure: bool,
}

#[derive(Debug, Clone)]
struct FunctionMetric {
    name: String,
    line: usize,
    lines: usize,
    parameter_count: usize,
    parameter_names: Vec<String>,
    complexity: usize,
    nesting_depth: usize,
}

#[derive(Debug, Clone)]
struct TypeMetric {
    name: String,
    line: usize,
    lines: usize,
    members: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawStructureFileMetric {
    pub path: String,
    pub imports: usize,
    pub public_items: usize,
    pub is_test: bool,
    pub functions: Vec<RawStructureFunctionMetric>,
    pub types: Vec<RawStructureTypeMetric>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawStructureFunctionMetric {
    pub path: String,
    pub name: String,
    pub line: usize,
    pub loc: usize,
    pub complexity: usize,
    pub nesting_depth: usize,
    pub parameter_count: usize,
    pub is_test: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawStructureTypeMetric {
    pub path: String,
    pub name: String,
    pub line: usize,
    pub loc: usize,
    pub member_count: usize,
    pub is_test: bool,
}

type Occurrence = RelatedLocation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FileNamingStyle {
    SnakeCase,
    KebabCase,
    PascalCase,
    CamelCase,
    Lowercase,
    DotSeparated,
    Mixed,
}

#[derive(Debug, Default)]
struct NamingDirectory {
    display_path: String,
    styles: BTreeMap<FileNamingStyle, Vec<Occurrence>>,
}

#[derive(Debug, Default)]
struct FileSignals {
    findings: Vec<Finding>,
    literals: Vec<(String, Occurrence)>,
    error_patterns: Vec<(String, Occurrence)>,
    data_clumps: Vec<(String, Occurrence)>,
    test_setups: Vec<(String, Occurrence)>,
    happy_path_test_files: Vec<(usize, Vec<Occurrence>)>,
    naming_directories: BTreeMap<PathBuf, NamingDirectory>,
    directory_files: BTreeMap<PathBuf, BTreeSet<String>>,
}

#[derive(Debug, Default)]
struct ProductionAstSignals {
    functions: Vec<FunctionMetric>,
    types: Vec<TypeMetric>,
}

#[derive(Debug, Clone, Copy)]
struct StructureTraversal<'a> {
    source: &'a str,
    family: LanguageFamily,
    include_test_structure: bool,
}

struct StructureSignalCollector<'a, 'signals> {
    file: &'a SourceFile,
    traversal: StructureTraversal<'a>,
    signals: &'signals mut FileSignals,
}

#[allow(dead_code)]
pub fn scan_structure(files: &[SourceFile], options: &StructureOptions) -> Result<Vec<Finding>> {
    let parsed_files = parse_source_files(files)?;
    scan_parsed_structure(&parsed_files, options)
}

pub(crate) fn scan_parsed_structure(
    files: &[ParsedSourceFile],
    options: &StructureOptions,
) -> Result<Vec<Finding>> {
    let mut signals = FileSignals::default();

    for file in files {
        collect_file_naming_style(&file.file, &mut signals);

        let is_test = is_test_source(&file.file.path);
        if !is_test || options.include_test_structure {
            scan_production_file(
                &file.file,
                file.family,
                file.tree.root_node(),
                options,
                &mut signals,
            );
        }

        if is_test {
            collect_test_setup_patterns(&file.file, file.tree.root_node(), &mut signals);
            collect_happy_path_test_risk(&file.file, file.family, &mut signals);
        }
    }

    signals.findings.extend(group_occurrences(
        signals.literals,
        options.min_repeated_literal_occurrences,
        FindingKind::RepeatedLiteral,
        |literal, count| format!("literal {literal:?} is repeated {count} times"),
    ));
    signals.findings.extend(group_occurrences(
        signals.error_patterns,
        options.min_repeated_literal_occurrences,
        FindingKind::RepeatedErrorPattern,
        |_, count| format!("error-handling pattern is repeated {count} times"),
    ));
    signals.findings.extend(group_occurrences(
        signals.data_clumps,
        options.min_data_clump_occurrences,
        FindingKind::DataClump,
        |clump, count| format!("parameter group ({clump}) appears in {count} functions"),
    ));
    signals.findings.extend(group_occurrences(
        signals.test_setups,
        options.min_data_clump_occurrences,
        FindingKind::TestDuplication,
        |_, count| format!("test setup pattern is repeated {count} times"),
    ));
    signals
        .findings
        .extend(happy_path_test_findings(signals.happy_path_test_files));
    signals
        .findings
        .extend(file_naming_drift_findings(&signals.naming_directories));
    signals
        .findings
        .extend(directory_drift_findings(&signals.directory_files, options));

    Ok(signals.findings)
}

pub(crate) fn collect_raw_structure_metrics(
    files: &[ParsedSourceFile],
) -> Vec<RawStructureFileMetric> {
    files
        .iter()
        .map(|file| {
            let root = file.tree.root_node();
            let is_test = is_test_source(&file.file.path);
            let traversal = StructureTraversal {
                source: &file.file.source,
                family: file.family,
                include_test_structure: true,
            };
            let mut signals = FileSignals::default();
            let ast_signals =
                collect_production_ast_signals(&file.file, root, traversal, &mut signals);
            let path = file.file.display_path.clone();
            RawStructureFileMetric {
                path: path.clone(),
                imports: count_imports(root, file.family),
                public_items: count_public_items(root, traversal),
                is_test,
                functions: ast_signals
                    .functions
                    .into_iter()
                    .map(|function| RawStructureFunctionMetric {
                        path: path.clone(),
                        name: function.name,
                        line: function.line,
                        loc: function.lines,
                        complexity: function.complexity,
                        nesting_depth: function.nesting_depth,
                        parameter_count: function.parameter_count,
                        is_test,
                    })
                    .collect(),
                types: ast_signals
                    .types
                    .into_iter()
                    .map(|type_metric| RawStructureTypeMetric {
                        path: path.clone(),
                        name: type_metric.name,
                        line: type_metric.line,
                        loc: type_metric.lines,
                        member_count: type_metric.members,
                        is_test,
                    })
                    .collect(),
            }
        })
        .collect()
}

pub fn is_supported_structure_source(path: &Path) -> bool {
    adapter_for_path(path).is_some()
}

fn scan_production_file(
    file: &SourceFile,
    family: LanguageFamily,
    root: Node<'_>,
    options: &StructureOptions,
    signals: &mut FileSignals,
) {
    let traversal = StructureTraversal {
        source: &file.source,
        family,
        include_test_structure: options.include_test_structure,
    };

    let ast_signals = collect_production_ast_signals(file, root, traversal, signals);
    scan_function_metrics(file, &ast_signals.functions, options, signals);
    scan_type_metrics(file, &ast_signals.types, options, signals);
    scan_file_metrics(file, root, traversal, options, signals);
    collect_cross_file_patterns(file, root, traversal, signals);
}

fn scan_function_metrics(
    file: &SourceFile,
    functions: &[FunctionMetric],
    options: &StructureOptions,
    signals: &mut FileSignals,
) {
    for function in functions {
        if function.lines > options.max_function_lines {
            signals.findings.push(crate::scanner::finding(
                FindingKind::LongFunction,
                file.display_path.clone(),
                Some(function.line),
                format!(
                    "function `{}` spans {} lines; consider extracting smaller steps",
                    function.name, function.lines
                ),
                vec![FindingMetric::threshold(
                    "function_lines",
                    function.lines,
                    options.max_function_lines,
                    "lines",
                )],
                Vec::new(),
            ));
        }

        if function.complexity > options.max_function_complexity {
            signals.findings.push(crate::scanner::finding(
                FindingKind::ComplexFunction,
                file.display_path.clone(),
                Some(function.line),
                format!(
                    "function `{}` has estimated complexity {}; consider reducing branches",
                    function.name, function.complexity
                ),
                vec![FindingMetric::threshold(
                    "function_complexity",
                    function.complexity,
                    options.max_function_complexity,
                    "complexity",
                )],
                Vec::new(),
            ));
        }

        if function.nesting_depth > options.max_nesting_depth {
            signals.findings.push(crate::scanner::finding(
                FindingKind::DeepNesting,
                file.display_path.clone(),
                Some(function.line),
                format!(
                    "function `{}` nests control flow {} levels deep",
                    function.name, function.nesting_depth
                ),
                vec![FindingMetric::threshold(
                    "nesting_depth",
                    function.nesting_depth,
                    options.max_nesting_depth,
                    "levels",
                )],
                Vec::new(),
            ));
        }

        if function.parameter_count > options.max_function_parameters {
            signals.findings.push(crate::scanner::finding(
                FindingKind::ManyParameters,
                file.display_path.clone(),
                Some(function.line),
                format!(
                    "function `{}` has {} parameters; consider grouping related data",
                    function.name, function.parameter_count
                ),
                vec![FindingMetric::threshold(
                    "function_parameters",
                    function.parameter_count,
                    options.max_function_parameters,
                    "parameters",
                )],
                Vec::new(),
            ));
        }

        collect_data_clumps(file, function, options, signals);
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
            signals.findings.push(crate::scanner::finding(
                FindingKind::LargeType,
                file.display_path.clone(),
                Some(type_metric.line),
                format!(
                    "type `{}` spans {} lines and has {} members; consider splitting responsibilities",
                    type_metric.name, type_metric.lines, type_metric.members
                ),
                vec![
                    FindingMetric::threshold(
                        "type_lines",
                        type_metric.lines,
                        options.max_type_lines,
                        "lines",
                    ),
                    FindingMetric::threshold(
                        "type_members",
                        type_metric.members,
                        options.max_type_members,
                        "members",
                    ),
                ],
                Vec::new(),
            ));
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
        signals.findings.push(crate::scanner::finding(
            FindingKind::ImportHeavyFile,
            file.display_path.clone(),
            Some(1),
            format!("file has {imports} imports; consider reducing module coupling"),
            vec![FindingMetric::threshold(
                "imports",
                imports,
                options.max_imports,
                "imports",
            )],
            Vec::new(),
        ));
    }

    let public_items = count_public_items(root, traversal);
    if public_items > options.max_public_items {
        signals.findings.push(crate::scanner::finding(
            FindingKind::LargePublicSurface,
            file.display_path.clone(),
            Some(1),
            format!("file exposes {public_items} public/exported items"),
            vec![FindingMetric::threshold(
                "public_items",
                public_items,
                options.max_public_items,
                "items",
            )],
            Vec::new(),
        ));
    }
}

fn collect_cross_file_patterns(
    file: &SourceFile,
    _root: Node<'_>,
    traversal: StructureTraversal<'_>,
    signals: &mut FileSignals,
) {
    collect_directory_concepts(file, traversal.family, signals);
}

fn collect_production_ast_signals(
    file: &SourceFile,
    root: Node<'_>,
    traversal: StructureTraversal<'_>,
    signals: &mut FileSignals,
) -> ProductionAstSignals {
    let mut ast_signals = ProductionAstSignals::default();
    let mut collector = StructureSignalCollector {
        file,
        traversal,
        signals,
    };
    collect_production_ast_signals_from(root, traversal, &mut collector, &mut ast_signals);
    ast_signals
}

fn collect_production_ast_signals_from(
    node: Node<'_>,
    traversal: StructureTraversal<'_>,
    collector: &mut StructureSignalCollector<'_, '_>,
    ast_signals: &mut ProductionAstSignals,
) {
    if should_skip_rust_test_module(node, traversal) {
        return;
    }

    if let Some(parts) = function_parts(node, traversal) {
        let parameter_names = parameter_names(parts.parameters, traversal.source, traversal.family);
        ast_signals.functions.push(FunctionMetric {
            name: parts.name,
            line: node.start_position().row + 1,
            lines: node_line_span(node),
            parameter_count: parameter_names.len(),
            parameter_names,
            complexity: complexity(parts.body, traversal),
            nesting_depth: max_nesting_depth(parts.body, traversal.family, 0),
        });
    }

    if let Some(metric) = type_metric(node, traversal) {
        ast_signals.types.push(metric);
    }

    collector.collect_literal_occurrence(node);
    collector.collect_error_occurrence(node);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_production_ast_signals_from(child, traversal, collector, ast_signals);
    }
}

struct FunctionParts<'tree> {
    name: String,
    parameters: Option<Node<'tree>>,
    body: Node<'tree>,
}

fn function_parts<'tree>(
    node: Node<'tree>,
    traversal: StructureTraversal<'_>,
) -> Option<FunctionParts<'tree>> {
    let kind = node.kind();
    let source = traversal.source;
    match traversal.family {
        LanguageFamily::Rust if kind == FUNCTION_ITEM => Some(FunctionParts {
            name: node
                .child_by_field_name(NAME_FIELD)?
                .utf8_text(source.as_bytes())
                .ok()?
                .to_string(),
            parameters: node.child_by_field_name(PARAMETERS_FIELD),
            body: node.child_by_field_name(BODY_FIELD)?,
        }),
        LanguageFamily::JavaScriptTypeScript
            if matches!(
                kind,
                FUNCTION_DECLARATION
                    | GENERATOR_FUNCTION_DECLARATION
                    | METHOD_DEFINITION
                    | ARROW_FUNCTION
            ) =>
        {
            Some(FunctionParts {
                name: function_name(node, source).unwrap_or_else(|| "<anonymous>".to_string()),
                parameters: node.child_by_field_name(PARAMETERS_FIELD),
                body: node.child_by_field_name(BODY_FIELD)?,
            })
        }
        LanguageFamily::Python if kind == FUNCTION_DEFINITION => Some(FunctionParts {
            name: node
                .child_by_field_name(NAME_FIELD)?
                .utf8_text(source.as_bytes())
                .ok()?
                .to_string(),
            parameters: node.child_by_field_name(PARAMETERS_FIELD),
            body: node.child_by_field_name(BODY_FIELD)?,
        }),
        LanguageFamily::Go if matches!(kind, FUNCTION_DECLARATION | METHOD_DECLARATION) => {
            Some(FunctionParts {
                name: node
                    .child_by_field_name(NAME_FIELD)?
                    .utf8_text(source.as_bytes())
                    .ok()?
                    .to_string(),
                parameters: node.child_by_field_name(PARAMETERS_FIELD),
                body: node.child_by_field_name(BODY_FIELD)?,
            })
        }
        _ => None,
    }
}

fn function_name(node: Node<'_>, source: &str) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .map(ToString::to_string)
}

fn parameter_names(
    parameters: Option<Node<'_>>,
    source: &str,
    family: LanguageFamily,
) -> Vec<String> {
    let Some(parameters) = parameters else {
        return Vec::new();
    };

    match family {
        LanguageFamily::Rust => rust_parameter_names(parameters, source),
        LanguageFamily::Go => go_parameter_names(parameters, source),
        _ => {
            let mut names = Vec::new();
            let mut cursor = parameters.walk();
            for child in parameters.named_children(&mut cursor) {
                collect_parameter_name(child, source, &mut names);
            }
            names
        }
    }
}

fn rust_parameter_names(parameters: Node<'_>, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = parameters.walk();
    for child in parameters.named_children(&mut cursor) {
        match child.kind() {
            "self_parameter" => {}
            "parameter" => {
                if let Some(pattern) = child.child_by_field_name("pattern") {
                    collect_parameter_name(pattern, source, &mut names);
                } else {
                    collect_parameter_name(child, source, &mut names);
                }
            }
            _ => collect_parameter_name(child, source, &mut names),
        }
    }
    names
}

fn go_parameter_names(parameters: Node<'_>, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = parameters.walk();
    for child in parameters.named_children(&mut cursor) {
        if child.kind() != "parameter_declaration" {
            continue;
        }

        let before_type = child
            .children_by_field_name("name", &mut child.walk())
            .filter_map(|name| name.utf8_text(source.as_bytes()).ok())
            .map(normalize_identifier)
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>();

        if before_type.is_empty() {
            names.push("value".to_string());
        } else {
            names.extend(before_type);
        }
    }
    names
}

fn collect_parameter_name(node: Node<'_>, source: &str, names: &mut Vec<String>) {
    match node.kind() {
        "identifier" | "field_identifier" | "shorthand_property_identifier" => {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                let name = normalize_identifier(text);
                if !name.is_empty() && name != "self" {
                    names.push(name);
                }
            }
        }
        "type_identifier" | "primitive_type" => {}
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_parameter_name(child, source, names);
            }
        }
    }
}

fn collect_data_clumps(
    file: &SourceFile,
    function: &FunctionMetric,
    options: &StructureOptions,
    signals: &mut FileSignals,
) {
    let names = function
        .parameter_names
        .iter()
        .filter(|name| name.len() > 1)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    if names.len() < 3 || options.min_data_clump_occurrences == 0 {
        return;
    }

    signals.data_clumps.push((
        names.join(", "),
        Occurrence {
            path: file.display_path.clone(),
            line: function.line,
            name: Some(function.name.clone()),
        },
    ));
}

fn complexity(node: Node<'_>, traversal: StructureTraversal<'_>) -> usize {
    let mut score = 1;
    add_complexity(node, traversal, &mut score);
    score
}

fn add_complexity(node: Node<'_>, traversal: StructureTraversal<'_>, score: &mut usize) {
    if is_decision_node(node, traversal) {
        *score += 1;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        add_complexity(child, traversal, score);
    }
}

fn max_nesting_depth(node: Node<'_>, family: LanguageFamily, current_depth: usize) -> usize {
    let next_depth = if is_nesting_node(node, family) {
        current_depth + 1
    } else {
        current_depth
    };

    let mut max_depth = next_depth;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        max_depth = max_depth.max(max_nesting_depth(child, family, next_depth));
    }
    max_depth
}

fn is_decision_node(node: Node<'_>, traversal: StructureTraversal<'_>) -> bool {
    let kind = node.kind();
    if matches!(
        kind,
        "if_expression"
            | "if_statement"
            | "for_expression"
            | "for_statement"
            | "while_expression"
            | "while_statement"
            | "loop_expression"
            | "match_expression"
            | "match_arm"
            | "switch_statement"
            | "case_clause"
            | "catch_clause"
            | "except_clause"
            | "conditional_expression"
    ) {
        return true;
    }

    if kind != "binary_expression" && kind != "boolean_operator" {
        return false;
    }

    node.utf8_text(traversal.source.as_bytes())
        .ok()
        .is_some_and(|text| {
            text.contains("&&")
                || text.contains("||")
                || (traversal.family == LanguageFamily::Python
                    && (text.contains(" and ") || text.contains(" or ")))
        })
}

fn is_nesting_node(node: Node<'_>, family: LanguageFamily) -> bool {
    let kind = node.kind();
    matches!(
        kind,
        "if_expression"
            | "if_statement"
            | "for_expression"
            | "for_statement"
            | "while_expression"
            | "while_statement"
            | "loop_expression"
            | "match_expression"
            | "switch_statement"
            | "case_clause"
            | "catch_clause"
            | "except_clause"
    ) || (family == LanguageFamily::Python && kind == "elif_clause")
}

mod analysis;

use analysis::*;

#[cfg(test)]
#[path = "../../structural_tests.rs"]
mod tests;
