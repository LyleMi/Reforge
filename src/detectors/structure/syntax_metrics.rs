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
        LanguageFamily::Java
        | LanguageFamily::CSharp
        | LanguageFamily::Kotlin
        | LanguageFamily::Php
        | LanguageFamily::Ruby => added_language_function_parts(node, traversal),
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
        parameters,
        body,
    })
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
            | "try_statement"
            | "if"
            | "unless"
            | "for"
            | "while"
            | "case"
            | "when"
            | "rescue"
    ) || (family == LanguageFamily::Python && kind == "elif_clause")
}
