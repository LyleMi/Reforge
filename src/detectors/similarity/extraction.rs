fn extract_candidates_from_parsed(
    file: &ParsedSourceFile,
    min_tokens: usize,
    include_test_similarity: bool,
    interner: &mut TokenInterner,
) -> Vec<FunctionCandidate> {
    let extraction = CandidateExtraction {
        source: &file.file.source,
        file: &file.file,
        family: file.family,
        min_tokens,
        include_test_similarity,
    };
    let mut candidates = Vec::new();
    collect_named_functions(file.tree.root_node(), extraction, interner, &mut candidates);
    candidates
}

fn collect_named_functions(
    node: Node<'_>,
    extraction: CandidateExtraction<'_>,
    interner: &mut TokenInterner,
    candidates: &mut Vec<FunctionCandidate>,
) {
    if should_skip_rust_test_module(node, extraction) {
        return;
    }

    if let Some((name, category, body)) = extract_function_parts(node, extraction) {
        let tokens = normalize_tokens(body, extraction.source.as_bytes(), interner);
        if tokens.len() >= extraction.min_tokens {
            let token_counts = token_counts(&tokens);
            candidates.push(FunctionCandidate {
                family: extraction.family,
                category,
                name,
                path: extraction.file.display_path.clone(),
                line: node.start_position().row + 1,
                tokens,
                token_counts,
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_named_functions(child, extraction, interner, candidates);
    }
}

fn extract_function_parts<'tree>(
    node: Node<'tree>,
    extraction: CandidateExtraction<'_>,
) -> Option<(String, FunctionCategory, Node<'tree>)> {
    match extraction.family {
        LanguageFamily::Rust => rust_function_parts(node, extraction.source),
        LanguageFamily::JavaScriptTypeScript => javascript_function_parts(node, extraction.source),
        LanguageFamily::Python => python_function_parts(node, extraction.source),
        LanguageFamily::Go => go_function_parts(node, extraction.source),
        _ => extract_added_language_function_parts(node, extraction),
    }
}

fn rust_function_parts<'tree>(
    node: Node<'tree>,
    source: &str,
) -> Option<(String, FunctionCategory, Node<'tree>)> {
    if node.kind() != FUNCTION_ITEM {
        return None;
    }
    let category = if has_ancestor_kind(node, "impl_item") {
        FunctionCategory::Method
    } else {
        FunctionCategory::Function
    };
    named_parts(node, source, category)
}

fn javascript_function_parts<'tree>(
    node: Node<'tree>,
    source: &str,
) -> Option<(String, FunctionCategory, Node<'tree>)> {
    if !matches!(
        node.kind(),
        FUNCTION_DECLARATION | GENERATOR_FUNCTION_DECLARATION | METHOD_DEFINITION
    ) {
        return None;
    }
    let category = if node.kind() == METHOD_DEFINITION {
        FunctionCategory::Method
    } else {
        FunctionCategory::Function
    };
    named_parts(node, source, category)
}

fn python_function_parts<'tree>(
    node: Node<'tree>,
    source: &str,
) -> Option<(String, FunctionCategory, Node<'tree>)> {
    (node.kind() == FUNCTION_DEFINITION)
        .then(|| named_parts(node, source, FunctionCategory::Function))?
}

fn go_function_parts<'tree>(
    node: Node<'tree>,
    source: &str,
) -> Option<(String, FunctionCategory, Node<'tree>)> {
    if !matches!(node.kind(), FUNCTION_DECLARATION | METHOD_DECLARATION) {
        return None;
    }
    let category = if node.kind() == METHOD_DECLARATION {
        FunctionCategory::Method
    } else {
        FunctionCategory::Function
    };
    named_parts(node, source, category)
}

fn named_parts<'tree>(
    node: Node<'tree>,
    source: &str,
    category: FunctionCategory,
) -> Option<(String, FunctionCategory, Node<'tree>)> {
    let name = node
        .child_by_field_name(NAME_FIELD)?
        .utf8_text(source.as_bytes())
        .ok()?;
    let body = node.child_by_field_name(BODY_FIELD)?;
    Some((name.to_string(), category, body))
}

fn extract_added_language_function_parts<'tree>(
    node: Node<'tree>,
    extraction: CandidateExtraction<'_>,
) -> Option<(String, FunctionCategory, Node<'tree>)> {
    match extraction.family {
        LanguageFamily::Java | LanguageFamily::CSharp => {
            method_parts(node, extraction.source, extraction.family)
        }
        LanguageFamily::Kotlin => kotlin_function_parts(node, extraction.source),
        LanguageFamily::Php => php_function_parts(node, extraction.source),
        LanguageFamily::Ruby => ruby_method_parts(node, extraction.source),
        LanguageFamily::Bash => bash_function_parts(node, extraction.source),
        LanguageFamily::PowerShell => powershell_function_parts(node, extraction.source),
        _ => None,
    }
}

fn method_parts<'tree>(
    node: Node<'tree>,
    source: &str,
    family: LanguageFamily,
) -> Option<(String, FunctionCategory, Node<'tree>)> {
    let is_supported = node.kind() == METHOD_DECLARATION
        || (family == LanguageFamily::CSharp
            && matches!(
                node.kind(),
                "constructor_declaration" | "local_function_statement"
            ));
    if !is_supported {
        return None;
    }

    let name = node
        .child_by_field_name(NAME_FIELD)?
        .utf8_text(source.as_bytes())
        .ok()?;
    let body = node.child_by_field_name(BODY_FIELD)?;
    Some((name.to_string(), FunctionCategory::Method, body))
}

