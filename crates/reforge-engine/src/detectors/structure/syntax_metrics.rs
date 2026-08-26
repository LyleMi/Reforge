struct FunctionParts<'tree> {
    name: String,
    is_anonymous: bool,
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
            is_anonymous: false,
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
                    | "function_expression"
            ) =>
        {
            let explicit_name = function_name(node, source);
            let binding_name = callable_binding_name(node, source);
            let is_anonymous = explicit_name.is_none() && binding_name.is_none();
            Some(FunctionParts {
                name: explicit_name
                    .or(binding_name)
                    .unwrap_or_else(|| "<anonymous>".to_string()),
                is_anonymous,
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
            is_anonymous: false,
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
                is_anonymous: false,
                parameters: node.child_by_field_name(PARAMETERS_FIELD),
                body: node.child_by_field_name(BODY_FIELD)?,
            })
        }
        LanguageFamily::Java
        | LanguageFamily::CSharp
        | LanguageFamily::Kotlin
        | LanguageFamily::Php
        | LanguageFamily::Ruby
        | LanguageFamily::Bash
        | LanguageFamily::PowerShell => added_language_function_parts(node, traversal),
        _ => None,
    }
}

fn function_name(node: Node<'_>, source: &str) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .map(ToString::to_string)
}

fn added_language_function_parts<'tree>(
    node: Node<'tree>,
    traversal: StructureTraversal<'_>,
) -> Option<FunctionParts<'tree>> {
    match traversal.family {
        LanguageFamily::Java | LanguageFamily::CSharp => method_function_parts(node, traversal),
        LanguageFamily::Kotlin => kotlin_function_parts(node, traversal),
        LanguageFamily::Php => php_function_parts(node, traversal),
        LanguageFamily::Ruby => ruby_function_parts(node, traversal),
        LanguageFamily::Bash => bash_function_parts(node, traversal),
        LanguageFamily::PowerShell => powershell_function_parts(node, traversal),
        _ => None,
    }
}

fn method_function_parts<'tree>(
    node: Node<'tree>,
    traversal: StructureTraversal<'_>,
) -> Option<FunctionParts<'tree>> {
    let is_supported = node.kind() == METHOD_DECLARATION
        || (traversal.family == LanguageFamily::CSharp
            && matches!(
                node.kind(),
                "constructor_declaration" | "local_function_statement"
            ));
    if !is_supported {
        return None;
    }
    named_function_parts(
        node,
        traversal.source,
        node.child_by_field_name(PARAMETERS_FIELD),
        node.child_by_field_name(BODY_FIELD)?,
    )
}

fn kotlin_function_parts<'tree>(
    node: Node<'tree>,
    traversal: StructureTraversal<'_>,
) -> Option<FunctionParts<'tree>> {
    if node.kind() != FUNCTION_DECLARATION {
        return None;
    }
    named_function_parts(
        node,
        traversal.source,
        child_by_kind(node, "function_value_parameters"),
        child_by_kind(node, "function_body")?,
    )
}

fn php_function_parts<'tree>(
    node: Node<'tree>,
    traversal: StructureTraversal<'_>,
) -> Option<FunctionParts<'tree>> {
    if !matches!(node.kind(), FUNCTION_DEFINITION | METHOD_DECLARATION) {
        return None;
    }
    named_function_parts(
        node,
        traversal.source,
        node.child_by_field_name(PARAMETERS_FIELD),
        node.child_by_field_name(BODY_FIELD)?,
    )
}

fn ruby_function_parts<'tree>(
    node: Node<'tree>,
    traversal: StructureTraversal<'_>,
) -> Option<FunctionParts<'tree>> {
    if !matches!(node.kind(), "method" | "singleton_method") {
        return None;
    }
    named_function_parts(
        node,
        traversal.source,
        node.child_by_field_name(PARAMETERS_FIELD),
        node.child_by_field_name(BODY_FIELD)?,
    )
}

fn bash_function_parts<'tree>(
    node: Node<'tree>,
    traversal: StructureTraversal<'_>,
) -> Option<FunctionParts<'tree>> {
    if node.kind() != FUNCTION_DEFINITION {
        return None;
    }
    named_function_parts(
        node,
        traversal.source,
        None,
        node.child_by_field_name(BODY_FIELD)?,
    )
}

fn powershell_function_parts<'tree>(
    node: Node<'tree>,
    traversal: StructureTraversal<'_>,
) -> Option<FunctionParts<'tree>> {
    if node.kind() != "function_statement" {
        return None;
    }
    Some(FunctionParts {
        name: child_by_kind(node, "function_name")?
            .utf8_text(traversal.source.as_bytes())
            .ok()?
            .to_string(),
        is_anonymous: false,
        parameters: child_by_kind(node, "function_parameter_declaration")
            .or_else(|| powershell_script_block_param_block(node)),
        body: child_by_kind(node, "script_block")?,
    })
}

