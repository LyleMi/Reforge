use super::*;

mod aggregation;
mod public_items;

pub(super) use aggregation::*;
pub(super) use public_items::count_public_items;

pub(super) fn type_metric(node: Node<'_>, traversal: StructureTraversal<'_>) -> Option<TypeMetric> {
    let kind = node.kind();
    let name_node = match traversal.family {
        LanguageFamily::Rust
            if matches!(
                kind,
                "struct_item" | "enum_item" | "trait_item" | "impl_item"
            ) =>
        {
            node.child_by_field_name("name")
        }
        LanguageFamily::JavaScriptTypeScript
            if matches!(
                kind,
                "class_declaration" | "interface_declaration" | "type_alias_declaration"
            ) =>
        {
            node.child_by_field_name("name")
        }
        LanguageFamily::Python if kind == "class_definition" => node.child_by_field_name("name"),
        LanguageFamily::Go if kind == "type_spec" => node.child_by_field_name("name"),
        LanguageFamily::Java
        | LanguageFamily::CSharp
        | LanguageFamily::Kotlin
        | LanguageFamily::Php
        | LanguageFamily::Ruby => added_language_type_name_node(node, traversal.family),
        _ => None,
    }?;

    let name = name_node
        .utf8_text(traversal.source.as_bytes())
        .ok()?
        .to_string();
    Some(TypeMetric {
        name,
        line: node.start_position().row + 1,
        lines: node_line_span(node),
        members: count_type_members(node, traversal.family),
    })
}

fn added_language_type_name_node<'tree>(
    node: Node<'tree>,
    family: LanguageFamily,
) -> Option<Node<'tree>> {
    let kind = node.kind();
    let is_type = match family {
        LanguageFamily::Java => matches!(
            kind,
            "class_declaration"
                | "interface_declaration"
                | "enum_declaration"
                | "annotation_type_declaration"
        ),
        LanguageFamily::CSharp => matches!(
            kind,
            "class_declaration"
                | "interface_declaration"
                | "struct_declaration"
                | "record_declaration"
                | "enum_declaration"
        ),
        LanguageFamily::Kotlin => matches!(kind, "class_declaration" | "object_declaration"),
        LanguageFamily::Php => matches!(
            kind,
            "class_declaration"
                | "interface_declaration"
                | "trait_declaration"
                | "enum_declaration"
        ),
        LanguageFamily::Ruby => matches!(kind, "class" | "module"),
        _ => false,
    };

    is_type.then(|| node.child_by_field_name("name")).flatten()
}

pub(super) fn count_type_members(node: Node<'_>, family: LanguageFamily) -> usize {
    let mut count = 0;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if is_type_member(child, family) {
            count += 1;
        }
        count += count_type_members(child, family);
    }
    count
}

pub(super) fn is_type_member(node: Node<'_>, family: LanguageFamily) -> bool {
    let kind = node.kind();
    match family {
        LanguageFamily::Rust => matches!(
            kind,
            "field_declaration" | FUNCTION_ITEM | "associated_type" | "const_item"
        ),
        LanguageFamily::JavaScriptTypeScript => matches!(
            kind,
            METHOD_DEFINITION
                | "public_field_definition"
                | "field_definition"
                | "property_signature"
                | "method_signature"
        ),
        LanguageFamily::Python => matches!(kind, FUNCTION_DEFINITION | "assignment"),
        LanguageFamily::Go => matches!(kind, "field_declaration" | "method_elem"),
        LanguageFamily::Java
        | LanguageFamily::CSharp
        | LanguageFamily::Kotlin
        | LanguageFamily::Php
        | LanguageFamily::Ruby => added_language_type_member(kind, family),
    }
}

fn added_language_type_member(kind: &str, family: LanguageFamily) -> bool {
    match family {
        LanguageFamily::Java => matches!(
            kind,
            METHOD_DECLARATION
                | "constructor_declaration"
                | "compact_constructor_declaration"
                | "field_declaration"
        ),
        LanguageFamily::CSharp => matches!(
            kind,
            METHOD_DECLARATION
                | "constructor_declaration"
                | "field_declaration"
                | "property_declaration"
        ),
        LanguageFamily::Kotlin => matches!(kind, FUNCTION_DECLARATION | "property_declaration"),
        LanguageFamily::Php => matches!(
            kind,
            METHOD_DECLARATION | "property_declaration" | "const_declaration"
        ),
        LanguageFamily::Ruby => matches!(kind, "method" | "singleton_method"),
        _ => false,
    }
}

