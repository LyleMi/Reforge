use super::*;

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
            _ => {}
        }
    }
    count
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
            _ => 0,
        };
    }
    count
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
    ) && node.child_by_field_name("visibility").is_some()
}

pub(super) fn should_skip_rust_test_module(
    node: Node<'_>,
    traversal: StructureTraversal<'_>,
) -> bool {
    traversal.family == LanguageFamily::Rust
        && !traversal.include_test_structure
        && node.kind() == "mod_item"
        && has_cfg_test_attribute(node, traversal.source)
}

pub(super) fn has_cfg_test_attribute(node: Node<'_>, source: &str) -> bool {
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

pub(super) fn is_cfg_test_attribute(attribute: &str) -> bool {
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

impl StructureSignalCollector<'_, '_> {
    pub(super) fn collect_literal_occurrence(&mut self, node: Node<'_>) {
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
    if unquoted.len() < 5 || is_ignored_repeated_literal(unquoted) {
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
            | "group"
            | "group_size"
            | "score"
            | "confidence"
            | "severity"
            | "threshold"
            | "metrics"
            | "metric"
            | "imports"
            | "concept"
            | "concepts"
            | "function"
            | "functions"
            | "parameter"
            | "parameters"
            | "schema_version"
            | "related_locations"
            | "source files"
            | "source file"
            | "findings"
            | "finding"
    )
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
                FindingKind::HappyPathOnlyTests,
                representative.path.clone(),
                Some(representative.line),
                format!(
                    "test file has {test_count} cases but no negative, error, or boundary assertions were detected"
                ),
                vec![FindingMetric::threshold("group_size", test_count, 3, "test cases")],
                locations,
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
    }
}

pub(super) fn test_case_name(line: &str, family: LanguageFamily) -> Option<String> {
    match family {
        LanguageFamily::Rust => Some("test attribute".to_string()),
        LanguageFamily::JavaScriptTypeScript => quoted_test_name(line),
        LanguageFamily::Python | LanguageFamily::Go => line
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
    ]
    .iter()
    .any(|pattern| normalized.contains(pattern))
}

pub(super) fn collect_file_naming_style(file: &SourceFile, signals: &mut FileSignals) {
    let Some(parent) = file.path.parent() else {
        return;
    };

    let Some(stem) = normalized_naming_stem(&file.path) else {
        return;
    };

    let Some(style) = classify_file_naming_style(&stem) else {
        return;
    };

    let entry = signals
        .naming_directories
        .entry(parent.to_path_buf())
        .or_insert_with(|| NamingDirectory {
            display_path: parent.to_string_lossy().replace('\\', "/"),
            styles: BTreeMap::new(),
        });
    entry.styles.entry(style).or_default().push(Occurrence {
        path: file.display_path.clone(),
        line: 1,
        name: Some(stem),
    });
}

pub(super) fn file_naming_drift_findings(
    directories: &BTreeMap<PathBuf, NamingDirectory>,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for directory in directories.values() {
        let styles = effective_naming_styles(directory);
        let total_files = styles.values().map(Vec::len).sum::<usize>();
        if total_files < 4 || styles.len() < 2 {
            continue;
        }

        let dominant = styles.iter().max_by_key(|(_, locations)| locations.len());
        let Some((dominant_style, dominant_locations)) = dominant else {
            continue;
        };

        let related_locations = naming_drift_locations(&styles, *dominant_style);
        if related_locations.is_empty() {
            continue;
        }

        findings.push(crate::scanner::finding(
            FindingKind::FileNamingDrift,
            directory.display_path.clone(),
            None,
            format!(
                "directory uses {} file naming styles across {total_files} files; dominant style is {} with {} files",
                styles.len(),
                dominant_style.label(),
                dominant_locations.len()
            ),
            vec![FindingMetric::threshold(
                "group_size",
                styles.len(),
                2,
                "naming styles",
            )],
            related_locations,
        ));
    }

    findings
}

pub(super) fn effective_naming_styles(
    directory: &NamingDirectory,
) -> BTreeMap<FileNamingStyle, Vec<Occurrence>> {
    let non_neutral = directory
        .styles
        .iter()
        .filter(|(style, _)| **style != FileNamingStyle::Lowercase)
        .map(|(style, locations)| (*style, locations.clone()))
        .collect::<BTreeMap<_, _>>();

    if non_neutral.is_empty() {
        directory.styles.clone()
    } else {
        non_neutral
    }
}

pub(super) fn naming_drift_locations(
    styles: &BTreeMap<FileNamingStyle, Vec<Occurrence>>,
    dominant_style: FileNamingStyle,
) -> Vec<Occurrence> {
    let dominant_count = styles
        .get(&dominant_style)
        .map(Vec::len)
        .unwrap_or_default();
    let total_files = styles.values().map(Vec::len).sum::<usize>();
    let has_clear_dominant = dominant_count >= 2 && dominant_count * 2 >= total_files;

    styles
        .iter()
        .filter(|(style, _)| !has_clear_dominant || **style != dominant_style)
        .flat_map(|(style, locations)| {
            locations.iter().map(|location| Occurrence {
                name: location
                    .name
                    .as_ref()
                    .map(|name| format!("{name} ({})", style.label())),
                ..location.clone()
            })
        })
        .collect()
}

pub(super) fn normalized_naming_stem(path: &Path) -> Option<String> {
    let mut stem = path.file_stem()?.to_str()?.to_string();

    while let Some(stripped) = test_file_suffix_base(&stem) {
        stem = stripped.to_string();
    }

    if stem.is_empty()
        || matches!(
            stem.as_str(),
            "mod" | "lib" | "main" | "index" | "__init__" | "package"
        )
    {
        None
    } else {
        Some(stem)
    }
}

pub(super) fn test_file_suffix_base(stem: &str) -> Option<&str> {
    stem.strip_suffix(".test")
        .or_else(|| stem.strip_suffix(".spec"))
        .or_else(|| stem.strip_suffix("_test"))
        .or_else(|| stem.strip_suffix("_tests"))
        .or_else(|| stem.strip_suffix("-test"))
        .or_else(|| stem.strip_suffix("-spec"))
}

pub(super) fn classify_file_naming_style(stem: &str) -> Option<FileNamingStyle> {
    if !stem
        .chars()
        .any(|character| character.is_ascii_alphabetic())
    {
        return None;
    }

    let has_underscore = stem.contains('_');
    let has_dash = stem.contains('-');
    let has_dot = stem.contains('.');
    let separator_count = [has_underscore, has_dash, has_dot]
        .into_iter()
        .filter(|has_separator| *has_separator)
        .count();
    if separator_count > 1 {
        return Some(FileNamingStyle::Mixed);
    }

    if has_underscore {
        return Some(if separated_words_are_lowercase(stem, '_') {
            FileNamingStyle::SnakeCase
        } else {
            FileNamingStyle::Mixed
        });
    }

    if has_dash {
        return Some(if separated_words_are_lowercase(stem, '-') {
            FileNamingStyle::KebabCase
        } else {
            FileNamingStyle::Mixed
        });
    }

    if has_dot {
        return Some(if separated_words_are_lowercase(stem, '.') {
            FileNamingStyle::DotSeparated
        } else {
            FileNamingStyle::Mixed
        });
    }

    let first = stem.chars().next()?;
    let has_uppercase = stem.chars().any(|character| character.is_ascii_uppercase());
    let has_lowercase = stem.chars().any(|character| character.is_ascii_lowercase());

    if first.is_ascii_uppercase() && has_lowercase {
        Some(FileNamingStyle::PascalCase)
    } else if first.is_ascii_lowercase() && has_uppercase {
        Some(FileNamingStyle::CamelCase)
    } else if stem
        .chars()
        .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
    {
        Some(FileNamingStyle::Lowercase)
    } else {
        Some(FileNamingStyle::Mixed)
    }
}

pub(super) fn separated_words_are_lowercase(stem: &str, separator: char) -> bool {
    stem.split(separator).all(|part| {
        !part.is_empty()
            && part
                .chars()
                .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
    })
}

impl FileNamingStyle {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::SnakeCase => "snake_case",
            Self::KebabCase => "kebab-case",
            Self::PascalCase => "PascalCase",
            Self::CamelCase => "camelCase",
            Self::Lowercase => "lowercase",
            Self::DotSeparated => "dot.separated",
            Self::Mixed => "mixed",
        }
    }
}

