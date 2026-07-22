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

struct FunctionAnalyzer<'a> {
    file: &'a ParsedSourceFile,
    graph: &'a mut FlowGraph,
    function_index: usize,
    scopes: Vec<BTreeMap<String, NodeId>>,
    ordinal: usize,
    stable_path: String,
}

impl FunctionAnalyzer<'_> {
    fn process_block(&mut self, block: Node<'_>, function_body: bool) -> Vec<NodeId> {
        if !function_body {
            self.scopes.push(BTreeMap::new());
        }
        let mut tail = Vec::new();
        let mut cursor = block.walk();
        let children = block.named_children(&mut cursor).collect::<Vec<_>>();
        for (index, child) in children.iter().copied().enumerate() {
            let is_tail = index + 1 == children.len()
                && !matches!(child.kind(), "let_declaration" | "expression_statement");
            match child.kind() {
                "let_declaration" => self.process_let(child),
                "expression_statement" => {
                    if let Some(expression) = child.named_child(0) {
                        self.process_statement_expression(expression);
                    }
                }
                "return_expression" => self.process_return(child),
                "function_item" => {}
                _ if is_tail => tail = self.eval_expr(child),
                _ => self.process_statement_expression(child),
            }
        }
        if !function_body {
            self.scopes.pop();
        }
        tail
    }

    fn process_statement_expression(&mut self, expression: Node<'_>) {
        match expression.kind() {
            "return_expression" => self.process_return(expression),
            "assignment_expression" | "compound_assignment_expr" => {
                self.process_assignment(expression)
            }
            "if_expression" | "match_expression" | "loop_expression" | "while_expression"
            | "for_expression" => self.process_control(expression),
            "block" => {
                self.process_block(expression, false);
            }
            _ => {
                self.eval_expr(expression);
            }
        }
    }

    fn process_control(&mut self, expression: Node<'_>) {
        let before = self.scopes.clone();
        let mut cursor = expression.walk();
        for child in expression.named_children(&mut cursor) {
            match child.kind() {
                "block" => {
                    self.scopes = before.clone();
                    self.process_block(child, false);
                }
                "else_clause" | "match_block" | "match_arm" => {
                    self.scopes = before.clone();
                    self.walk_control_child(child);
                }
                _ if child.kind().ends_with("expression") => {
                    self.eval_expr(child);
                }
                _ => {}
            }
        }
        self.scopes = before;
    }

    fn walk_control_child(&mut self, node: Node<'_>) {
        if node.kind() == "block" {
            self.process_block(node, false);
            return;
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "block" {
                self.process_block(child, false);
            } else {
                self.process_statement_expression(child);
            }
        }
    }

    fn process_let(&mut self, declaration: Node<'_>) {
        let Some(pattern) = declaration.child_by_field_name("pattern") else {
            return;
        };
        let value = declaration.child_by_field_name("value");
        if pattern.kind() == "tuple_pattern"
            && value.is_some_and(|value| value.kind() == "tuple_expression")
        {
            let value = value.unwrap();
            let mut pattern_cursor = pattern.walk();
            let patterns = pattern
                .named_children(&mut pattern_cursor)
                .collect::<Vec<_>>();
            let mut value_cursor = value.walk();
            let values = value.named_children(&mut value_cursor).collect::<Vec<_>>();
            if patterns.len() == values.len() {
                for (pattern, value) in patterns.into_iter().zip(values) {
                    let sources = self.eval_expr(value);
                    self.bind_pattern(pattern, &sources);
                }
                return;
            }
        }
        let sources = value.map(|value| self.eval_expr(value)).unwrap_or_default();
        self.bind_pattern(pattern, &sources);
    }

    fn bind_pattern(&mut self, pattern: Node<'_>, sources: &[NodeId]) {
        for (binding, line) in pattern_bindings(pattern, self.file) {
            let node = self.new_local(&binding, line);
            self.connect_sources(sources, node, line, format!("assign to {binding}"));
            self.scopes.last_mut().unwrap().insert(binding, node);
        }
    }

    fn process_assignment(&mut self, assignment: Node<'_>) {
        let Some(left) = assignment.child_by_field_name("left") else {
            return;
        };
        let Some(right) = assignment.child_by_field_name("right") else {
            return;
        };
        let sources = self.eval_expr(right);
        if left.kind() != "identifier" {
            self.graph.unresolved("unsupported Rust assignment target");
            return;
        }
        let Some(name) = text(left, self.file) else {
            return;
        };
        if self.resolve_binding(&name).is_none() {
            self.graph
                .unresolved("assignment to unresolved Rust binding");
            return;
        }
        let line = left.start_position().row + 1;
        let node = self.new_local(&name, line);
        self.connect_sources(&sources, node, line, format!("reassign {name}"));
        self.replace_binding(&name, node);
    }

    fn process_return(&mut self, expression: Node<'_>) {
        let sources = expression
            .named_child(0)
            .map(|value| self.eval_expr(value))
            .unwrap_or_default();
        self.connect_return(sources, expression.start_position().row + 1);
    }

    fn connect_return(&mut self, sources: Vec<NodeId>, line: usize) {
        let target = self.graph.functions[self.function_index].return_node;
        self.connect_sources(&sources, target, line, "return value".into());
    }

    fn eval_expr(&mut self, expression: Node<'_>) -> Vec<NodeId> {
        match expression.kind() {
            "identifier" => text(expression, self.file)
                .and_then(|name| self.resolve_binding(&name))
                .into_iter()
                .collect(),
            "call_expression" => self.eval_call(expression),
            "reference_expression" | "parenthesized_expression" | "try_expression" => expression
                .named_child(0)
                .map(|child| self.eval_expr(child))
                .unwrap_or_default(),
            "tuple_expression" | "array_expression" => {
                let mut sources = Vec::new();
                let mut cursor = expression.walk();
                for child in expression.named_children(&mut cursor) {
                    sources.extend(self.eval_expr(child));
                }
                dedup(sources)
            }
            "return_expression" => {
                self.process_return(expression);
                Vec::new()
            }
            "assignment_expression" | "compound_assignment_expr" => {
                self.process_assignment(expression);
                Vec::new()
            }
            "if_expression" | "match_expression" | "loop_expression" | "while_expression"
            | "for_expression" => {
                if contains_binding_use(expression, self.file, &self.scopes) {
                    self.graph
                        .unresolved("unsupported Rust control-flow value merge");
                }
                self.process_control(expression);
                Vec::new()
            }
            "block" => self.process_block(expression, false),
            "closure_expression" => {
                self.graph.unresolved("unsupported Rust closure flow");
                Vec::new()
            }
            "await_expression" => {
                self.graph.unresolved("unsupported Rust async flow");
                Vec::new()
            }
            "field_expression" => {
                self.graph
                    .unresolved("unsupported Rust field or method flow");
                Vec::new()
            }
            "macro_invocation" => {
                self.graph.unresolved("unsupported Rust macro flow");
                Vec::new()
            }
            kind if is_literal(kind) => Vec::new(),
            _ => {
                if contains_binding_use(expression, self.file, &self.scopes) {
                    self.graph.unresolved(format!(
                        "unsupported Rust {} value transform",
                        expression.kind()
                    ));
                }
                Vec::new()
            }
        }
    }

    fn eval_call(&mut self, call: Node<'_>) -> Vec<NodeId> {
        let Some(function) = call.child_by_field_name("function") else {
            return Vec::new();
        };
        let line = call.start_position().row + 1;
        let call_ordinal = self.ordinal;
        self.ordinal += 1;
        let argument_nodes = self.eval_call_arguments(call, call_ordinal);

        if !matches!(function.kind(), "identifier" | "scoped_identifier") {
            self.graph
                .unresolved("unsupported Rust method or function-value call");
            return Vec::new();
        }
        let Some(raw_target) = text(function, self.file) else {
            return Vec::new();
        };
        let module = self.graph.functions[self.function_index].module.clone();
        let crate_key = self.graph.functions[self.function_index].crate_key.clone();
        let Some((target, target_index)) =
            resolve_function(&raw_target, &crate_key, &module, self.graph)
        else {
            self.graph
                .unresolved(format!("unresolved Rust call {raw_target}"));
            return Vec::new();
        };
        self.record_resolved_call(target, target_index, argument_nodes, line, call_ordinal)
    }

    fn eval_call_arguments(&mut self, call: Node<'_>, call_ordinal: usize) -> Vec<NodeId> {
        let mut argument_nodes = Vec::new();
        if let Some(arguments) = call.child_by_field_name("arguments") {
            let mut cursor = arguments.walk();
            for (index, argument) in arguments.named_children(&mut cursor).enumerate() {
                let sources = self.eval_expr(argument);
                let node = self.add_node(
                    FlowNodeKind::Argument,
                    argument.start_position().row + 1,
                    &format!("argument {index}"),
                    &format!("arg-{call_ordinal}-{index}"),
                );
                self.connect_sources(
                    &sources,
                    node,
                    argument.start_position().row + 1,
                    format!("argument {index}"),
                );
                argument_nodes.push(node);
            }
        }
        argument_nodes
    }

    fn record_resolved_call(
        &mut self,
        target: String,
        target_index: usize,
        argument_nodes: Vec<NodeId>,
        line: usize,
        call_ordinal: usize,
    ) -> Vec<NodeId> {
        let call_site = format!(
            "{}:{}:{}:{}",
            self.stable_path, line, self.function_index, call_ordinal
        );
        let parameter_groups = self.graph.functions[target_index].parameter_groups.clone();
        let parameter_groups_exact = self.graph.functions[target_index]
            .parameter_groups_exact
            .clone();
        if argument_nodes.len() != parameter_groups.len() {
            self.graph
                .unresolved(format!("argument arity mismatch for {target}"));
            return Vec::new();
        }
        for (index, (argument, parameters)) in argument_nodes
            .iter()
            .copied()
            .zip(parameter_groups)
            .enumerate()
        {
            if !parameter_groups_exact[index] {
                self.graph
                    .unresolved(format!("unsupported destructured parameter for {target}"));
                continue;
            }
            for parameter in parameters {
                self.graph.add_edge(FlowEdge {
                    from: argument,
                    to: parameter,
                    kind: FlowEdgeKind::ArgumentToParameter,
                    resolution: FlowResolution::Exact,
                    path: self.file.file.display_path.clone(),
                    line,
                    name: format!("call {target} argument {index}"),
                    call_site: Some(call_site.clone()),
                    transition: CallTransition::Enter,
                });
            }
        }
        let result = self.add_node(
            FlowNodeKind::CallResult,
            line,
            &format!("result of {target}"),
            &format!("result-{call_ordinal}"),
        );
        let return_node = self.graph.functions[target_index].return_node;
        self.graph.add_edge(FlowEdge {
            from: return_node,
            to: result,
            kind: FlowEdgeKind::ReturnToResult,
            resolution: FlowResolution::Exact,
            path: self.file.file.display_path.clone(),
            line,
            name: format!("return from {target}"),
            call_site: Some(call_site),
            transition: CallTransition::Return,
        });
        self.graph.calls.push(CallRecord {
            target,
            function_index: target_index,
            path: self.file.file.display_path.clone(),
            line,
        });
        vec![result]
    }

    fn new_local(&mut self, name: &str, line: usize) -> NodeId {
        let ordinal = self.ordinal;
        self.ordinal += 1;
        self.add_node(FlowNodeKind::Local, line, name, &format!("local-{ordinal}"))
    }

    fn add_node(&mut self, kind: FlowNodeKind, line: usize, name: &str, suffix: &str) -> NodeId {
        let function = &self.graph.functions[self.function_index];
        let id = self.graph.nodes.len();
        self.graph.nodes.push(FlowLocation {
            id: format!("flow:{}:{}:{}", self.stable_path, function.symbol, suffix),
            kind,
            path: self.file.file.display_path.clone(),
            line,
            function: function.symbol.clone(),
            module: function.module.clone(),
            name: name.to_string(),
        });
        id
    }

    fn connect_sources(&mut self, sources: &[NodeId], target: NodeId, line: usize, name: String) {
        for source in sources.iter().copied() {
            self.graph.add_edge(FlowEdge {
                from: source,
                to: target,
                kind: FlowEdgeKind::Assignment,
                resolution: FlowResolution::Exact,
                path: self.file.file.display_path.clone(),
                line,
                name: name.clone(),
                call_site: None,
                transition: CallTransition::None,
            });
        }
    }

    fn resolve_binding(&self, name: &str) -> Option<NodeId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn replace_binding(&mut self, name: &str, node: NodeId) {
        if let Some(scope) = self
            .scopes
            .iter_mut()
            .rev()
            .find(|scope| scope.contains_key(name))
        {
            scope.insert(name.to_string(), node);
        }
    }
}

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
