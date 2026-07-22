mod resolution;

use std::collections::BTreeMap;
use std::path::Path;

use tree_sitter::Node;

use crate::detectors::similarity::ParsedSourceFile;
use crate::lang::LanguageFamily;
use crate::model::{FlowEdgeKind, FlowLocation, FlowNodeKind, FlowResolution};

use super::model::{CallRecord, CallTransition, FlowEdge, FlowGraph, FunctionRecord, NodeId};
use resolution::{canonical_path, file_module, resolution_key, resolve_function, stable_path};

struct RustFileContext<'a> {
    root: &'a Path,
    file: &'a ParsedSourceFile,
    crate_key: &'a str,
}

pub(super) fn build_graph(root: &Path, parsed_sources: &[ParsedSourceFile]) -> FlowGraph {
    let mut files = parsed_sources
        .iter()
        .filter(|file| file.family == LanguageFamily::Rust)
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.file.display_path.cmp(&right.file.display_path));

    let mut graph = FlowGraph::default();
    for file in &files {
        let module = file_module(root, &file.file.path);
        let context = RustFileContext {
            root,
            file,
            crate_key: &module.crate_key,
        };
        index_items(&context, file.tree.root_node(), &module.symbol, &mut graph);
    }
    for file in &files {
        analyze_file(root, file, &mut graph);
    }
    graph.finish();
    graph
}

fn index_items(
    context: &RustFileContext<'_>,
    container: Node<'_>,
    module: &str,
    graph: &mut FlowGraph,
) {
    let mut cursor = container.walk();
    for node in container.named_children(&mut cursor) {
        match node.kind() {
            "function_item" => index_function(context, node, module, graph),
            "mod_item" => {
                let Some(name) = node
                    .child_by_field_name("name")
                    .and_then(|name| text(name, context.file))
                else {
                    continue;
                };
                if let Some(body) = node.child_by_field_name("body") {
                    index_items(context, body, &format!("{module}::{name}"), graph);
                }
            }
            "use_declaration" => index_use(context, node, module, graph),
            _ => {}
        }
    }
}

fn index_function(
    context: &RustFileContext<'_>,
    node: Node<'_>,
    module: &str,
    graph: &mut FlowGraph,
) {
    let Some(name) = node
        .child_by_field_name("name")
        .and_then(|name| text(name, context.file))
    else {
        return;
    };
    let symbol = format!("{module}::{name}");
    let function_index = graph.functions.len();
    let line = node.start_position().row + 1;
    let stable_path = stable_path(context.root, &context.file.file.path);
    let parameters = index_parameters(context, node, module, &symbol, graph);
    let return_node = add_location(
        graph,
        FlowLocation {
            id: format!("flow:{stable_path}:{symbol}:return"),
            kind: FlowNodeKind::Return,
            path: context.file.file.display_path.clone(),
            line,
            function: symbol.clone(),
            module: module.to_string(),
            name: "return".into(),
        },
    );
    graph.functions.push(FunctionRecord {
        symbol: symbol.clone(),
        crate_key: context.crate_key.to_string(),
        module: module.to_string(),
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        parameter_nodes: parameters.nodes,
        parameter_groups: parameters.groups,
        parameter_groups_exact: parameters.groups_exact,
        return_node,
    });
    graph
        .functions_by_symbol
        .entry(resolution_key(context.crate_key, &symbol))
        .or_default()
        .push(function_index);
}

struct IndexedParameters {
    nodes: Vec<NodeId>,
    groups: Vec<Vec<NodeId>>,
    groups_exact: Vec<bool>,
}

fn index_parameters(
    context: &RustFileContext<'_>,
    node: Node<'_>,
    module: &str,
    symbol: &str,
    graph: &mut FlowGraph,
) -> IndexedParameters {
    let stable_path = stable_path(context.root, &context.file.file.path);
    let mut parameter_nodes = Vec::new();
    let mut parameter_groups = Vec::new();
    let mut parameter_groups_exact = Vec::new();
    if let Some(parameters) = node.child_by_field_name("parameters") {
        let mut cursor = parameters.walk();
        for parameter in parameters.named_children(&mut cursor) {
            if parameter.kind() != "parameter" {
                continue;
            }
            let Some(pattern) = parameter.child_by_field_name("pattern") else {
                continue;
            };
            let mut group = Vec::new();
            for (binding, binding_line) in pattern_bindings(pattern, context.file) {
                let ordinal = parameter_nodes.len();
                let parameter_node = add_location(
                    graph,
                    FlowLocation {
                        id: format!("flow:{stable_path}:{symbol}:param-{ordinal}"),
                        kind: FlowNodeKind::Parameter,
                        path: context.file.file.display_path.clone(),
                        line: binding_line,
                        function: symbol.to_string(),
                        module: module.to_string(),
                        name: binding,
                    },
                );
                parameter_nodes.push(parameter_node);
                group.push(parameter_node);
            }
            parameter_groups.push(group);
            parameter_groups_exact.push(is_exact_parameter_pattern(pattern));
        }
    }
    IndexedParameters {
        nodes: parameter_nodes,
        groups: parameter_groups,
        groups_exact: parameter_groups_exact,
    }
}

