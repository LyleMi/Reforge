fn type_metric(node: Node<'_>, traversal: StructureTraversal<'_>) -> Option<TypeMetric> {
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

fn count_type_members(node: Node<'_>, family: LanguageFamily) -> usize {
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

fn is_type_member(node: Node<'_>, family: LanguageFamily) -> bool {
    let kind = node.kind();
    match family {
        LanguageFamily::Rust => matches!(
            kind,
            "field_declaration" | "enum_variant" | FUNCTION_ITEM | "associated_type" | "const_item"
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
    }
}

fn count_imports(root: Node<'_>, family: LanguageFamily) -> usize {
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
            _ => {}
        }
    }
    count
}

fn count_public_items(root: Node<'_>, traversal: StructureTraversal<'_>) -> usize {
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
            _ => 0,
        };
    }
    count
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
    ) && node.child_by_field_name("visibility").is_some()
}

fn should_skip_rust_test_module(node: Node<'_>, traversal: StructureTraversal<'_>) -> bool {
    traversal.family == LanguageFamily::Rust
        && !traversal.include_test_structure
        && node.kind() == "mod_item"
        && has_cfg_test_attribute(node, traversal.source)
}

fn has_cfg_test_attribute(node: Node<'_>, source: &str) -> bool {
    let mut end = node.start_byte().min(source.len());
    let bytes = source.as_bytes();

    loop {
        while end > 0 && bytes[end - 1].is_ascii_whitespace() {
            end -= 1;
        }

        if end == 0 || bytes[end - 1] != b']' {
            return false;
        }

        let Some(start) = source[..end].rfind("#[") else {
            return false;
        };
        let attribute = &source[start..end];
        if is_cfg_test_attribute(attribute) {
            return true;
        }

        end = start;
    }
}

fn is_cfg_test_attribute(attribute: &str) -> bool {
    let compact = attribute
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let Some(inner) = compact
        .strip_prefix("#[cfg(")
        .and_then(|value| value.strip_suffix(")]"))
    else {
        return false;
    };

    inner == "test"
        || inner.starts_with("any(test")
        || inner.starts_with("all(test")
        || inner.contains("(test,")
        || inner.contains(",test,")
        || inner.ends_with(",test")
        || inner.ends_with(",test)")
}

fn python_public_item(node: Node<'_>, source: &str) -> bool {
    if !matches!(node.kind(), FUNCTION_DEFINITION | "class_definition") {
        return false;
    }

    node.child_by_field_name("name")
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .is_some_and(|name| !name.starts_with('_'))
}

fn go_public_item(node: Node<'_>, source: &str) -> bool {
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

fn is_exported_go_identifier(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|character| character.is_uppercase())
}

impl StructureSignalCollector<'_, '_> {
    fn collect_repeated_literals(&mut self, node: Node<'_>) {
        if self.should_skip(node) {
            return;
        }

        self.collect_literal_occurrence(node);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_repeated_literals(child);
        }
    }

    fn collect_literal_occurrence(&mut self, node: Node<'_>) {
        if is_literal_node(node)
            && !has_literal_ancestor(node)
            && let Ok(text) = node.utf8_text(self.traversal.source.as_bytes())
            && let Some(literal) = normalize_literal(text)
        {
            self.signals
                .literals
                .push((literal, occurrence(self.file, node, None)));
        }
    }

    fn collect_error_patterns(&mut self, node: Node<'_>) {
        if self.should_skip(node) {
            return;
        }

        self.collect_error_occurrence(node);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_error_patterns(child);
        }
    }

    fn collect_error_occurrence(&mut self, node: Node<'_>) {
        if is_error_pattern_node(node, self.traversal)
            && let Ok(text) = node.utf8_text(self.traversal.source.as_bytes())
        {
            self.signals
                .error_patterns
                .push((normalize_pattern(text), occurrence(self.file, node, None)));
        }
    }

    fn should_skip(&self, node: Node<'_>) -> bool {
        should_skip_rust_test_module(node, self.traversal)
    }
}

fn is_literal_node(node: Node<'_>) -> bool {
    let kind = node.kind();
    kind.contains("string")
        || kind.contains("number")
        || kind.contains("integer")
        || kind.contains("float")
        || matches!(kind, "raw_string_literal" | "interpreted_string_literal")
}

fn has_literal_ancestor(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if is_literal_node(parent) {
            return true;
        }
        node = parent;
    }

    false
}

fn normalize_literal(text: &str) -> Option<String> {
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
    if unquoted.len() < 5 {
        None
    } else {
        Some(unquoted.to_string())
    }
}