pub(super) fn count_imports(root: Node<'_>, family: LanguageFamily) -> usize {
    let mut count = 0;
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match family {
            LanguageFamily::Rust if child.kind() == "use_declaration" => count += 1,
            LanguageFamily::JavaScriptTypeScript if child.kind() == "import_statement" => {
                count += 1
            }
            LanguageFamily::Python
                if matches!(child.kind(), "import_statement" | "import_from_statement") =>
            {
                count += 1
            }
            LanguageFamily::Go if child.kind() == "import_declaration" => {
                count += count_named_descendants(child, "import_spec").max(1);
            }
            LanguageFamily::Java
            | LanguageFamily::CSharp
            | LanguageFamily::Kotlin
            | LanguageFamily::Php
                if added_language_import(child.kind(), family) =>
            {
                count += 1
            }
            _ => {}
        }
    }
    count
}

fn added_language_import(kind: &str, family: LanguageFamily) -> bool {
    matches!(
        (family, kind),
        (LanguageFamily::Java, "import_declaration")
            | (LanguageFamily::CSharp, "using_directive")
            | (LanguageFamily::Kotlin, "import_header")
            | (LanguageFamily::Php, "namespace_use_declaration")
    )
}

pub(super) fn should_skip_rust_test_module(
    node: Node<'_>,
    traversal: StructureTraversal<'_>,
) -> bool {
    traversal.family == LanguageFamily::Rust
        && !traversal.include_test_structure
        && node.kind() == "mod_item"
        && has_rust_cfg_test_attribute(node, traversal.source)
}

pub(super) fn python_public_item(node: Node<'_>, source: &str) -> bool {
    if !matches!(node.kind(), FUNCTION_DEFINITION | "class_definition") {
        return false;
    }

    node.child_by_field_name("name")
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .is_some_and(|name| !name.starts_with('_'))
}

pub(super) fn go_public_item(node: Node<'_>, source: &str) -> bool {
    if !matches!(
        node.kind(),
        FUNCTION_DECLARATION | METHOD_DECLARATION | "type_declaration"
    ) {
        return false;
    }

    node.child_by_field_name("name")
        .or_else(|| {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|child| child.kind() == "type_spec")
                .and_then(|spec| spec.child_by_field_name("name"))
        })
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .is_some_and(is_exported_go_identifier)
}

pub(super) fn is_exported_go_identifier(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|character| character.is_uppercase())
}

fn has_public_modifier(node: Node<'_>, source: &str) -> bool {
    child_by_kind(node, "modifiers")
        .or_else(|| child_by_kind(node, "modifier"))
        .and_then(|modifiers| modifiers.utf8_text(source.as_bytes()).ok())
        .is_some_and(|text| text.split_whitespace().any(|token| token == "public"))
}

fn kotlin_public_item(node: Node<'_>, source: &str) -> bool {
    if !matches!(
        node.kind(),
        FUNCTION_DECLARATION | "class_declaration" | "object_declaration"
    ) {
        return false;
    }

    child_by_kind(node, "modifiers")
        .and_then(|modifiers| modifiers.utf8_text(source.as_bytes()).ok())
        .is_none_or(|text| {
            !text
                .split_whitespace()
                .any(|token| matches!(token, "private" | "internal" | "protected"))
        })
}

fn php_public_item(node: Node<'_>, source: &str) -> bool {
    match node.kind() {
        FUNCTION_DEFINITION
        | "class_declaration"
        | "interface_declaration"
        | "trait_declaration"
        | "enum_declaration" => true,
        METHOD_DECLARATION => node
            .utf8_text(source.as_bytes())
            .ok()
            .is_none_or(|text| !text.split_whitespace().any(|token| token == "private")),
        _ => false,
    }
}

fn ruby_public_item(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "class" | "module" | "method" | "singleton_method"
    )
}

impl StructureSignalCollector<'_, '_> {
    pub(super) fn collect_literal_occurrence(&mut self, node: Node<'_>) {
        if is_literal_node(node)
            && !has_literal_ancestor(node)
            && !has_repeated_literal_noise_ancestor(node)
            && let Ok(text) = node.utf8_text(self.traversal.source.as_bytes())
            && let Some(literal) = normalize_literal(text)
        {
            self.signals
                .literals
                .push((literal, occurrence(self.file, node, None)));
        }
    }

    pub(super) fn collect_error_occurrence(&mut self, node: Node<'_>) {
        if is_error_pattern_node(node, self.traversal)
            && let Ok(text) = node.utf8_text(self.traversal.source.as_bytes())
        {
            self.signals
                .error_patterns
                .push((normalize_pattern(text), occurrence(self.file, node, None)));
        }
    }
}
