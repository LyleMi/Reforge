#[derive(Default)]
struct CSharpTypeIndex {
    by_namespace: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    by_qualified_name: BTreeMap<String, Vec<String>>,
}

fn csharp_type_index(sources: &[SourceFile]) -> CSharpTypeIndex {
    let mut index = CSharpTypeIndex::default();
    for source in sources
        .iter()
        .filter(|source| Language::for_path(&source.path) == Language::CSharp)
    {
        let code = csharp_code_only(&source.source);
        let namespaces = csharp_declared_namespaces(&code);
        let namespace = namespaces.first().cloned().unwrap_or_default();
        for type_name in csharp_declared_types(&code) {
            let paths = index
                .by_namespace
                .entry(namespace.clone())
                .or_default()
                .entry(type_name.clone())
                .or_default();
            if !paths.contains(&source.display_path) {
                paths.push(source.display_path.clone());
            }
            let qualified = if namespace.is_empty() {
                type_name
            } else {
                format!("{namespace}.{type_name}")
            };
            let paths = index.by_qualified_name.entry(qualified).or_default();
            if !paths.contains(&source.display_path) {
                paths.push(source.display_path.clone());
            }
        }
    }
    index
}

fn csharp_declared_types(source: &str) -> Vec<String> {
    let tokens = csharp_identifiers(source);
    tokens
        .windows(2)
        .filter(|window| {
            matches!(
                window[0].as_str(),
                "class" | "struct" | "interface" | "enum" | "record"
            )
        })
        .map(|window| window[1].clone())
        .collect()
}

fn csharp_identifiers(source: &str) -> Vec<String> {
    source
        .split(|character: char| !(character == '_' || character.is_ascii_alphanumeric()))
        .filter(|value| {
            !value.is_empty()
                && value
                    .chars()
                    .next()
                    .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        })
        .map(str::to_string)
        .collect()
}

fn resolve_csharp_dependencies(source: &SourceFile, index: &CSharpTypeIndex) -> Vec<String> {
    let code = csharp_code_only(&source.source);
    let declared_namespaces = csharp_declared_namespaces(&code);
    let identifiers = csharp_identifiers(&code)
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let (imported_namespaces, aliases) = csharp_imports(&code);
    let mut targets = std::collections::BTreeSet::new();
    add_namespace_targets(
        declared_namespaces.iter().chain(imported_namespaces.iter()),
        &identifiers,
        index,
        &mut targets,
    );
    add_alias_targets(aliases, &identifiers, index, &mut targets);
    for (qualified, paths) in &index.by_qualified_name {
        if code.contains(qualified) {
            targets.extend(paths.iter().cloned());
        }
    }
    targets.into_iter().collect()
}

fn csharp_imports(code: &str) -> (Vec<String>, Vec<(String, String)>) {
    let mut namespaces = Vec::new();
    let mut aliases = Vec::new();
    for line in code.lines() {
        let trimmed = line.trim();
        let Some(specifier) = csharp_import_specifier(trimmed) else {
            continue;
        };
        match trimmed
            .strip_prefix("using ")
            .and_then(|value| value.split_once('='))
        {
            Some((left, _)) => aliases.push((left.trim().to_string(), specifier)),
            None => namespaces.push(specifier),
        }
    }
    (namespaces, aliases)
}

fn add_namespace_targets<'a>(
    namespaces: impl Iterator<Item = &'a String>,
    identifiers: &std::collections::BTreeSet<String>,
    index: &CSharpTypeIndex,
    targets: &mut std::collections::BTreeSet<String>,
) {
    for namespace in namespaces {
        let Some(types) = index.by_namespace.get(namespace) else {
            continue;
        };
        for (type_name, paths) in types {
            if identifiers.contains(type_name) {
                targets.extend(paths.iter().cloned());
            }
        }
    }
}

fn add_alias_targets(
    aliases: Vec<(String, String)>,
    identifiers: &std::collections::BTreeSet<String>,
    index: &CSharpTypeIndex,
    targets: &mut std::collections::BTreeSet<String>,
) {
    for (alias, qualified) in aliases {
        if !identifiers.contains(&alias) {
            continue;
        }
        if let Some(paths) = index.by_qualified_name.get(&qualified) {
            targets.extend(paths.iter().cloned());
        }
    }
}

#[derive(Clone, Copy)]
enum CSharpLexState {
    Code,
    LineComment,
    BlockComment,
    String,
    Character,
}

fn csharp_code_only(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut state = CSharpLexState::Code;
    while let Some(character) = chars.next() {
        state = mask_csharp_character(state, character, &mut chars, &mut output);
    }
    output
}

fn mask_csharp_character(
    state: CSharpLexState,
    character: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    output: &mut String,
) -> CSharpLexState {
    match state {
        CSharpLexState::Code => mask_csharp_code(character, chars, output),
        CSharpLexState::LineComment => mask_csharp_line_comment(character, output),
        CSharpLexState::BlockComment => mask_csharp_block_comment(character, chars, output),
        CSharpLexState::String => mask_csharp_quoted(character, '"', chars, output),
        CSharpLexState::Character => mask_csharp_quoted(character, '\'', chars, output),
    }
}

fn mask_csharp_code(
    character: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    output: &mut String,
) -> CSharpLexState {
    let next_state = match (character, chars.peek()) {
        ('/', Some('/')) => Some(CSharpLexState::LineComment),
        ('/', Some('*')) => Some(CSharpLexState::BlockComment),
        ('"', _) => Some(CSharpLexState::String),
        ('\'', _) => Some(CSharpLexState::Character),
        _ => None,
    };
    if let Some(next_state) = next_state {
        output.push(' ');
        if matches!(
            next_state,
            CSharpLexState::LineComment | CSharpLexState::BlockComment
        ) {
            output.push(' ');
            chars.next();
        }
        next_state
    } else {
        output.push(character);
        CSharpLexState::Code
    }
}

fn mask_csharp_line_comment(character: char, output: &mut String) -> CSharpLexState {
    if character == '\n' {
        output.push('\n');
        CSharpLexState::Code
    } else {
        output.push(' ');
        CSharpLexState::LineComment
    }
}

fn mask_csharp_block_comment(
    character: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    output: &mut String,
) -> CSharpLexState {
    if character == '*' && chars.peek() == Some(&'/') {
        output.push_str("  ");
        chars.next();
        CSharpLexState::Code
    } else {
        output.push(if character == '\n' { '\n' } else { ' ' });
        CSharpLexState::BlockComment
    }
}

fn mask_csharp_quoted(
    character: char,
    quote: char,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    output: &mut String,
) -> CSharpLexState {
    if character == '\\' {
        output.push(' ');
        if let Some(escaped) = chars.next() {
            output.push(if escaped == '\n' { '\n' } else { ' ' });
        }
    } else {
        output.push(if character == '\n' { '\n' } else { ' ' });
    }
    if character == quote || character == '\n' {
        CSharpLexState::Code
    } else if quote == '"' {
        CSharpLexState::String
    } else {
        CSharpLexState::Character
    }
}

fn csharp_declared_namespaces(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("namespace ")?;
            let namespace = rest
                .chars()
                .take_while(|character| {
                    *character == '.' || *character == '_' || character.is_ascii_alphanumeric()
                })
                .collect::<String>();
            namespace_like(&namespace).then_some(namespace)
        })
        .collect()
}
