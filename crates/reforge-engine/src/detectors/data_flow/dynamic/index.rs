use super::*;

pub(super) struct FunctionIndexer<'a> {
    root: &'a Path,
    file_index: usize,
    file: &'a ParsedSourceFile,
    graph: &'a mut FlowGraph,
    indexed: &'a mut Vec<IndexedFunction>,
}

impl<'a> FunctionIndexer<'a> {
    pub(super) fn new(
        root: &'a Path,
        file_index: usize,
        file: &'a ParsedSourceFile,
        graph: &'a mut FlowGraph,
        indexed: &'a mut Vec<IndexedFunction>,
    ) -> Self {
        Self {
            root,
            file_index,
            file,
            graph,
            indexed,
        }
    }

    pub(super) fn index(&mut self, node: Node<'_>) {
        self.index_with_owner(node, None);
    }

    fn index_with_owner(&mut self, node: Node<'_>, owner: Option<usize>) {
        if let Some(parts) = function_parts(node, self.file) {
            let module = stable_path(self.root, &self.file.file.path);
            let language = language(self.file);
            let qualified_name = owner.map_or_else(
                || parts.name.clone(),
                |owner| {
                    format!(
                        "{}::{}",
                        self.graph.functions[owner]
                            .symbol
                            .split_once("::")
                            .map(|(_, name)| name)
                            .unwrap_or(&self.graph.functions[owner].symbol),
                        parts.name
                    )
                },
            );
            let symbol = format!("{language}:{module}::{qualified_name}");
            let parameters =
                index_parameters(self.file, parts.parameters, &module, &symbol, self.graph);
            let return_node = add_location(
                self.graph,
                FlowLocation {
                    id: format!("flow:{language}:{module}:{symbol}:return"),
                    kind: FlowNodeKind::Return,
                    language: language.into(),
                    path: self.file.file.display_path.clone(),
                    line: node.start_position().row + 1,
                    function: symbol.clone(),
                    module: module.clone(),
                    name: "return".into(),
                },
            );
            let function_index = self.graph.functions.len();
            self.graph.functions.push(FunctionRecord {
                symbol,
                public: owner.is_none() && parts.public,
                owner,
                crate_key: language.into(),
                module,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                parameter_nodes: parameters.clone(),
                parameter_groups: parameters.iter().map(|node| vec![*node]).collect(),
                parameter_groups_exact: vec![true; parameters.len()],
                return_node,
            });
            self.indexed.push(IndexedFunction {
                file_index: self.file_index,
                function_index,
                node_start: node.start_byte(),
                body_start: parts.body.start_byte(),
            });
            self.index_children(parts.body, Some(function_index));
            return;
        }

        self.index_children(node, owner);
    }

    fn index_children(&mut self, node: Node<'_>, owner: Option<usize>) {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.index_with_owner(child, owner);
        }
    }
}

struct FunctionParts<'tree> {
    name: String,
    parameters: Node<'tree>,
    body: Node<'tree>,
    public: bool,
}

fn function_parts<'tree>(
    node: Node<'tree>,
    file: &ParsedSourceFile,
) -> Option<FunctionParts<'tree>> {
    match file.family {
        LanguageFamily::JavaScriptTypeScript
            if matches!(
                node.kind(),
                "function_declaration" | "generator_function_declaration"
            ) =>
        {
            let name = node_text(node.child_by_field_name("name")?, file)?;
            Some(FunctionParts {
                public: has_export_ancestor(node),
                name,
                parameters: node.child_by_field_name("parameters")?,
                body: node.child_by_field_name("body")?,
            })
        }
        LanguageFamily::JavaScriptTypeScript if node.kind() == "variable_declarator" => {
            let value = node.child_by_field_name("value")?;
            if !matches!(
                value.kind(),
                "arrow_function" | "function_expression" | "generator_function"
            ) {
                return None;
            }
            Some(FunctionParts {
                public: has_export_ancestor(node),
                name: node_text(node.child_by_field_name("name")?, file)?,
                parameters: value.child_by_field_name("parameters")?,
                body: value.child_by_field_name("body")?,
            })
        }
        LanguageFamily::Python if node.kind() == "function_definition" => {
            let name = node_text(node.child_by_field_name("name")?, file)?;
            Some(FunctionParts {
                public: !name.starts_with('_'),
                name,
                parameters: node.child_by_field_name("parameters")?,
                body: node.child_by_field_name("body")?,
            })
        }
        _ => None,
    }
}

fn has_export_ancestor(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == "export_statement" {
            return true;
        }
        if matches!(parent.kind(), "program" | "module") {
            break;
        }
        node = parent;
    }
    false
}

fn index_parameters(
    file: &ParsedSourceFile,
    parameters: Node<'_>,
    module: &str,
    symbol: &str,
    graph: &mut FlowGraph,
) -> Vec<NodeId> {
    let mut names = Vec::new();
    collect_parameter_names(parameters, file, &mut names);
    names
        .into_iter()
        .enumerate()
        .map(|(ordinal, (name, line))| {
            add_location(
                graph,
                FlowLocation {
                    id: format!("flow:{}:{module}:{symbol}:param-{ordinal}", language(file)),
                    kind: FlowNodeKind::Parameter,
                    language: language(file).into(),
                    path: file.file.display_path.clone(),
                    line,
                    function: symbol.into(),
                    module: module.into(),
                    name,
                },
            )
        })
        .collect()
}

fn collect_parameter_names(
    node: Node<'_>,
    file: &ParsedSourceFile,
    output: &mut Vec<(String, usize)>,
) {
    if matches!(
        node.kind(),
        "identifier" | "shorthand_property_identifier_pattern"
    ) {
        if let Some(name) = node_text(node, file) {
            output.push((name, node.start_position().row + 1));
        }
        return;
    }
    if matches!(
        node.kind(),
        "type_annotation" | "type" | "comment" | "string"
    ) {
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_parameter_names(child, file, output);
    }
}
