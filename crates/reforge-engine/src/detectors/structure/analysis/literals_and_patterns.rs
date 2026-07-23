pub(super) fn is_literal_node(node: Node<'_>) -> bool {
    let kind = node.kind();
    kind.contains("string")
        || kind.contains("number")
        || kind.contains("integer")
        || kind.contains("float")
        || matches!(kind, "raw_string_literal" | "interpreted_string_literal")
}

pub(super) fn has_literal_ancestor(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if is_literal_node(parent) {
            return true;
        }
        node = parent;
    }

    false
}

pub(super) fn has_repeated_literal_noise_ancestor(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if is_import_or_export_node(parent) {
            return true;
        }
        node = parent;
    }

    false
}

fn is_import_or_export_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "import_statement" | "import_declaration" | "export_statement" | "export_declaration"
    )
}

pub(super) fn normalize_literal(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.chars().all(|character| character.is_ascii_digit()) {
        return if trimmed.len() < 3 {
            None
        } else {
            Some(trimmed.to_string())
        };
    }

    let unquoted = trimmed
        .trim_start_matches(['r', 'b', 'f', 'u'])
        .trim_matches(['"', '\'', '`']);
    if unquoted.len() < 5
        || is_ignored_repeated_literal(unquoted)
        || is_module_specifier_literal(unquoted)
    {
        None
    } else {
        Some(unquoted.to_string())
    }
}

pub(super) fn is_ignored_repeated_literal(literal: &str) -> bool {
    let normalized = literal.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "line"
            | "lines"
            | "occurrence"
            | "occurrences"
            | "reference"
            | "references"
            | "group"
            | "group_size"
            | "boolean"
            | "number"
            | "object"
            | "string"
            | "symbol"
            | "unknown"
            | "undefined"
            | "bigint"
            | "score"
            | "confidence"
            | "severity"
            | "threshold"
            | "metrics"
            | "metric"
            | "method"
            | "request"
            | "response"
            | "status"
            | "medium"
            | "strong"
            | "utf-8"
            | "utf8"
            | "value"
            | "callee"
            | "content"
            | "filename"
            | "provider_prefix"
            | "mod_item"
            | "imports"
            | "concept"
            | "concepts"
            | "function"
            | "functions"
            | "parameter"
            | "parameters"
            | "schema_version"
            | "related_locations"
            | "snake_case"
            | "source files"
            | "source file"
            | "detections"
            | "detection"
    )
}

fn is_module_specifier_literal(literal: &str) -> bool {
    let trimmed = literal.trim();
    if trimmed.contains(char::is_whitespace) {
        return false;
    }

    let looks_relative = trimmed.starts_with("./") || trimmed.starts_with("../");
    let looks_scoped_package = trimmed.starts_with('@') && trimmed.contains('/');
    let has_source_extension = [
        ".c", ".cc", ".cpp", ".cs", ".go", ".java", ".js", ".jsx", ".kt", ".py", ".rb", ".rs",
        ".ts", ".tsx",
    ]
    .iter()
    .any(|extension| trimmed.ends_with(extension));

    looks_relative || looks_scoped_package || has_source_extension
}

pub(super) fn is_error_pattern_node(node: Node<'_>, traversal: StructureTraversal<'_>) -> bool {
    let kind = node.kind();
    match traversal.family {
        LanguageFamily::JavaScriptTypeScript => kind == "catch_clause",
        LanguageFamily::Python => kind == "except_clause",
        LanguageFamily::Go if kind == "if_statement" => node
            .utf8_text(traversal.source.as_bytes())
            .ok()
            .is_some_and(|text| {
                text.contains("err") && text.contains("!=") && text.contains("nil")
            }),
        LanguageFamily::Rust => {
            kind == "match_arm"
                && node
                    .utf8_text(traversal.source.as_bytes())
                    .ok()
                    .is_some_and(|text| text.contains("Err"))
        }
        _ => false,
    }
}

pub(super) fn collect_test_setup_patterns(
    file: &SourceFile,
    node: Node<'_>,
    signals: &mut FileSignals,
) {
    walk_file_nodes(file, node, signals, collect_test_setup_occurrence);
}

pub(super) fn collect_test_setup_occurrence(
    file: &SourceFile,
    node: Node<'_>,
    signals: &mut FileSignals,
) {
    if matches!(node.kind(), "call_expression" | "call")
        && let Ok(text) = node.utf8_text(file.source.as_bytes())
    {
        let normalized = normalize_pattern(text);
        if is_setup_pattern(&normalized) {
            signals
                .test_setups
                .push((normalized, occurrence(file, node, None)));
        }
    }
}

pub(super) fn walk_file_nodes(
    file: &SourceFile,
    node: Node<'_>,
    signals: &mut FileSignals,
    visit: fn(&SourceFile, Node<'_>, &mut FileSignals),
) {
    visit(file, node, signals);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_file_nodes(file, child, signals, visit);
    }
}

pub(super) fn occurrence(file: &SourceFile, node: Node<'_>, name: Option<String>) -> Occurrence {
    Occurrence {
        path: file.display_path.clone(),
        line: node.start_position().row + 1,
        name,
    }
}

pub(super) fn is_setup_pattern(pattern: &str) -> bool {
    pattern.contains("setup")
        || pattern.contains("fixture")
        || pattern.contains("mock")
        || pattern.contains("fake")
        || pattern.contains("before_each")
        || pattern.contains("beforeeach")
        || pattern.contains("beforeall")
}
