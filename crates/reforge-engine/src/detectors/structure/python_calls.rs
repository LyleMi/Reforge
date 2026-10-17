use super::*;

pub(super) fn calls(body: Node<'_>, source: &str) -> BTreeSet<String> {
    let mut calls = BTreeSet::new();
    let mut pending = vec![body];
    while let Some(node) = pending.pop() {
        if introduces_scope(node.kind()) {
            continue;
        }
        if node.kind() == "call"
            && let Some(callee) = node.child_by_field_name("function")
            && callee.kind() == IDENTIFIER_KIND
            && let Ok(name) = callee.utf8_text(source.as_bytes())
        {
            calls.insert(name.into());
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
    calls
}

fn introduces_scope(kind: &str) -> bool {
    matches!(
        kind,
        "function_definition"
            | "class_definition"
            | "lambda"
            | "list_comprehension"
            | "set_comprehension"
            | "dictionary_comprehension"
            | "generator_expression"
    )
}

pub(super) fn bindings(body: Node<'_>, source: &str, output: &mut BTreeSet<String>) {
    let mut pending = vec![body];
    while let Some(node) = pending.pop() {
        if introduces_scope(node.kind()) {
            if let Some(name) = node.child_by_field_name("name") {
                identifiers(name, source, output);
            }
            continue;
        }
        collect_binding_target(node, source, output);
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
}

fn collect_binding_target(node: Node<'_>, source: &str, output: &mut BTreeSet<String>) {
    let field = match node.kind() {
        "assignment" | "augmented_assignment" | "for_statement" | "for_in_clause" => "left",
        "named_expression" => "name",
        "as_pattern" => "alias",
        "import_statement"
        | "import_from_statement"
        | "global_statement"
        | "nonlocal_statement" => {
            // Conservative for aliases and mutation of module state: no call
            // through these names is treated as an exact local function edge.
            identifiers(node, source, output);
            return;
        }
        _ => return,
    };
    if let Some(target) = node.child_by_field_name(field) {
        identifiers(target, source, output);
    }
}

fn identifiers(root: Node<'_>, source: &str, output: &mut BTreeSet<String>) {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.kind() == IDENTIFIER_KIND
            && let Ok(name) = node.utf8_text(source.as_bytes())
        {
            output.insert(name.into());
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
}

pub(super) fn module_rebindings(root: Node<'_>, source: &str) -> BTreeSet<String> {
    let mut rebound = BTreeSet::new();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        if node.kind() != "function_definition" {
            bindings(node, source, &mut rebound);
        }
    }
    rebound
}
