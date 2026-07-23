use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tree_sitter::Node;

use crate::detectors::similarity::ParsedSourceFile;
use crate::lang::{JAVASCRIPT_LANGUAGE, LanguageFamily, TYPESCRIPT_LANGUAGE};
use crate::model::{FlowEdgeKind, FlowLocation, FlowNodeKind, FlowResolution};

use super::model::{CallRecord, CallTransition, FlowEdge, FlowGraph, FunctionRecord, NodeId};

mod imports;
use imports::{module_matches, static_imports};
mod index;
use index::FunctionIndexer;

#[derive(Debug, Clone)]
struct IndexedFunction {
    file_index: usize,
    function_index: usize,
    node_start: usize,
    body_start: usize,
}

#[derive(Debug, Clone)]
struct ImportTarget {
    exported_name: String,
    module_hint: String,
}

pub(super) fn extend_graph(
    root: &Path,
    parsed_sources: &[ParsedSourceFile],
    graph: &mut FlowGraph,
) {
    let files = parsed_sources
        .iter()
        .enumerate()
        .filter(|(_, file)| {
            matches!(
                file.family,
                LanguageFamily::JavaScriptTypeScript | LanguageFamily::Python
            )
        })
        .collect::<Vec<_>>();
    let mut indexed = Vec::new();
    for (file_index, file) in &files {
        FunctionIndexer::new(root, *file_index, file, graph, &mut indexed)
            .index(file.tree.root_node());
    }
    for function in indexed {
        let file = &parsed_sources[function.file_index];
        let Some(node) = find_node(file.tree.root_node(), function.node_start) else {
            continue;
        };
        analyze_function(
            FunctionAnalysis {
                root,
                file,
                function_node: node,
                body_start: function.body_start,
                function_index: function.function_index,
                imports: static_imports(root, file),
            },
            graph,
        );
    }
}

struct FunctionAnalysis<'source, 'tree> {
    root: &'source Path,
    file: &'source ParsedSourceFile,
    function_node: Node<'tree>,
    body_start: usize,
    function_index: usize,
    imports: BTreeMap<String, ImportTarget>,
}

fn analyze_function(input: FunctionAnalysis<'_, '_>, graph: &mut FlowGraph) {
    let Some(body) = find_node(input.function_node, input.body_start) else {
        return;
    };
    let mut analyzer = DynamicAnalyzer {
        root: input.root,
        file: input.file,
        graph,
        function_index: input.function_index,
        bindings: BTreeMap::new(),
        imports: input.imports,
        ordinal: 0,
    };
    for parameter in analyzer.graph.functions[input.function_index]
        .parameter_nodes
        .clone()
    {
        let name = analyzer.graph.nodes[parameter].name.clone();
        analyzer.bindings.insert(name, parameter);
    }
    if body.kind() == "statement_block" || body.kind() == "block" {
        analyzer.process_children(body);
    } else {
        let sources = analyzer.eval_expr(body);
        analyzer.connect_return(&sources, body.end_position().row + 1);
    }
}

struct DynamicAnalyzer<'a> {
    root: &'a Path,
    file: &'a ParsedSourceFile,
    graph: &'a mut FlowGraph,
    function_index: usize,
    bindings: BTreeMap<String, NodeId>,
    imports: BTreeMap<String, ImportTarget>,
    ordinal: usize,
}

