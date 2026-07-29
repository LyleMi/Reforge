use super::*;

impl DynamicAnalyzer<'_> {
    pub(super) fn eval_expr(&mut self, node: Node<'_>) -> Vec<NodeId> {
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
            _ => self.eval_transformation(node),
        }
    }

    fn eval_transformation(&mut self, node: Node<'_>) -> Vec<NodeId> {
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

    pub(super) fn eval_named_children(&mut self, node: Node<'_>) -> Vec<NodeId> {
        let mut values = Vec::new();
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            values.extend(self.eval_expr(child));
        }
        values.sort_unstable();
        values.dedup();
        values
    }

    pub(super) fn eval_field(&mut self, node: Node<'_>, edge_kind: FlowEdgeKind) -> Vec<NodeId> {
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
            self.graph.unresolved(
                language(self.file),
                format!(
                    "argument arity mismatch for {} {target_name}",
                    language(self.file)
                ),
            );
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
            .or_else(|| node.child_by_field_name("name"))?;
        if function.kind() != "identifier" {
            self.graph.unresolved(
                language(self.file),
                format!("unsupported {} method or dynamic call", language(self.file)),
            );
            self.eval_named_children(node);
            return None;
        }
        node_text(function, self.file)
    }

    fn call_arguments(&mut self, node: Node<'_>) -> Vec<Vec<NodeId>> {
        node.child_by_field_name("arguments")
            .or_else(|| node.child_by_field_name("argument_list"))
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
        if imported.is_some() {
            matches.retain(|index| self.graph.functions[*index].public);
        } else {
            matches = self.closest_lexical_matches(matches);
        }
        if matches.len() != 1 {
            self.graph.unresolved(
                language(self.file),
                format!(
                    "{} {} call target {target_name}",
                    if matches.is_empty() {
                        "unresolved"
                    } else {
                        "ambiguous"
                    },
                    language(self.file)
                ),
            );
            return None;
        }
        Some(matches[0])
    }

    fn closest_lexical_matches(&self, matches: Vec<usize>) -> Vec<usize> {
        let current_module = &self.graph.functions[self.function_index].module;
        let mut local = matches
            .iter()
            .copied()
            .filter(|index| self.graph.functions[*index].module == *current_module)
            .filter_map(|index| {
                self.lexical_distance(index)
                    .map(|distance| (index, distance))
            })
            .collect::<Vec<_>>();
        local.sort_by_key(|(_, distance)| *distance);
        let Some(closest) = local.first().map(|(_, distance)| *distance) else {
            return matches;
        };
        local
            .into_iter()
            .take_while(|(_, distance)| *distance == closest)
            .map(|(index, _)| index)
            .collect()
    }

    fn lexical_distance(&self, candidate: usize) -> Option<usize> {
        if candidate == self.function_index {
            return Some(0);
        }
        let mut scope = Some(self.function_index);
        let mut distance = 1;
        while let Some(owner) = scope {
            if self.graph.functions[candidate].owner == Some(owner) {
                return Some(distance);
            }
            scope = self.graph.functions[owner].owner;
            distance += 1;
        }
        self.graph.functions[candidate]
            .owner
            .is_none()
            .then_some(distance)
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
        result
    }
}