fn is_error_pattern_node(node: Node<'_>, traversal: StructureTraversal<'_>) -> bool {
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

fn collect_test_setup_patterns(file: &SourceFile, node: Node<'_>, signals: &mut FileSignals) {
    walk_file_nodes(file, node, signals, collect_test_setup_occurrence);
}

fn collect_test_setup_occurrence(file: &SourceFile, node: Node<'_>, signals: &mut FileSignals) {
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

fn walk_file_nodes(
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

fn occurrence(file: &SourceFile, node: Node<'_>, name: Option<String>) -> Occurrence {
    Occurrence {
        path: file.display_path.clone(),
        line: node.start_position().row + 1,
        name,
    }
}

fn is_setup_pattern(pattern: &str) -> bool {
    pattern.contains("setup")
        || pattern.contains("fixture")
        || pattern.contains("mock")
        || pattern.contains("fake")
        || pattern.contains("before_each")
        || pattern.contains("beforeeach")
        || pattern.contains("beforeall")
}

fn collect_directory_concepts(
    file: &SourceFile,
    family: LanguageFamily,
    signals: &mut FileSignals,
) {
    let Some(parent) = file.path.parent() else {
        return;
    };

    let Some(stem) = file.path.file_stem().and_then(|stem| stem.to_str()) else {
        return;
    };

    let mut concepts = split_directory_concept_words(stem);
    concepts.push(format!("{family:?}").to_ascii_lowercase());
    let entry = signals
        .directory_files
        .entry(parent.to_path_buf())
        .or_default();
    for concept in concepts {
        entry.insert(concept);
    }
}

fn directory_drift_findings(
    directories: &BTreeMap<PathBuf, BTreeSet<String>>,
    options: &StructureOptions,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (directory, concepts) in directories {
        let threshold = options.max_dir_files.max(4);
        if concepts.len() > threshold {
            findings.push(Finding {
                kind: FindingKind::DirectoryDrift,
                severity: Severity::Info,
                path: directory.to_string_lossy().replace('\\', "/"),
                line: None,
                magnitude: Some(concepts.len()),
                message: format!(
                    "directory mixes {} naming/language concepts; consider grouping cohesive responsibilities",
                    concepts.len()
                ),
                related_locations: Vec::new(),
            });
        }
    }
    findings
}

fn group_occurrences(
    occurrences: Vec<(String, Occurrence)>,
    min_occurrences: usize,
    kind: FindingKind,
    severity: Severity,
    message: impl Fn(&str, usize) -> String,
) -> Vec<Finding> {
    if min_occurrences == 0 {
        return Vec::new();
    }

    let mut by_key: BTreeMap<String, Vec<Occurrence>> = BTreeMap::new();
    for (key, occurrence) in occurrences {
        by_key.entry(key).or_default().push(occurrence);
    }

    let mut findings = Vec::new();
    for (key, mut group) in by_key {
        group.sort_by(|left, right| left.path.cmp(&right.path).then(left.line.cmp(&right.line)));
        if group.len() < min_occurrences {
            continue;
        }

        let representative = &group[0];
        findings.push(Finding {
            kind,
            severity,
            path: representative.path.clone(),
            line: Some(representative.line),
            magnitude: Some(group.len()),
            message: message(&key, group.len()),
            related_locations: group
                .iter()
                .map(|occurrence| RelatedLocation {
                    path: occurrence.path.clone(),
                    line: occurrence.line,
                    name: occurrence.name.clone(),
                })
                .collect(),
        });
    }

    findings
}

fn count_named_descendants(node: Node<'_>, kind: &str) -> usize {
    let mut count = usize::from(node.kind() == kind);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        count += count_named_descendants(child, kind);
    }
    count
}

fn node_line_span(node: Node<'_>) -> usize {
    node.end_position()
        .row
        .saturating_sub(node.start_position().row)
        + 1
}

fn normalize_identifier(text: &str) -> String {
    text.trim_matches(|character: char| !character.is_alphanumeric() && character != '_')
        .to_ascii_lowercase()
}

fn normalize_pattern(text: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_space = false;
    for character in text.chars() {
        if let Some(character) = normalized_pattern_char(character, &mut previous_was_space) {
            normalized.push(character);
        }
    }
    normalized.trim().to_string()
}

fn normalized_pattern_char(character: char, previous_was_space: &mut bool) -> Option<char> {
    if character.is_ascii_digit() {
        return Some('#');
    }

    if matches!(character, '"' | '\'' | '`') {
        return Some('"');
    }

    if !character.is_whitespace() {
        *previous_was_space = false;
        return Some(character.to_ascii_lowercase());
    }

    if *previous_was_space {
        None
    } else {
        *previous_was_space = true;
        Some(' ')
    }
}

fn split_directory_concept_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if character == '_' || character == '-' || character == '.' {
            if !current.is_empty() {
                words.push(current.to_ascii_lowercase());
                current.clear();
            }
        } else if character.is_uppercase() && !current.is_empty() {
            words.push(current.to_ascii_lowercase());
            current.clear();
            current.push(character);
        } else if character.is_alphanumeric() {
            current.push(character);
        }
    }

    if !current.is_empty() {
        words.push(current.to_ascii_lowercase());
    }

    words
        .into_iter()
        .filter(|word| word.len() > 2 && !matches!(word.as_str(), "mod" | "lib" | "main" | "test"))
        .collect()
}

#[cfg(test)]
#[path = "structural_tests.rs"]
mod tests;
