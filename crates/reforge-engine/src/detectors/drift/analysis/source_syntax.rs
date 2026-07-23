pub(super) fn function_or_class_name(line: &str) -> Option<String> {
    let line = strip_line_comment(line)
        .trim()
        .trim_start_matches("export ");
    let line = line.trim_start_matches("pub ").trim_start_matches("async ");

    if let Some(rest) = line.strip_prefix("func ") {
        return identifier_before(go_function_name_start(rest), '(');
    }
    for (prefix, delimiters) in [
        ("def ", &['('][..]),
        ("class ", &['(', ':', '{'][..]),
        ("fn ", &['('][..]),
        ("function ", &['('][..]),
    ] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return identifier_before_any(rest, delimiters);
        }
    }

    callable_binding_name(line)
}

fn go_function_name_start(rest: &str) -> &str {
    if !rest.trim_start().starts_with('(') {
        return rest;
    }
    rest.find(')')
        .and_then(|position| rest.get(position + 1..))
        .unwrap_or(rest)
        .trim_start()
}

fn callable_binding_name(line: &str) -> Option<String> {
    for prefix in ["const ", "let ", "var "] {
        let Some(rest) = line.strip_prefix(prefix) else {
            continue;
        };
        let Some(name) = identifier_before_any(rest, &['=', ':']) else {
            continue;
        };
        if rest.contains("=>") || rest.contains("function") {
            return Some(name);
        }
    }
    None
}

pub(super) fn type_start(line: &str) -> Option<(String, bool)> {
    let trimmed = strip_line_comment(line)
        .trim()
        .trim_start_matches("export ");
    let trimmed = trimmed.trim_start_matches("pub ");

    if let Some(rest) = trimmed.strip_prefix("struct ") {
        return identifier_before(rest, '{').map(|name| (name, true));
    }
    if let Some(rest) = trimmed.strip_prefix("interface ") {
        return identifier_before(rest, '{').map(|name| (name, true));
    }
    if let Some(rest) = trimmed.strip_prefix("type ") {
        if rest.contains(" struct ") {
            return identifier_before(rest, ' ').map(|name| (name, true));
        }
        if rest.contains("= {") {
            return identifier_before(rest, '=').map(|name| (name, true));
        }
    }
    if let Some(rest) = trimmed.strip_prefix("class ") {
        return identifier_before_any(rest, &['(', ':', '{']).map(|name| {
            let braced = trimmed.contains('{');
            (name, braced)
        });
    }

    None
}

pub(super) fn field_names_from_line(line: &str) -> Vec<String> {
    strip_line_comment(line)
        .split([',', ';'])
        .filter_map(field_name_from_segment)
        .collect()
}

pub(super) fn code_without_quoted_literals(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    let mut state = QuoteMaskState::default();
    for character in line.chars() {
        output.push(state.mask(character));
    }
    output
}

#[derive(Default)]
struct QuoteMaskState {
    quote: Option<char>,
    escaped: bool,
}

impl QuoteMaskState {
    fn mask(&mut self, character: char) -> char {
        match (self.escaped, self.quote, character) {
            (true, _, _) => {
                self.escaped = false;
                ' '
            }
            (false, Some(_), '\\') => {
                self.escaped = true;
                ' '
            }
            (false, Some(quote), value) if quote == value => {
                self.quote = None;
                ' '
            }
            (false, None, value @ ('"' | '\'' | '`')) => {
                self.quote = Some(value);
                ' '
            }
            (false, Some(_), _) => ' ',
            (false, None, value) => value,
        }
    }
}

pub(super) fn field_name_from_segment(segment: &str) -> Option<String> {
    let segment = normalized_field_segment(segment);
    if is_non_field_segment(segment) {
        return None;
    }

    if let Some(name) = self_field_name(segment) {
        return Some(name);
    }

    let cleaned = segment
        .trim_start_matches("pub ")
        .trim_start_matches("readonly ")
        .trim_start_matches("private ")
        .trim_start_matches("protected ")
        .trim_start_matches("public ")
        .trim_start_matches("static ")
        .trim();

    typed_field_name(cleaned)
        .or_else(|| declaration_field_name(cleaned))
        .map(|name| normalize_word(&name))
}