fn kotlin_function_parts<'tree>(
    node: Node<'tree>,
    source: &str,
) -> Option<(String, FunctionCategory, Node<'tree>)> {
    if node.kind() != FUNCTION_DECLARATION {
        return None;
    }

    let name = node
        .child_by_field_name(NAME_FIELD)?
        .utf8_text(source.as_bytes())
        .ok()?;
    let body = child_by_kind(node, "function_body")?;
    let category = if has_ancestor_kind(node, "class_declaration")
        || has_ancestor_kind(node, "object_declaration")
    {
        FunctionCategory::Method
    } else {
        FunctionCategory::Function
    };
    Some((name.to_string(), category, body))
}

fn php_function_parts<'tree>(
    node: Node<'tree>,
    source: &str,
) -> Option<(String, FunctionCategory, Node<'tree>)> {
    let kind = node.kind();
    if !matches!(kind, FUNCTION_DEFINITION | METHOD_DECLARATION) {
        return None;
    }

    let name = node
        .child_by_field_name(NAME_FIELD)?
        .utf8_text(source.as_bytes())
        .ok()?;
    let body = node.child_by_field_name(BODY_FIELD)?;
    let category = if kind == METHOD_DECLARATION {
        FunctionCategory::Method
    } else {
        FunctionCategory::Function
    };
    Some((name.to_string(), category, body))
}

fn ruby_method_parts<'tree>(
    node: Node<'tree>,
    source: &str,
) -> Option<(String, FunctionCategory, Node<'tree>)> {
    if !matches!(node.kind(), "method" | "singleton_method") {
        return None;
    }

    let name = node
        .child_by_field_name(NAME_FIELD)?
        .utf8_text(source.as_bytes())
        .ok()?;
    let body = node.child_by_field_name(BODY_FIELD)?;
    Some((name.to_string(), FunctionCategory::Method, body))
}

fn bash_function_parts<'tree>(
    node: Node<'tree>,
    source: &str,
) -> Option<(String, FunctionCategory, Node<'tree>)> {
    if node.kind() != FUNCTION_DEFINITION {
        return None;
    }
    named_parts(node, source, FunctionCategory::Function)
}

fn powershell_function_parts<'tree>(
    node: Node<'tree>,
    source: &str,
) -> Option<(String, FunctionCategory, Node<'tree>)> {
    if node.kind() != "function_statement" {
        return None;
    }
    let name = child_by_kind(node, "function_name")?
        .utf8_text(source.as_bytes())
        .ok()?;
    let body = child_by_kind(node, "script_block")?;
    Some((name.to_string(), FunctionCategory::Function, body))
}

fn has_ancestor_kind(mut node: Node<'_>, kind: &str) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return true;
        }
        node = parent;
    }

    false
}

fn should_skip_rust_test_module(node: Node<'_>, extraction: CandidateExtraction<'_>) -> bool {
    extraction.family == LanguageFamily::Rust
        && !extraction.include_test_similarity
        && node.kind() == "mod_item"
        && has_rust_cfg_test_attribute(node, extraction.source)
}

fn normalize_tokens(node: Node<'_>, source: &[u8], interner: &mut TokenInterner) -> Vec<TokenId> {
    let mut tokens = Vec::new();
    normalize_node(node, source, interner, &mut tokens);
    tokens
}

fn normalize_node(
    node: Node<'_>,
    source: &[u8],
    interner: &mut TokenInterner,
    tokens: &mut Vec<TokenId>,
) {
    let kind = node.kind();

    if is_comment_kind(kind) {
        return;
    }

    if let Some(token) = normalized_named_token(kind) {
        tokens.push(interner.intern(token));
        return;
    }

    if node.child_count() == 0 {
        if node.is_named() {
            tokens.push(interner.intern(kind));
        } else if let Ok(text) = node.utf8_text(source)
            && !text.trim().is_empty()
        {
            tokens.push(interner.intern(text));
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        normalize_node(child, source, interner, tokens);
    }
}

fn normalized_named_token(kind: &str) -> Option<&'static str> {
    if is_identifier_like_kind(kind) {
        Some("ID")
    } else if is_string_kind(kind) {
        Some("STR")
    } else if is_number_kind(kind) {
        Some("NUM")
    } else {
        None
    }
}

fn token_counts(tokens: &[TokenId]) -> Vec<(TokenId, usize)> {
    let mut counts = BTreeMap::new();
    for token in tokens {
        *counts.entry(*token).or_insert(0) += 1;
    }
    counts.into_iter().collect()
}

fn is_comment_kind(kind: &str) -> bool {
    kind.contains("comment")
}

fn is_string_kind(kind: &str) -> bool {
    kind.contains("string") || matches!(kind, "raw_string_literal" | "interpreted_string_literal")
}

fn is_number_kind(kind: &str) -> bool {
    kind.contains("number")
        || kind.contains("integer")
        || kind.contains("float")
        || kind == "imaginary_literal"
}