fn index_use(context: &RustFileContext<'_>, node: Node<'_>, module: &str, graph: &mut FlowGraph) {
    let Some(mut declaration) = text(node, context.file) else {
        return;
    };
    declaration = declaration.trim().trim_end_matches(';').trim().to_string();
    declaration = declaration
        .strip_prefix("pub ")
        .unwrap_or(&declaration)
        .trim()
        .to_string();
    let Some(path) = declaration.strip_prefix("use ").map(str::trim) else {
        return;
    };
    if path.contains('{') || path.contains('*') {
        graph.unresolved("unsupported grouped or glob Rust import");
        return;
    }
    let (target, alias) = if let Some((target, alias)) = path.rsplit_once(" as ") {
        (target.trim(), alias.trim())
    } else {
        (path, path.rsplit("::").next().unwrap_or(path))
    };
    let target = canonical_path(target, module);
    graph
        .imports
        .entry(resolution_key(context.crate_key, module))
        .or_default()
        .insert(alias.to_string(), target);
}

fn analyze_file(root: &Path, file: &ParsedSourceFile, graph: &mut FlowGraph) {
    let module = file_module(root, &file.file.path);
    let context = RustFileContext {
        root,
        file,
        crate_key: &module.crate_key,
    };
    analyze_items(&context, file.tree.root_node(), &module.symbol, graph);
}

fn analyze_items(
    context: &RustFileContext<'_>,
    container: Node<'_>,
    module: &str,
    graph: &mut FlowGraph,
) {
    let mut cursor = container.walk();
    for node in container.named_children(&mut cursor) {
        match node.kind() {
            "function_item" => analyze_function(context, node, module, graph),
            "mod_item" => {
                let Some(name) = node
                    .child_by_field_name("name")
                    .and_then(|name| text(name, context.file))
                else {
                    continue;
                };
                if let Some(body) = node.child_by_field_name("body") {
                    analyze_items(context, body, &format!("{module}::{name}"), graph);
                }
            }
            _ => {}
        }
    }
}

fn analyze_function(
    context: &RustFileContext<'_>,
    node: Node<'_>,
    module: &str,
    graph: &mut FlowGraph,
) {
    let Some(name) = node
        .child_by_field_name("name")
        .and_then(|name| text(name, context.file))
    else {
        return;
    };
    let symbol = format!("{module}::{name}");
    let Some(function_index) = graph
        .functions_by_symbol
        .get(&resolution_key(context.crate_key, &symbol))
        .and_then(|indices| {
            indices.iter().copied().find(|index| {
                graph.functions[*index].start_byte == node.start_byte()
                    && graph.functions[*index].end_byte == node.end_byte()
            })
        })
    else {
        return;
    };
    let mut analyzer = FunctionAnalyzer {
        file: context.file,
        graph,
        function_index,
        scopes: vec![BTreeMap::new()],
        ordinal: 0,
        stable_path: stable_path(context.root, &context.file.file.path),
    };
    let parameters = analyzer.graph.functions[function_index]
        .parameter_nodes
        .clone();
    for parameter in parameters {
        let name = analyzer.graph.nodes[parameter].name.clone();
        analyzer.scopes[0].insert(name, parameter);
    }
    if let Some(body) = node.child_by_field_name("body") {
        let sources = analyzer.process_block(body, true);
        analyzer.connect_return(sources, body.end_position().row + 1);
    }
}

include!("analysis.rs");

fn add_location(graph: &mut FlowGraph, location: FlowLocation) -> NodeId {
    let id = graph.nodes.len();
    graph.nodes.push(location);
    id
}

fn pattern_bindings(pattern: Node<'_>, file: &ParsedSourceFile) -> Vec<(String, usize)> {
    let mut bindings = Vec::new();
    collect_pattern_bindings(pattern, file, &mut bindings);
    bindings
}

fn is_exact_parameter_pattern(pattern: Node<'_>) -> bool {
    match pattern.kind() {
        "identifier" => true,
        "mutable_pattern" | "reference_pattern" => pattern
            .named_child(0)
            .is_some_and(is_exact_parameter_pattern),
        _ => false,
    }
}

fn collect_pattern_bindings(
    node: Node<'_>,
    file: &ParsedSourceFile,
    bindings: &mut Vec<(String, usize)>,
) {
    if node.kind() == "identifier" {
        if let Some(name) = text(node, file) {
            bindings.push((name, node.start_position().row + 1));
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if !matches!(child.kind(), "type_identifier" | "scoped_type_identifier") {
            collect_pattern_bindings(child, file, bindings);
        }
    }
}

fn contains_binding_use(
    node: Node<'_>,
    file: &ParsedSourceFile,
    scopes: &[BTreeMap<String, NodeId>],
) -> bool {
    if node.kind() == "identifier" {
        return text(node, file)
            .is_some_and(|name| scopes.iter().rev().any(|scope| scope.contains_key(&name)));
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| contains_binding_use(child, file, scopes))
}

fn is_literal(kind: &str) -> bool {
    kind.ends_with("literal")
        || matches!(
            kind,
            "integer_literal"
                | "float_literal"
                | "char_literal"
                | "string_literal"
                | "raw_string_literal"
                | "boolean_literal"
                | "unit_expression"
        )
}

fn dedup(mut nodes: Vec<NodeId>) -> Vec<NodeId> {
    nodes.sort_unstable();
    nodes.dedup();
    nodes
}

fn text(node: Node<'_>, file: &ParsedSourceFile) -> Option<String> {
    node.utf8_text(file.file.source.as_bytes())
        .ok()
        .map(ToString::to_string)
}