fn powershell_script_block_param_block<'tree>(node: Node<'tree>) -> Option<Node<'tree>> {
    let script_block = child_by_kind(node, "script_block")?;
    child_by_kind(script_block, "param_block")
}

fn named_function_parts<'tree>(
    node: Node<'tree>,
    source: &str,
    parameters: Option<Node<'tree>>,
    body: Node<'tree>,
) -> Option<FunctionParts<'tree>> {
    Some(FunctionParts {
        name: node
            .child_by_field_name(NAME_FIELD)?
            .utf8_text(source.as_bytes())
            .ok()?
            .to_string(),
        is_anonymous: false,
        parameters,
        body,
    })
}

fn callable_binding_name(node: Node<'_>, source: &str) -> Option<String> {
    let declarator = node.parent().filter(|parent| parent.kind() == "variable_declarator")?;
    if declarator.child_by_field_name("value")?.id() != node.id() {
        return None;
    }
    let name = declarator.child_by_field_name(NAME_FIELD)?;
    if name.kind() != IDENTIFIER_KIND {
        return None;
    }
    name.utf8_text(source.as_bytes()).ok().map(str::to_string)
}

fn is_module_scope_callable(node: Node<'_>, family: LanguageFamily) -> bool {
    if family != LanguageFamily::JavaScriptTypeScript || node.kind() == METHOD_DEFINITION {
        return false;
    }
    let mut parent = node.parent();
    if matches!(node.kind(), ARROW_FUNCTION | "function_expression") {
        let Some(declarator) = parent.filter(|parent| parent.kind() == "variable_declarator") else {
            return false;
        };
        parent = declarator.parent();
    }
    while parent.is_some_and(|node| {
        matches!(
            node.kind(),
            "lexical_declaration" | "variable_declaration" | "export_statement"
        )
    }) {
        parent = parent.and_then(|node| node.parent());
    }
    parent.is_some_and(|node| node.kind() == "program")
}

fn direct_identifier_calls(body: Node<'_>, traversal: StructureTraversal<'_>) -> BTreeSet<String> {
    if traversal.family != LanguageFamily::JavaScriptTypeScript {
        return BTreeSet::new();
    }
    let mut calls = BTreeSet::new();
    collect_direct_identifier_calls(body, traversal.source, &mut calls, true);
    calls
}

fn collect_direct_identifier_calls(
    node: Node<'_>,
    source: &str,
    calls: &mut BTreeSet<String>,
    is_root: bool,
) {
    if !is_root && is_javascript_callable(node.kind()) {
        return;
    }
    if node.kind() == "call_expression"
        && let Some(callee) = node.child_by_field_name("function")
        && callee.kind() == IDENTIFIER_KIND
        && let Ok(name) = callee.utf8_text(source.as_bytes())
    {
        calls.insert(name.to_string());
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_direct_identifier_calls(child, source, calls, false);
    }
}

fn is_javascript_callable(kind: &str) -> bool {
    matches!(
        kind,
        FUNCTION_DECLARATION
            | GENERATOR_FUNCTION_DECLARATION
            | METHOD_DEFINITION
            | ARROW_FUNCTION
            | "function_expression"
    )
}

fn collect_local_call_bindings(
    node: Node<'_>,
    traversal: StructureTraversal<'_>,
    bindings: &mut BTreeSet<String>,
    is_root: bool,
) {
    if traversal.family != LanguageFamily::JavaScriptTypeScript {
        return;
    }
    if !is_root && is_javascript_callable(node.kind()) {
        if matches!(node.kind(), FUNCTION_DECLARATION | GENERATOR_FUNCTION_DECLARATION)
            && let Some(name) = node.child_by_field_name(NAME_FIELD)
            && let Ok(name) = name.utf8_text(traversal.source.as_bytes())
        {
            bindings.insert(name.to_string());
        }
        return;
    }
    if node.kind() == "variable_declarator"
        && let Some(name) = node.child_by_field_name(NAME_FIELD)
        && name.kind() == IDENTIFIER_KIND
        && let Ok(name) = name.utf8_text(traversal.source.as_bytes())
    {
        bindings.insert(name.to_string());
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_local_call_bindings(child, traversal, bindings, false);
    }
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
            | "switch_statement"
            | "case_clause"
            | "case_item"
            | "elif_clause"
            | "catch_clause"
            | "except_clause"
            | "conditional_expression"
            | "try_statement"
            | "if"
            | "unless"
            | "for"
            | "while"
            | "case"
            | "when"
            | "rescue"
            | "foreach_statement"
            | "do_statement"
            | "case_statement"
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
            | "case_item"
            | "elif_clause"
            | "catch_clause"
            | "except_clause"
            | "try_statement"
            | "if"
            | "unless"
            | "for"
            | "while"
            | "case"
            | "when"
            | "rescue"
            | "foreach_statement"
            | "do_statement"
            | "case_statement"
    ) || (family == LanguageFamily::Python && kind == "elif_clause")
}