pub(super) fn collect_directory_concepts(
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

pub(super) fn directory_drift_findings(
    directories: &BTreeMap<PathBuf, BTreeSet<String>>,
    options: &StructureOptions,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (directory, concepts) in directories {
        let threshold = options.max_dir_files.max(4);
        if concepts.len() > threshold {
            findings.push(crate::scanner::finding(
                FindingKind::DirectoryDrift,
                directory.to_string_lossy().replace('\\', "/"),
                None,
                format!(
                    "directory mixes {} naming/language concepts; consider grouping cohesive responsibilities",
                    concepts.len()
                ),
                vec![FindingMetric::threshold(
                    "group_size",
                    concepts.len(),
                    threshold,
                    "concepts",
                )],
                Vec::new(),
            ));
        }
    }
    findings
}

pub(super) fn group_occurrences(
    occurrences: Vec<(String, Occurrence)>,
    min_occurrences: usize,
    kind: FindingKind,
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
        let related_locations = group
            .iter()
            .map(|occurrence| RelatedLocation {
                path: occurrence.path.clone(),
                line: occurrence.line,
                name: occurrence.name.clone(),
            })
            .collect::<Vec<_>>();
        let metrics = vec![FindingMetric::threshold(
            "group_size",
            group.len(),
            min_occurrences,
            "occurrences",
        )];
        let finding = if kind == FindingKind::RepeatedLiteral {
            crate::scanner::scored_finding(
                kind,
                representative.path.clone(),
                Some(representative.line),
                message(&key, group.len()),
                metrics,
                repeated_literal_confidence(&key, &group),
                related_locations,
            )
        } else {
            crate::scanner::finding(
                kind,
                representative.path.clone(),
                Some(representative.line),
                message(&key, group.len()),
                metrics,
                related_locations,
            )
        };
        findings.push(finding);
    }

    findings
}

pub(super) fn count_named_descendants(node: Node<'_>, kind: &str) -> usize {
    let mut count = usize::from(node.kind() == kind);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        count += count_named_descendants(child, kind);
    }
    count
}

pub(super) fn node_line_span(node: Node<'_>) -> usize {
    node.end_position()
        .row
        .saturating_sub(node.start_position().row)
        + 1
}

pub(super) fn normalize_identifier(text: &str) -> String {
    text.trim_matches(|character: char| !character.is_alphanumeric() && character != '_')
        .to_ascii_lowercase()
}

pub(super) fn normalize_pattern(text: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_space = false;
    for character in text.chars() {
        if let Some(character) = normalized_pattern_char(character, &mut previous_was_space) {
            normalized.push(character);
        }
    }
    normalized.trim().to_string()
}

pub(super) fn normalized_pattern_char(
    character: char,
    previous_was_space: &mut bool,
) -> Option<char> {
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

pub(super) fn split_directory_concept_words(text: &str) -> Vec<String> {
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
