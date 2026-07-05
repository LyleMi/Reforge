use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tree_sitter::{Language, Node, Parser};

use crate::scanner::{Finding, FindingKind, RelatedLocation, Severity, is_test_source};
use crate::similar_functions::SourceFile;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LanguageFamily {
    Rust,
    JavaScriptTypeScript,
    Python,
    Go,
}

#[derive(Debug, Clone, Copy)]
struct LanguageAdapter {
    family: LanguageFamily,
    language: fn() -> Language,
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

#[derive(Debug, Clone)]
struct Occurrence {
    path: String,
    line: usize,
    name: Option<String>,
}

#[derive(Debug, Default)]
struct FileSignals {
    findings: Vec<Finding>,
    literals: Vec<(String, Occurrence)>,
    error_patterns: Vec<(String, Occurrence)>,
    data_clumps: Vec<(String, Occurrence)>,
    test_setups: Vec<(String, Occurrence)>,
    directory_files: BTreeMap<PathBuf, BTreeSet<String>>,
}

#[derive(Debug, Clone, Copy)]
struct StructureTraversal<'a> {
    source: &'a str,
    family: LanguageFamily,
    include_test_structure: bool,
}

pub fn scan_structure(files: &[SourceFile], options: &StructureOptions) -> Result<Vec<Finding>> {
    let mut signals = FileSignals::default();

    for file in files {
        let Some(adapter) = adapter_for_path(&file.path) else {
            continue;
        };

        let mut parser = Parser::new();
        parser
            .set_language(&(adapter.language)())
            .with_context(|| format!("failed to load parser for {}", file.display_path))?;

        let Some(tree) = parser.parse(&file.source, None) else {
            continue;
        };

        if tree.root_node().has_error() {
            continue;
        }

        let is_test = is_test_source(&file.path);
        if !is_test || options.include_test_structure {
            scan_production_file(
                file,
                adapter.family,
                tree.root_node(),
                options,
                &mut signals,
            );
        }

        if is_test {
            collect_test_setup_patterns(file, tree.root_node(), &mut signals);
        }
    }

    signals.findings.extend(group_occurrences(
        signals.literals,
        options.min_repeated_literal_occurrences,
        FindingKind::RepeatedLiteral,
        Severity::Info,
        |literal, count| format!("literal {literal:?} is repeated {count} times"),
    ));
    signals.findings.extend(group_occurrences(
        signals.error_patterns,
        options.min_repeated_literal_occurrences,
        FindingKind::RepeatedErrorPattern,
        Severity::Info,
        |_, count| format!("error-handling pattern is repeated {count} times"),
    ));
    signals.findings.extend(group_occurrences(
        signals.data_clumps,
        options.min_data_clump_occurrences,
        FindingKind::DataClump,
        Severity::Info,
        |clump, count| format!("parameter group ({clump}) appears in {count} functions"),
    ));
    signals.findings.extend(group_occurrences(
        signals.test_setups,
        options.min_data_clump_occurrences,
        FindingKind::TestDuplication,
        Severity::Info,
        |_, count| format!("test setup pattern is repeated {count} times"),
    ));
    signals
        .findings
        .extend(directory_drift_findings(&signals.directory_files, options));

    Ok(signals.findings)
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

    let functions = collect_function_metrics(root, traversal);
    for function in &functions {
        if function.lines > options.max_function_lines {
            signals.findings.push(Finding {
                kind: FindingKind::LongFunction,
                severity: Severity::Warning,
                path: file.display_path.clone(),
                line: Some(function.line),
                magnitude: Some(function.lines),
                message: format!(
                    "function `{}` spans {} lines; consider extracting smaller steps",
                    function.name, function.lines
                ),
                related_locations: Vec::new(),
            });
        }

        if function.complexity > options.max_function_complexity {
            signals.findings.push(Finding {
                kind: FindingKind::ComplexFunction,
                severity: Severity::Warning,
                path: file.display_path.clone(),
                line: Some(function.line),
                magnitude: Some(function.complexity),
                message: format!(
                    "function `{}` has estimated complexity {}; consider reducing branches",
                    function.name, function.complexity
                ),
                related_locations: Vec::new(),
            });
        }

        if function.nesting_depth > options.max_nesting_depth {
            signals.findings.push(Finding {
                kind: FindingKind::DeepNesting,
                severity: Severity::Warning,
                path: file.display_path.clone(),
                line: Some(function.line),
                magnitude: Some(function.nesting_depth),
                message: format!(
                    "function `{}` nests control flow {} levels deep",
                    function.name, function.nesting_depth
                ),
                related_locations: Vec::new(),
            });
        }

        if function.parameter_count > options.max_function_parameters {
            signals.findings.push(Finding {
                kind: FindingKind::ManyParameters,
                severity: Severity::Warning,
                path: file.display_path.clone(),
                line: Some(function.line),
                magnitude: Some(function.parameter_count),
                message: format!(
                    "function `{}` has {} parameters; consider grouping related data",
                    function.name, function.parameter_count
                ),
                related_locations: Vec::new(),
            });
        }

        collect_data_clumps(file, function, options, signals);
    }

    for type_metric in collect_type_metrics(root, traversal) {
        if type_metric.lines > options.max_type_lines
            || type_metric.members > options.max_type_members
        {
            signals.findings.push(Finding {
                kind: FindingKind::LargeType,
                severity: Severity::Warning,
                path: file.display_path.clone(),
                line: Some(type_metric.line),
                magnitude: Some(type_metric.lines.max(type_metric.members)),
                message: format!(
                    "type `{}` spans {} lines and has {} members; consider splitting responsibilities",
                    type_metric.name, type_metric.lines, type_metric.members
                ),
                related_locations: Vec::new(),
            });
        }
    }

    let imports = count_imports(root, family);
    if imports > options.max_imports {
        signals.findings.push(Finding {
            kind: FindingKind::ImportHeavyFile,
            severity: Severity::Warning,
            path: file.display_path.clone(),
            line: Some(1),
            magnitude: Some(imports),
            message: format!("file has {imports} imports; consider reducing module coupling"),
            related_locations: Vec::new(),
        });
    }

    let public_items = count_public_items(root, traversal);
    if public_items > options.max_public_items {
        signals.findings.push(Finding {
            kind: FindingKind::LargePublicSurface,
            severity: Severity::Warning,
            path: file.display_path.clone(),
            line: Some(1),
            magnitude: Some(public_items),
            message: format!("file exposes {public_items} public/exported items"),
            related_locations: Vec::new(),
        });
    }

    collect_repeated_literals(file, root, traversal, signals);
    collect_error_patterns(file, root, traversal, signals);
    collect_directory_concepts(file, family, signals);
}

