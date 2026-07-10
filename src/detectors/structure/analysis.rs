use super::*;

mod aggregation;

pub(super) use aggregation::*;

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

pub(super) fn count_public_items(root: Node<'_>, traversal: StructureTraversal<'_>) -> usize {
    let mut count = 0;
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if should_skip_rust_test_module(child, traversal) {
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

pub(super) fn rust_public_item(node: Node<'_>) -> bool {
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
            | "findings"
            | "finding"
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

pub(super) fn repeated_literal_confidence(literal: &str, locations: &[Occurrence]) -> f64 {
    let normalized = literal.to_ascii_lowercase();
    let weak_text = contains_report_or_fixture_text(&normalized)
        || locations
            .iter()
            .all(|location| is_test_source(Path::new(&location.path)));

    if weak_text {
        0.55
    } else if crosses_files(locations) {
        0.80
    } else {
        0.65
    }
}

pub(super) fn contains_report_or_fixture_text(literal: &str) -> bool {
    [
        "report", "schema", "fixture", "mock", "snapshot", "expected", "actual", "should", "test ",
    ]
    .iter()
    .any(|word| literal.contains(word))
}

pub(super) fn crosses_files(locations: &[Occurrence]) -> bool {
    locations
        .iter()
        .map(|location| location.path.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        > 1
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

pub(super) fn collect_happy_path_test_risk(
    file: &SourceFile,
    family: LanguageFamily,
    signals: &mut FileSignals,
) {
    let test_cases = test_case_occurrences(file, family);
    if test_cases.len() < 3 {
        return;
    }

    if has_assertion_evidence(&file.source) && !has_negative_or_boundary_test_evidence(&file.source)
    {
        signals
            .happy_path_test_files
            .push((test_cases.len(), test_cases));
    }
}

pub(super) fn happy_path_test_findings(test_files: Vec<(usize, Vec<Occurrence>)>) -> Vec<Finding> {
    test_files
        .into_iter()
        .filter_map(|(test_count, locations)| {
            let representative = locations.first()?;
            Some(crate::scanner::finding(
                FindingInput::new(
                    FindingKind::HappyPathOnlyTests,
                    representative.path.clone(),
                    Some(representative.line),
                    format!(
                        "test file has {test_count} cases but no negative, error, or boundary assertions were detected"
                    ),
                    vec![FindingMetric::threshold(
                        crate::model::MetricId::GroupSize,
                        test_count,
                        3,
                        "test cases",
                    )],
                )
                .with_related_locations(locations),
            ))
        })
        .collect()
}

pub(super) fn test_case_occurrences(file: &SourceFile, family: LanguageFamily) -> Vec<Occurrence> {
    file.source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim_start();
            if is_test_case_line(trimmed, family) {
                Some(Occurrence {
                    path: file.display_path.clone(),
                    line: index + 1,
                    name: test_case_name(trimmed, family),
                })
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn is_test_case_line(line: &str, family: LanguageFamily) -> bool {
    match family {
        LanguageFamily::Rust => {
            line.starts_with("#[test]")
                || line.starts_with("#[tokio::test")
                || line.starts_with("#[async_std::test")
        }
        LanguageFamily::JavaScriptTypeScript => {
            line.starts_with("test(")
                || line.starts_with("it(")
                || line.starts_with("test.each")
                || line.starts_with("it.each")
        }
        LanguageFamily::Python => {
            line.starts_with("def test_") || line.starts_with("async def test_")
        }
        LanguageFamily::Go => line.starts_with("func Test"),
        LanguageFamily::Java | LanguageFamily::CSharp | LanguageFamily::Kotlin => {
            line.starts_with("@Test")
                || line.starts_with("[Test")
                || line.starts_with("[Fact")
                || line.starts_with("[Theory")
        }
        LanguageFamily::Php => {
            line.starts_with("public function test") || line.starts_with("function test")
        }
        LanguageFamily::Ruby => line.starts_with("def test_") || line.starts_with("it "),
    }
}

pub(super) fn test_case_name(line: &str, family: LanguageFamily) -> Option<String> {
    match family {
        LanguageFamily::Rust => Some("test attribute".to_string()),
        LanguageFamily::JavaScriptTypeScript => quoted_test_name(line),
        LanguageFamily::Python
        | LanguageFamily::Go
        | LanguageFamily::Java
        | LanguageFamily::CSharp
        | LanguageFamily::Kotlin
        | LanguageFamily::Php
        | LanguageFamily::Ruby => line
            .split(['(', '{'])
            .next()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(ToString::to_string),
    }
}

pub(super) fn quoted_test_name(line: &str) -> Option<String> {
    let quote_index = line.find(['"', '\'', '`'])?;
    let quote = line[quote_index..].chars().next()?;
    let rest = &line[quote_index + quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

pub(super) fn has_assertion_evidence(source: &str) -> bool {
    let normalized = source.to_ascii_lowercase();
    [
        "expect(", "assert", "should", "t.error", "t.fatal", "require.", "pytest.",
    ]
    .iter()
    .any(|pattern| normalized.contains(pattern))
}

pub(super) fn has_negative_or_boundary_test_evidence(source: &str) -> bool {
    let normalized = source.to_ascii_lowercase();
    [
        "tothrow",
        "to_throw",
        ".rejects",
        "raises(",
        "pytest.raises",
        "should_panic",
        "is_err",
        "unwrap_err",
        "expect_err",
        " err == nil",
        "err == nil",
        " err != nil",
        "err != nil",
        "invalid",
        "missing",
        "empty",
        "none",
        "null",
        "nil",
        "zero",
        "negative",
        "unauthorized",
        "forbidden",
        "not found",
        "not_found",
        "error",
        "failure",
        "panic",
        "duplicate",
        "overflow",
        "underflow",
        "timeout",
        "denied",
        "boundary",
        "edge",
        "does not",
        "doesn't",
        "without",
        "ignore",
        "ignores",
        "ignored",
        "skip",
        "skips",
        "skipped",
        "caps",
        "prevents",
    ]
    .iter()
    .any(|pattern| normalized.contains(pattern))
}
