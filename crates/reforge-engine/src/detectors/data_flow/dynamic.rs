use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tree_sitter::Node;

use crate::detectors::similarity::ParsedSourceFile;
use crate::lang::{JAVASCRIPT_LANGUAGE, LanguageFamily, TYPESCRIPT_LANGUAGE};
use crate::model::{FlowEdgeKind, FlowLocation, FlowNodeKind, FlowResolution};

use super::model::{CallTransition, FlowEdge, FlowGraph, FunctionRecord, NodeId};

mod accessors;
use accessors::*;
mod expressions;
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
    family: LanguageFamily,
) {
    let files = parsed_sources
        .iter()
        .enumerate()
        .filter(|(_, file)| file.family == family)
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
    if matches!(body.kind(), "statement_block" | "block") {
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
            self.graph.unresolved(
                language(self.file),
                format!(
                    "unsupported {} destructuring assignment",
                    language(self.file)
                ),
            );
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
                    resolution: if matches!(
                        kind,
                        FlowEdgeKind::FieldRead | FlowEdgeKind::FieldWrite | FlowEdgeKind::Mutation
                    ) {
                        FlowResolution::Modeled
                    } else {
                        FlowResolution::Exact
                    },
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