fn collect_function_metrics(
    root: Node<'_>,
    traversal: StructureTraversal<'_>,
) -> Vec<FunctionMetric> {
    let mut functions = Vec::new();
    collect_function_metrics_from(root, traversal, &mut functions);
    functions
}

fn collect_function_metrics_from(
    node: Node<'_>,
    traversal: StructureTraversal<'_>,
    functions: &mut Vec<FunctionMetric>,
) {
    if should_skip_rust_test_module(node, traversal) {
        return;
    }

    if let Some(parts) = function_parts(node, traversal.source, traversal.family) {
        let parameter_names = parameter_names(parts.parameters, traversal.source, traversal.family);
        functions.push(FunctionMetric {
            name: parts.name,
            line: node.start_position().row + 1,
            lines: node_line_span(node),
            parameter_count: parameter_names.len(),
            parameter_names,
            complexity: complexity(parts.body, traversal.source, traversal.family),
            nesting_depth: max_nesting_depth(parts.body, traversal.family, 0),
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_function_metrics_from(child, traversal, functions);
    }
}

struct FunctionParts<'tree> {
    name: String,
    parameters: Option<Node<'tree>>,
    body: Node<'tree>,
}

fn function_parts<'tree>(
    node: Node<'tree>,
    source: &str,
    family: LanguageFamily,
) -> Option<FunctionParts<'tree>> {
    let kind = node.kind();
    match family {
        LanguageFamily::Rust if kind == "function_item" => Some(FunctionParts {
            name: node
                .child_by_field_name("name")?
                .utf8_text(source.as_bytes())
                .ok()?
                .to_string(),
            parameters: node.child_by_field_name("parameters"),
            body: node.child_by_field_name("body")?,
        }),
        LanguageFamily::JavaScriptTypeScript
            if matches!(
                kind,
                "function_declaration"
                    | "generator_function_declaration"
                    | "method_definition"
                    | "arrow_function"
            ) =>
        {
            Some(FunctionParts {
                name: function_name(node, source).unwrap_or_else(|| "<anonymous>".to_string()),
                parameters: node.child_by_field_name("parameters"),
                body: node.child_by_field_name("body")?,
            })
        }
        LanguageFamily::Python if kind == "function_definition" => Some(FunctionParts {
            name: node
                .child_by_field_name("name")?
                .utf8_text(source.as_bytes())
                .ok()?
                .to_string(),
            parameters: node.child_by_field_name("parameters"),
            body: node.child_by_field_name("body")?,
        }),
        LanguageFamily::Go if matches!(kind, "function_declaration" | "method_declaration") => {
            Some(FunctionParts {
                name: node
                    .child_by_field_name("name")?
                    .utf8_text(source.as_bytes())
                    .ok()?
                    .to_string(),
                parameters: node.child_by_field_name("parameters"),
                body: node.child_by_field_name("body")?,
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

    for first in 0..names.len() - 2 {
        for second in first + 1..names.len() - 1 {
            for third in second + 1..names.len() {
                signals.data_clumps.push((
                    format!("{}, {}, {}", names[first], names[second], names[third]),
                    Occurrence {
                        path: file.display_path.clone(),
                        line: function.line,
                        name: Some(function.name.clone()),
                    },
                ));
            }
        }
    }
}

fn complexity(node: Node<'_>, source: &str, family: LanguageFamily) -> usize {
    let mut score = 1;
    add_complexity(node, source, family, &mut score);
    score
}

fn add_complexity(node: Node<'_>, source: &str, family: LanguageFamily, score: &mut usize) {
    if is_decision_node(node, source, family) {
        *score += 1;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        add_complexity(child, source, family, score);
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

fn is_decision_node(node: Node<'_>, source: &str, family: LanguageFamily) -> bool {
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

    node.utf8_text(source.as_bytes()).ok().is_some_and(|text| {
        text.contains("&&")
            || text.contains("||")
            || (family == LanguageFamily::Python
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

fn collect_type_metrics(root: Node<'_>, traversal: StructureTraversal<'_>) -> Vec<TypeMetric> {
    let mut types = Vec::new();
    collect_type_metrics_from(root, traversal, &mut types);
    types
}

fn collect_type_metrics_from(
    node: Node<'_>,
    traversal: StructureTraversal<'_>,
    types: &mut Vec<TypeMetric>,
) {
    if should_skip_rust_test_module(node, traversal) {
        return;
    }

    if let Some(metric) = type_metric(node, traversal.source, traversal.family) {
        types.push(metric);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_type_metrics_from(child, traversal, types);
    }
}

fn type_metric(node: Node<'_>, source: &str, family: LanguageFamily) -> Option<TypeMetric> {
    let kind = node.kind();
    let name_node = match family {
        LanguageFamily::Rust
            if matches!(
                kind,
                "struct_item" | "enum_item" | "trait_item" | "impl_item"
            ) =>
        {
            node.child_by_field_name("name")
        }
        LanguageFamily::JavaScriptTypeScript
            if matches!(
                kind,
                "class_declaration" | "interface_declaration" | "type_alias_declaration"
            ) =>
        {
            node.child_by_field_name("name")
        }
        LanguageFamily::Python if kind == "class_definition" => node.child_by_field_name("name"),
        LanguageFamily::Go if kind == "type_spec" => node.child_by_field_name("name"),
        _ => None,
    }?;

    let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();
    Some(TypeMetric {
        name,
        line: node.start_position().row + 1,
        lines: node_line_span(node),
        members: count_type_members(node, family),
    })
}

fn count_type_members(node: Node<'_>, family: LanguageFamily) -> usize {
    let mut count = 0;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if is_type_member(child, family) {
            count += 1;
        }
        count += count_type_members(child, family);
    }
    count
}

fn is_type_member(node: Node<'_>, family: LanguageFamily) -> bool {
    let kind = node.kind();
    match family {
        LanguageFamily::Rust => matches!(
            kind,
            "field_declaration"
                | "enum_variant"
                | "function_item"
                | "associated_type"
                | "const_item"
        ),
        LanguageFamily::JavaScriptTypeScript => matches!(
            kind,
            "method_definition"
                | "public_field_definition"
                | "field_definition"
                | "property_signature"
                | "method_signature"
        ),
        LanguageFamily::Python => matches!(kind, "function_definition" | "assignment"),
        LanguageFamily::Go => matches!(kind, "field_declaration" | "method_elem"),
    }
}

fn count_imports(root: Node<'_>, family: LanguageFamily) -> usize {
    let mut count = 0;
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match family {
            LanguageFamily::Rust if child.kind() == "use_declaration" => count += 1,
            LanguageFamily::JavaScriptTypeScript if child.kind() == "import_statement" => {
                count += 1
            }
            LanguageFamily::Python
                if matches!(child.kind(), "import_statement" | "import_from_statement") =>
            {
                count += 1
            }
            LanguageFamily::Go if child.kind() == "import_declaration" => {
                count += count_named_descendants(child, "import_spec").max(1);
            }
            _ => {}
        }
    }
    count
}

fn count_public_items(root: Node<'_>, traversal: StructureTraversal<'_>) -> usize {
    let mut count = 0;
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if should_skip_rust_test_module(child, traversal) {
            continue;
        }

        count += match traversal.family {
            LanguageFamily::Rust if rust_public_item(child) => 1,
            LanguageFamily::JavaScriptTypeScript if child.kind() == "export_statement" => 1,
            LanguageFamily::Python if python_public_item(child, traversal.source) => 1,
            LanguageFamily::Go if go_public_item(child, traversal.source) => 1,
            _ => 0,
        };
    }
    count
}

fn rust_public_item(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "function_item"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "type_item"
            | "const_item"
            | "static_item"
            | "mod_item"
    ) && node.child_by_field_name("visibility").is_some()
}

fn should_skip_rust_test_module(node: Node<'_>, traversal: StructureTraversal<'_>) -> bool {
    traversal.family == LanguageFamily::Rust
        && !traversal.include_test_structure
        && node.kind() == "mod_item"
        && has_cfg_test_attribute(node, traversal.source)
}

fn has_cfg_test_attribute(node: Node<'_>, source: &str) -> bool {
    let mut end = node.start_byte().min(source.len());
    let bytes = source.as_bytes();

    loop {
        while end > 0 && bytes[end - 1].is_ascii_whitespace() {
            end -= 1;
        }

        if end == 0 || bytes[end - 1] != b']' {
            return false;
        }

        let Some(start) = source[..end].rfind("#[") else {
            return false;
        };
        let attribute = &source[start..end];
        if is_cfg_test_attribute(attribute) {
            return true;
        }

        end = start;
    }
}

fn is_cfg_test_attribute(attribute: &str) -> bool {
    let compact = attribute
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let Some(inner) = compact
        .strip_prefix("#[cfg(")
        .and_then(|value| value.strip_suffix(")]"))
    else {
        return false;
    };

    inner == "test"
        || inner.starts_with("any(test")
        || inner.starts_with("all(test")
        || inner.contains("(test,")
        || inner.contains(",test,")
        || inner.ends_with(",test")
        || inner.ends_with(",test)")
}

fn python_public_item(node: Node<'_>, source: &str) -> bool {
    if !matches!(node.kind(), "function_definition" | "class_definition") {
        return false;
    }

    node.child_by_field_name("name")
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .is_some_and(|name| !name.starts_with('_'))
}

fn go_public_item(node: Node<'_>, source: &str) -> bool {
    if !matches!(
        node.kind(),
        "function_declaration" | "method_declaration" | "type_declaration"
    ) {
        return false;
    }

    node.child_by_field_name("name")
        .or_else(|| {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|child| child.kind() == "type_spec")
                .and_then(|spec| spec.child_by_field_name("name"))
        })
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .is_some_and(is_exported_go_identifier)
}

fn is_exported_go_identifier(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|character| character.is_uppercase())
}

fn collect_repeated_literals(
    file: &SourceFile,
    node: Node<'_>,
    traversal: StructureTraversal<'_>,
    signals: &mut FileSignals,
) {
    if should_skip_rust_test_module(node, traversal) {
        return;
    }

    if is_literal_node(node)
        && !has_literal_ancestor(node)
        && let Ok(text) = node.utf8_text(traversal.source.as_bytes())
        && let Some(literal) = normalize_literal(text)
    {
        signals.literals.push((
            literal,
            Occurrence {
                path: file.display_path.clone(),
                line: node.start_position().row + 1,
                name: None,
            },
        ));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_repeated_literals(file, child, traversal, signals);
    }
}

fn is_literal_node(node: Node<'_>) -> bool {
    let kind = node.kind();
    kind.contains("string")
        || kind.contains("number")
        || kind.contains("integer")
        || kind.contains("float")
        || matches!(kind, "raw_string_literal" | "interpreted_string_literal")
}

fn has_literal_ancestor(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if is_literal_node(parent) {
            return true;
        }
        node = parent;
    }

    false
}

fn normalize_literal(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.chars().all(|character| character.is_ascii_digit()) {
        return if trimmed.len() < 3 {
            None
        } else {
            Some(trimmed.to_string())
        };
    }

    let unquoted = trimmed
        .trim_start_matches(['r', 'b', 'f', 'u'])
        .trim_matches(['"', '\'', '`']);
    if unquoted.len() < 5 {
        None
    } else {
        Some(unquoted.to_string())
    }
}

fn collect_error_patterns(
    file: &SourceFile,
    node: Node<'_>,
    traversal: StructureTraversal<'_>,
    signals: &mut FileSignals,
) {
    if should_skip_rust_test_module(node, traversal) {
        return;
    }

    if is_error_pattern_node(node, traversal.source, traversal.family)
        && let Ok(text) = node.utf8_text(traversal.source.as_bytes())
    {
        signals.error_patterns.push((
            normalize_pattern(text),
            Occurrence {
                path: file.display_path.clone(),
                line: node.start_position().row + 1,
                name: None,
            },
        ));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_error_patterns(file, child, traversal, signals);
    }
}

fn is_error_pattern_node(node: Node<'_>, source: &str, family: LanguageFamily) -> bool {
    let kind = node.kind();
    match family {
        LanguageFamily::JavaScriptTypeScript => kind == "catch_clause",
        LanguageFamily::Python => kind == "except_clause",
        LanguageFamily::Go if kind == "if_statement" => {
            node.utf8_text(source.as_bytes()).ok().is_some_and(|text| {
                text.contains("err") && text.contains("!=") && text.contains("nil")
            })
        }
        LanguageFamily::Rust => {
            kind == "match_arm"
                && node
                    .utf8_text(source.as_bytes())
                    .ok()
                    .is_some_and(|text| text.contains("Err"))
        }
        _ => false,
    }
}

fn collect_test_setup_patterns(file: &SourceFile, node: Node<'_>, signals: &mut FileSignals) {
    if matches!(node.kind(), "call_expression" | "call")
        && let Ok(text) = node.utf8_text(file.source.as_bytes())
    {
        let normalized = normalize_pattern(text);
        if is_setup_pattern(&normalized) {
            signals.test_setups.push((
                normalized,
                Occurrence {
                    path: file.display_path.clone(),
                    line: node.start_position().row + 1,
                    name: None,
                },
            ));
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_test_setup_patterns(file, child, signals);
    }
}

fn is_setup_pattern(pattern: &str) -> bool {
    pattern.contains("setup")
        || pattern.contains("fixture")
        || pattern.contains("mock")
        || pattern.contains("fake")
        || pattern.contains("before_each")
        || pattern.contains("beforeeach")
        || pattern.contains("beforeall")
}

fn collect_directory_concepts(
    file: &SourceFile,
    family: LanguageFamily,
    signals: &mut FileSignals,
) {
    let Some(parent) = file.path.parent() else {
        return;
    };

    let Some(stem) = file.path.file_stem().and_then(|stem| stem.to_str()) else {
        return;
    };

    let mut concepts = split_identifier_words(stem);
    concepts.push(format!("{family:?}").to_ascii_lowercase());
    let entry = signals
        .directory_files
        .entry(parent.to_path_buf())
        .or_default();
    for concept in concepts {
        entry.insert(concept);
    }
}

fn directory_drift_findings(
    directories: &BTreeMap<PathBuf, BTreeSet<String>>,
    options: &StructureOptions,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (directory, concepts) in directories {
        let threshold = options.max_dir_files.max(4);
        if concepts.len() > threshold {
            findings.push(Finding {
                kind: FindingKind::DirectoryDrift,
                severity: Severity::Info,
                path: directory.to_string_lossy().replace('\\', "/"),
                line: None,
                magnitude: Some(concepts.len()),
                message: format!(
                    "directory mixes {} naming/language concepts; consider grouping cohesive responsibilities",
                    concepts.len()
                ),
                related_locations: Vec::new(),
            });
        }
    }
    findings
}

fn group_occurrences(
    occurrences: Vec<(String, Occurrence)>,
    min_occurrences: usize,
    kind: FindingKind,
    severity: Severity,
    message: impl Fn(&str, usize) -> String,
) -> Vec<Finding> {
    if min_occurrences == 0 {
        return Vec::new();
    }

    let mut by_key: BTreeMap<String, Vec<Occurrence>> = BTreeMap::new();
    for (key, occurrence) in occurrences {
        by_key.entry(key).or_default().push(occurrence);
    }

    let mut findings = Vec::new();
    for (key, mut group) in by_key {
        group.sort_by(|left, right| left.path.cmp(&right.path).then(left.line.cmp(&right.line)));
        if group.len() < min_occurrences {
            continue;
        }

        let representative = &group[0];
        findings.push(Finding {
            kind: kind.clone(),
            severity: severity.clone(),
            path: representative.path.clone(),
            line: Some(representative.line),
            magnitude: Some(group.len()),
            message: message(&key, group.len()),
            related_locations: group
                .iter()
                .map(|occurrence| RelatedLocation {
                    path: occurrence.path.clone(),
                    line: occurrence.line,
                    name: occurrence.name.clone(),
                })
                .collect(),
        });
    }

    findings
}

fn adapter_for_path(path: &Path) -> Option<LanguageAdapter> {
    let extension = path.extension()?.to_str()?;

    match extension {
        "rs" => Some(LanguageAdapter {
            family: LanguageFamily::Rust,
            language: || tree_sitter_rust::LANGUAGE.into(),
        }),
        "js" | "jsx" => Some(LanguageAdapter {
            family: LanguageFamily::JavaScriptTypeScript,
            language: || tree_sitter_javascript::LANGUAGE.into(),
        }),
        "ts" => Some(LanguageAdapter {
            family: LanguageFamily::JavaScriptTypeScript,
            language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }),
        "tsx" => Some(LanguageAdapter {
            family: LanguageFamily::JavaScriptTypeScript,
            language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
        }),
        "py" => Some(LanguageAdapter {
            family: LanguageFamily::Python,
            language: || tree_sitter_python::LANGUAGE.into(),
        }),
        "go" => Some(LanguageAdapter {
            family: LanguageFamily::Go,
            language: || tree_sitter_go::LANGUAGE.into(),
        }),
        _ => None,
    }
}

fn count_named_descendants(node: Node<'_>, kind: &str) -> usize {
    let mut count = usize::from(node.kind() == kind);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        count += count_named_descendants(child, kind);
    }
    count
}

fn node_line_span(node: Node<'_>) -> usize {
    node.end_position()
        .row
        .saturating_sub(node.start_position().row)
        + 1
}

fn normalize_identifier(text: &str) -> String {
    text.trim_matches(|character: char| !character.is_alphanumeric() && character != '_')
        .to_ascii_lowercase()
}

fn normalize_pattern(text: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_space = false;
    for character in text.chars() {
        let replacement = if character.is_ascii_digit() {
            Some('#')
        } else if character == '"' || character == '\'' || character == '`' {
            Some('"')
        } else if character.is_whitespace() {
            if previous_was_space {
                None
            } else {
                previous_was_space = true;
                Some(' ')
            }
        } else {
            previous_was_space = false;
            Some(character.to_ascii_lowercase())
        };

        if let Some(character) = replacement {
            normalized.push(character);
        }
    }
    normalized.trim().to_string()
}

fn split_identifier_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if character == '_' || character == '-' || character == '.' {
            if !current.is_empty() {
                words.push(current.to_ascii_lowercase());
                current.clear();
            }
        } else if character.is_uppercase() && !current.is_empty() {
            words.push(current.to_ascii_lowercase());
            current.clear();
            current.push(character);
        } else if character.is_alphanumeric() {
            current.push(character);
        }
    }

    if !current.is_empty() {
        words.push(current.to_ascii_lowercase());
    }

    words
        .into_iter()
        .filter(|word| word.len() > 2 && !matches!(word.as_str(), "mod" | "lib" | "main" | "test"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_file(path: &str, source: &str) -> SourceFile {
        SourceFile {
            path: PathBuf::from(path),
            display_path: path.to_string(),
            source: source.to_string(),
        }
    }

    fn options() -> StructureOptions {
        StructureOptions {
            max_function_lines: 6,
            max_function_complexity: 3,
            max_nesting_depth: 2,
            max_function_parameters: 3,
            max_type_lines: 6,
            max_type_members: 3,
            max_imports: 2,
            max_public_items: 2,
            min_repeated_literal_occurrences: 3,
            min_data_clump_occurrences: 3,
            max_dir_files: 3,
            include_test_structure: false,
        }
    }

    #[test]
    fn reports_rust_function_level_signals() -> Result<()> {
        let source = r#"
pub fn process(a: i32, b: i32, c: i32, d: i32) -> i32 {
    if a > 0 {
        for value in [b, c] {
            if value > 1 {
                return value;
            }
        }
    }
    d
}
"#;

        let findings = scan_structure(&[source_file("src/lib.rs", source)], &options())?;

        assert!(
            findings
                .iter()
                .any(|finding| finding.kind == FindingKind::LongFunction)
        );
        assert!(
            findings
                .iter()
                .any(|finding| finding.kind == FindingKind::ComplexFunction)
        );
        assert!(
            findings
                .iter()
                .any(|finding| finding.kind == FindingKind::DeepNesting)
        );
        assert!(
            findings
                .iter()
                .any(|finding| finding.kind == FindingKind::ManyParameters)
        );
        Ok(())
    }

    #[test]
    fn counts_rust_parameter_patterns_without_type_identifiers() -> Result<()> {
        let source = r#"
fn collect_named_functions(
    node: Node<'_>,
    extraction: CandidateExtraction<'_>,
    interner: &mut TokenInterner,
    candidates: &mut Vec<FunctionCandidate>,
) {
}
"#;
        let mut opts = options();
        opts.max_function_parameters = 4;

        let findings = scan_structure(&[source_file("src/lib.rs", source)], &opts)?;

        assert!(
            !findings
                .iter()
                .any(|finding| finding.kind == FindingKind::ManyParameters),
            "{findings:#?}"
        );
        Ok(())
    }

    #[test]
    fn reports_typescript_module_level_signals() -> Result<()> {
        let source = r#"
import a from "a";
import b from "b";
import c from "c";
export function one() {}
export function two() {}
export function three() {}
export class BigThing {
  one() {}
  two() {}
  three() {}
  four() {}
}
"#;

        let findings = scan_structure(&[source_file("src/app.ts", source)], &options())?;

        assert!(
            findings
                .iter()
                .any(|finding| finding.kind == FindingKind::ImportHeavyFile)
        );
        assert!(
            findings
                .iter()
                .any(|finding| finding.kind == FindingKind::LargePublicSurface)
        );
        assert!(
            findings
                .iter()
                .any(|finding| finding.kind == FindingKind::LargeType)
        );
        Ok(())
    }

    #[test]
    fn reports_python_repeated_literals_and_data_clumps() -> Result<()> {
        let source = r#"
def one(customer_id, account_id, region_id):
    return "shared literal"

def two(customer_id, account_id, region_id):
    return "shared literal"

def three(customer_id, account_id, region_id):
    return "shared literal"
"#;

        let findings = scan_structure(&[source_file("src/app.py", source)], &options())?;

        assert!(
            findings
                .iter()
                .any(|finding| finding.kind == FindingKind::RepeatedLiteral)
        );
        assert!(
            findings
                .iter()
                .any(|finding| finding.kind == FindingKind::DataClump)
        );
        Ok(())
    }

    #[test]
    fn reports_go_repeated_error_patterns() -> Result<()> {
        let source = r#"
package app

func One() error {
    value, err := load()
    if err != nil {
        return err
    }
    return value.Close()
}

func Two() error {
    value, err := load()
    if err != nil {
        return err
    }
    return value.Close()
}

func Three() error {
    value, err := load()
    if err != nil {
        return err
    }
    return value.Close()
}
"#;

        let findings = scan_structure(&[source_file("src/app.go", source)], &options())?;

        assert!(
            findings
                .iter()
                .any(|finding| finding.kind == FindingKind::RepeatedErrorPattern)
        );
        Ok(())
    }

    #[test]
    fn skips_test_files_for_structure_by_default_but_reports_test_duplication() -> Result<()> {
        let source = r#"
test("one", () => {
  setupUserFixture();
  const label = "shared literal";
  expect(1).toBe(1);
});
test("two", () => {
  setupUserFixture();
  const label = "shared literal";
  expect(2).toBe(2);
});
test("three", () => {
  setupUserFixture();
  const label = "shared literal";
  expect(3).toBe(3);
});
"#;

        let mut opts = options();
        opts.max_imports = 0;
        let findings = scan_structure(&[source_file("tests/app.test.js", source)], &opts)?;

        assert!(
            findings
                .iter()
                .any(|finding| finding.kind == FindingKind::TestDuplication)
        );
        assert!(
            !findings
                .iter()
                .any(|finding| finding.kind == FindingKind::ImportHeavyFile)
        );

        opts.include_test_structure = true;
        let included = scan_structure(&[source_file("tests/app.test.js", source)], &opts)?;
        assert!(
            included
                .iter()
                .any(|finding| finding.kind == FindingKind::RepeatedLiteral)
        );
        Ok(())
    }

    #[test]
    fn skips_rust_cfg_test_modules_for_structure_by_default() -> Result<()> {
        let source = r#"
pub fn production() -> &'static str {
    "production"
}

#[cfg(test)]
mod tests {
    fn one(customer_id: i32, account_id: i32, region_id: i32) -> &'static str {
        "shared test literal"
    }

    fn two(customer_id: i32, account_id: i32, region_id: i32) -> &'static str {
        "shared test literal"
    }

    fn three(customer_id: i32, account_id: i32, region_id: i32) -> &'static str {
        "shared test literal"
    }
}
"#;

        let findings = scan_structure(&[source_file("src/lib.rs", source)], &options())?;

        assert!(
            !findings
                .iter()
                .any(|finding| finding.kind == FindingKind::RepeatedLiteral)
        );
        assert!(
            !findings
                .iter()
                .any(|finding| finding.kind == FindingKind::DataClump)
        );

        let mut opts = options();
        opts.include_test_structure = true;
        let included = scan_structure(&[source_file("src/lib.rs", source)], &opts)?;

        assert!(
            included
                .iter()
                .any(|finding| finding.kind == FindingKind::RepeatedLiteral)
        );
        assert!(
            included
                .iter()
                .any(|finding| finding.kind == FindingKind::DataClump)
        );
        Ok(())
    }

    #[test]
    fn reports_directory_drift() -> Result<()> {
        let files = [
            source_file("src/payments/user_invoice.rs", "fn a() {}\n"),
            source_file("src/payments/cache_token.rs", "fn b() {}\n"),
            source_file("src/payments/report_export.rs", "fn c() {}\n"),
            source_file("src/payments/email_template.rs", "fn d() {}\n"),
        ];
        let mut opts = options();
        opts.max_dir_files = 2;

        let findings = scan_structure(&files, &opts)?;

        assert!(
            findings
                .iter()
                .any(|finding| finding.kind == FindingKind::DirectoryDrift)
        );
        Ok(())
    }
}