fn normalized_field_segment(segment: &str) -> &str {
    let mut segment = segment.trim().trim_end_matches(',').trim();
    if let Some((_, after_brace)) = segment.rsplit_once('{') {
        segment = after_brace.trim();
    }
    if let Some((before_brace, _)) = segment.split_once('}') {
        segment = before_brace.trim();
    }
    segment
}

fn is_non_field_segment(segment: &str) -> bool {
    segment.is_empty()
        || segment.starts_with("fn ")
        || segment.starts_with("def ")
        || segment.starts_with("func ")
        || segment.starts_with("constructor")
        || segment.starts_with("return ")
}

fn self_field_name(segment: &str) -> Option<String> {
    let position = segment.find("self.")?;
    let rest = &segment[position + "self.".len()..];
    identifier_before_any(rest, &['=', ':', ' ', ')']).map(|name| normalize_word(&name))
}

fn typed_field_name(cleaned: &str) -> Option<String> {
    let name = identifier_before_any(cleaned, &[':', '?'])?;
    (is_valid_field_name(&name) && cleaned.contains(':')).then_some(name)
}

fn declaration_field_name(cleaned: &str) -> Option<String> {
    let mut parts = cleaned.split_whitespace();
    let first = parts.next()?;
    if is_valid_field_name(first) && parts.next().is_some() && !first.contains('(') {
        return Some(first.to_string());
    }
    None
}

pub(super) fn identifier_before(rest: &str, delimiter: char) -> Option<String> {
    rest.split(delimiter)
        .next()
        .map(str::trim)
        .filter(|name| is_valid_identifier(name))
        .map(ToString::to_string)
}

pub(super) fn identifier_before_any(rest: &str, delimiters: &[char]) -> Option<String> {
    let position = rest
        .char_indices()
        .find_map(|(index, character)| delimiters.contains(&character).then_some(index))
        .unwrap_or(rest.len());
    let name = rest[..position].trim();
    is_valid_identifier(name).then(|| name.to_string())
}

pub(super) fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && chars.all(is_identifier_char)
}

pub(super) fn is_identifier_char(character: char) -> bool {
    character == '_' || character == '-' || character.is_ascii_alphanumeric()
}

pub(super) fn is_valid_field_name(name: &str) -> bool {
    is_valid_identifier(name)
        && !matches!(
            name,
            "if" | "for"
                | "while"
                | "match"
                | "switch"
                | "case"
                | "return"
                | "let"
                | "const"
                | "var"
                | "type"
                | "interface"
                | "class"
                | "struct"
        )
}

pub(super) fn string_literals(line: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut chars = line.char_indices().peekable();

    while let Some((start, character)) = chars.next() {
        if !matches!(character, '"' | '\'' | '`') {
            continue;
        }

        let quote = character;
        let mut escaped = false;
        let mut end = None;
        for (index, current) in chars.by_ref() {
            if escaped {
                escaped = false;
                continue;
            }
            if current == '\\' {
                escaped = true;
                continue;
            }
            if current == quote {
                end = Some(index);
                break;
            }
        }

        if let Some(end) = end
            && let Some(literal) = line.get(start + quote.len_utf8()..end)
        {
            literals.push(literal.to_string());
        }
    }

    literals
}

pub(super) fn constant_keys(line: &str) -> Vec<String> {
    let trimmed = strip_line_comment(line).trim();
    let lowered = trimmed.to_ascii_lowercase();
    if !contains_any(&lowered, &["const ", "static ", "const_", "public const"]) {
        return Vec::new();
    }

    trimmed
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|part| is_config_key(part))
        .map(ToString::to_string)
        .collect()
}

pub(super) fn is_config_key(value: &str) -> bool {
    if value.len() < 4 {
        return false;
    }
    if value.starts_with('/') {
        return value.matches('/').count() >= 2 && !value.contains(' ');
    }

    let upperish = value.chars().all(|character| {
        character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
    });
    if !upperish || !value.contains('_') {
        return false;
    }

    let lowered = value.to_ascii_lowercase();
    CONFIG_KEY_WORDS
        .iter()
        .any(|word| lowered.split('_').any(|part| part == *word))
}
