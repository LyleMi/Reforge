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
