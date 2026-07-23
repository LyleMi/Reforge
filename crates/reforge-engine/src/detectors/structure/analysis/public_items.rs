use super::*;

pub(in crate::detectors::structure) fn count_public_items(
    root: Node<'_>,
    traversal: StructureTraversal<'_>,
) -> usize {
    count_public_items_in_scope(root, traversal)
}

fn count_public_items_in_scope(scope: Node<'_>, traversal: StructureTraversal<'_>) -> usize {
    let mut count = 0;
    let mut cursor = scope.walk();
    for child in scope.named_children(&mut cursor) {
        if should_skip_rust_test_module(child, traversal) {
            continue;
        }

        if traversal.family == LanguageFamily::CSharp
            && matches!(
                child.kind(),
                "namespace_declaration" | "file_scoped_namespace_declaration" | "declaration_list"
            )
        {
            count += count_public_items_in_scope(child, traversal);
            continue;
        }

        count += match traversal.family {
            LanguageFamily::Rust if rust_public_item(child) => 1,
            LanguageFamily::JavaScriptTypeScript if child.kind() == "export_statement" => 1,
            LanguageFamily::Python if python_public_item(child, traversal.source) => 1,
            LanguageFamily::Go if go_public_item(child, traversal.source) => 1,
            LanguageFamily::Java
            | LanguageFamily::CSharp
            | LanguageFamily::Kotlin
            | LanguageFamily::Php
            | LanguageFamily::Ruby
                if added_language_public_item(child, traversal) =>
            {
                1
            }
            _ => 0,
        };
    }
    count
}

fn added_language_public_item(node: Node<'_>, traversal: StructureTraversal<'_>) -> bool {
    match traversal.family {
        LanguageFamily::Java | LanguageFamily::CSharp => {
            has_public_modifier(node, traversal.source)
        }
        LanguageFamily::Kotlin => kotlin_public_item(node, traversal.source),
        LanguageFamily::Php => php_public_item(node, traversal.source),
        LanguageFamily::Ruby => ruby_public_item(node),
        _ => false,
    }
}

fn rust_public_item(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        FUNCTION_ITEM
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "type_item"
            | "const_item"
            | "static_item"
            | "mod_item"
            | "use_declaration"
    ) && has_rust_visibility_modifier(node)
}

fn has_rust_visibility_modifier(node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| child.kind() == "visibility_modifier")
}