impl DynamicAnalyzer<'_> {
    fn process_children(&mut self, node: Node<'_>) {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.process_statement(child);
        }
    }

    fn process_statement(&mut self, node: Node<'_>) {
        match node.kind() {
            "function_declaration" | "function_definition" | "class_declaration" => (),
            "lexical_declaration" | "variable_declaration" => {
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    if child.kind() == "variable_declarator" {
                        self.process_binding(
                            child.child_by_field_name("name"),
                            child.child_by_field_name("value"),
                            child.start_position().row + 1,
                        );
                    }
                }
            }
            "assignment" | "assignment_expression" => {
                self.process_binding(
                    node.child_by_field_name("left"),
                    node.child_by_field_name("right"),
                    node.start_position().row + 1,
                );
            }
            "return_statement" => {
                let value = node
                    .child_by_field_name("argument")
                    .or_else(|| node.named_child(0));
                let sources = value.map(|value| self.eval_expr(value)).unwrap_or_default();
                self.connect_return(&sources, node.start_position().row + 1);
            }
            "expression_statement" => {
                if let Some(expression) = node.named_child(0) {
                    if matches!(expression.kind(), "assignment" | "assignment_expression") {
                        self.process_statement(expression);
                    } else {
                        self.eval_expr(expression);
                    }
                }
            }
            _ => self.process_children(node),
        }
    }

    fn process_binding(&mut self, left: Option<Node<'_>>, right: Option<Node<'_>>, line: usize) {
        let Some(left) = left else { return };
        if matches!(left.kind(), "member_expression" | "attribute") {
            let value_sources = right.map(|right| self.eval_expr(right)).unwrap_or_default();
            let fields = self.eval_field(left, FlowEdgeKind::FieldWrite);
            for field in fields {
                self.connect(
                    &value_sources,
                    field,
                    FlowEdgeKind::Mutation,
                    line,
                    "mutate field".into(),
                );
            }
            return;
        }
        let Some(name) = binding_name(left, self.file) else {
            self.graph.unresolved(format!(
                "unsupported {} destructuring assignment",
                language(self.file)
            ));
            return;
        };
        let sources = right.map(|right| self.eval_expr(right)).unwrap_or_default();
        let target = self.add_node(FlowNodeKind::Local, line, &name, "local");
        self.connect(
            &sources,
            target,
            FlowEdgeKind::Assignment,
            line,
            format!("assign {name}"),
        );
        self.bindings.insert(name, target);
    }

    fn eval_expr(&mut self, node: Node<'_>) -> Vec<NodeId> {
        match node.kind() {
            "identifier" => node_text(node, self.file)
                .and_then(|name| self.bindings.get(&name).copied())
                .into_iter()
                .collect(),
            "call_expression" | "call" => self.eval_call(node),
            "parenthesized_expression" => node
                .named_child(0)
                .map(|child| self.eval_expr(child))
                .unwrap_or_default(),
            "member_expression" | "attribute" => self.eval_field(node, FlowEdgeKind::FieldRead),
            kind if is_literal(kind) => {
                let node_id = self.add_node(
                    FlowNodeKind::Literal,
                    node.start_position().row + 1,
                    "literal",
                    "literal",
                );
                vec![node_id]
            }
            _ => {
                let sources = self.eval_named_children(node);
                if sources.is_empty() {
                    return sources;
                }
                let transformation = self.add_node(
                    FlowNodeKind::Local,
                    node.start_position().row + 1,
                    "transformation",
                    "transform",
                );
                self.connect(
                    &sources,
                    transformation,
                    FlowEdgeKind::Transformation,
                    node.start_position().row + 1,
                    format!("{} transformation", language(self.file)),
                );
                vec![transformation]
            }
        }
    }

    fn eval_named_children(&mut self, node: Node<'_>) -> Vec<NodeId> {
        let mut values = Vec::new();
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            values.extend(self.eval_expr(child));
        }
        values.sort_unstable();
        values.dedup();
        values
    }

    fn eval_field(&mut self, node: Node<'_>, edge_kind: FlowEdgeKind) -> Vec<NodeId> {
        let object = node
            .child_by_field_name("object")
            .or_else(|| node.child_by_field_name("value"))
            .map(|object| self.eval_expr(object))
            .unwrap_or_default();
        let name = node
            .child_by_field_name("property")
            .or_else(|| node.child_by_field_name("attribute"))
            .and_then(|property| node_text(property, self.file))
            .unwrap_or_else(|| "field".into());
        let field = self.add_node(
            FlowNodeKind::Field,
            node.start_position().row + 1,
            &name,
            "field",
        );
        self.connect(
            &object,
            field,
            edge_kind,
            node.start_position().row + 1,
            format!(
                "{} field {name}",
                if edge_kind == FlowEdgeKind::FieldRead {
                    "read"
                } else {
                    "write"
                }
            ),
        );
        vec![field]
    }

    fn eval_call(&mut self, node: Node<'_>) -> Vec<NodeId> {
        let Some(target_name) = self.direct_call_name(node) else {
            return Vec::new();
        };
        let argument_values = self.call_arguments(node);
        let Some(target_index) = self.resolve_call_target(&target_name) else {
            return Vec::new();
        };
        let parameters = self.graph.functions[target_index].parameter_nodes.clone();
        if parameters.len() != argument_values.len() {
            self.graph.unresolved(format!(
                "argument arity mismatch for {} {target_name}",
                language(self.file)
            ));
            return Vec::new();
        }
        let line = node.start_position().row + 1;
        let call_site = format!(
            "{}:{line}:{}:{}",
            stable_path(self.root, &self.file.file.path),
            self.function_index,
            self.ordinal
        );
        self.connect_call_arguments(&target_name, &argument_values, parameters, line, &call_site);
        vec![self.record_call_result(target_index, &target_name, line, call_site)]
    }

    fn direct_call_name(&mut self, node: Node<'_>) -> Option<String> {
        let function = node
            .child_by_field_name("function")
            .or_else(|| node.child_by_field_name("name"));
        let function = function?;
        if function.kind() != "identifier" {
            self.graph.unresolved(format!(
                "unsupported {} method or dynamic call",
                language(self.file)
            ));
            self.eval_named_children(node);
            return None;
        }
        node_text(function, self.file)
    }

    fn call_arguments(&mut self, node: Node<'_>) -> Vec<Vec<NodeId>> {
        let arguments = node
            .child_by_field_name("arguments")
            .or_else(|| node.child_by_field_name("argument_list"));
        arguments
            .map(|arguments| {
                let mut cursor = arguments.walk();
                arguments
                    .named_children(&mut cursor)
                    .map(|argument| self.eval_expr(argument))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    fn resolve_call_target(&mut self, target_name: &str) -> Option<usize> {
        let imported = self.imports.get(target_name);
        let mut matches = self
            .graph
            .functions
            .iter()
            .enumerate()
            .filter(|(_, function)| {
                function.crate_key == language(self.file)
                    && function.symbol.rsplit("::").next()
                        == Some(
                            imported
                                .map(|target| target.exported_name.as_str())
                                .unwrap_or(target_name),
                        )
                    && imported
                        .is_none_or(|target| module_matches(&function.module, &target.module_hint))
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if imported.is_none() {
            let current_module = &self.graph.functions[self.function_index].module;
            let local = matches
                .iter()
                .copied()
                .filter(|index| self.graph.functions[*index].module == *current_module)
                .collect::<Vec<_>>();
            if !local.is_empty() {
                matches = local;
            }
        }
        if matches.len() != 1 {
            self.graph.unresolved(format!(
                "{} {} call target {target_name}",
                if matches.is_empty() {
                    "unresolved"
                } else {
                    "ambiguous"
                },
                language(self.file)
            ));
            return None;
        }
        Some(matches[0])
    }

    fn connect_call_arguments(
        &mut self,
        target_name: &str,
        argument_values: &[Vec<NodeId>],
        parameters: Vec<NodeId>,
        line: usize,
        call_site: &str,
    ) {
        for (index, (sources, parameter)) in argument_values.iter().zip(parameters).enumerate() {
            for source in sources {
                self.graph.add_edge(FlowEdge {
                    from: *source,
                    to: parameter,
                    kind: FlowEdgeKind::ArgumentToParameter,
                    resolution: FlowResolution::Exact,
                    path: self.file.file.display_path.clone(),
                    line,
                    name: format!("call {target_name} argument {index}"),
                    call_site: Some(call_site.into()),
                    transition: CallTransition::Enter,
                });
            }
        }
    }

    fn record_call_result(
        &mut self,
        target_index: usize,
        target_name: &str,
        line: usize,
        call_site: String,
    ) -> NodeId {
        let result = self.add_node(
            FlowNodeKind::CallResult,
            line,
            &format!("result of {target_name}"),
            "result",
        );
        self.graph.add_edge(FlowEdge {
            from: self.graph.functions[target_index].return_node,
            to: result,
            kind: FlowEdgeKind::ReturnToResult,
            resolution: FlowResolution::Exact,
            path: self.file.file.display_path.clone(),
            line,
            name: format!("return from {target_name}"),
            call_site: Some(call_site),
            transition: CallTransition::Return,
        });
        self.graph.calls.push(CallRecord {
            target: self.graph.functions[target_index].symbol.clone(),
            function_index: target_index,
            path: self.file.file.display_path.clone(),
            line,
        });
        result
    }

    fn connect_return(&mut self, sources: &[NodeId], line: usize) {
        let target = self.graph.functions[self.function_index].return_node;
        self.connect(
            sources,
            target,
            FlowEdgeKind::Assignment,
            line,
            "return".into(),
        );
    }

    fn add_node(&mut self, kind: FlowNodeKind, line: usize, name: &str, category: &str) -> NodeId {
        let ordinal = self.ordinal;
        self.ordinal += 1;
        let function = &self.graph.functions[self.function_index];
        add_location(
            self.graph,
            FlowLocation {
                id: format!(
                    "flow:{}:{}:{}-{ordinal}",
                    language(self.file),
                    stable_path(self.root, &self.file.file.path),
                    category
                ),
                kind,
                language: language(self.file).into(),
                path: self.file.file.display_path.clone(),
                line,
                function: function.symbol.clone(),
                module: function.module.clone(),
                name: name.into(),
            },
        )
    }

    fn connect(
        &mut self,
        sources: &[NodeId],
        target: NodeId,
        kind: FlowEdgeKind,
        line: usize,
        name: String,
    ) {
        for source in sources {
            if *source != target {
                self.graph.add_edge(FlowEdge {
                    from: *source,
                    to: target,
                    kind,
                    resolution: FlowResolution::Exact,
                    path: self.file.file.display_path.clone(),
                    line,
                    name: name.clone(),
                    call_site: None,
                    transition: CallTransition::None,
                });
            }
        }
    }
}

fn binding_name(node: Node<'_>, file: &ParsedSourceFile) -> Option<String> {
    if node.kind() == "identifier" {
        node_text(node, file)
    } else {
        None
    }
}

fn find_node(root: Node<'_>, start: usize) -> Option<Node<'_>> {
    if root.start_byte() == start {
        return Some(root);
    }
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.start_byte() <= start
            && child.end_byte() >= start
            && let Some(found) = find_node(child, start)
        {
            return Some(found);
        }
    }
    None
}

fn node_text(node: Node<'_>, file: &ParsedSourceFile) -> Option<String> {
    node.utf8_text(file.file.source.as_bytes())
        .ok()
        .map(str::to_owned)
}

fn add_location(graph: &mut FlowGraph, location: FlowLocation) -> NodeId {
    let id = graph.nodes.len();
    graph.nodes.push(location);
    id
}

fn stable_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn language(file: &ParsedSourceFile) -> &'static str {
    match file.family {
        LanguageFamily::JavaScriptTypeScript => match file
            .file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("ts" | "mts" | "cts") => TYPESCRIPT_LANGUAGE,
            Some("tsx" | "vue") => "tsx",
            _ => JAVASCRIPT_LANGUAGE,
        },
        LanguageFamily::Python => "python",
        _ => "unsupported",
    }
}

fn is_literal(kind: &str) -> bool {
    kind.contains("string")
        || kind.contains("number")
        || kind.contains("integer")
        || kind.contains("float")
        || matches!(kind, "true" | "false" | "none" | "null" | "undefined")
}
