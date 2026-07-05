use super::*;

pub(super) fn parallel_implementation_message(key: &str, count: usize) -> String {
    format!("capability `{key}` has {count} parallel-looking implementations")
}

pub(super) fn shadowed_abstraction_message(key: &str, count: usize) -> String {
    format!("helper abstraction `{key}` is shadowed in {count} locations")
}

pub(super) fn config_key_drift_message(key: &str, count: usize) -> String {
    format!("config or route key {key:?} appears in {count} locations")
}

pub(super) fn fixture_factory_drift_message(key: &str, count: usize) -> String {
    format!("test fixture factory concept `{key}` appears in {count} locations")
}

pub(super) fn boundary_inventory(files: &[SourceFile]) -> BoundaryInventory {
    let mut inventory = BoundaryInventory::default();

    for file in files {
        let words = path_words(&file.path);
        if words
            .iter()
            .any(|word| HTTP_BOUNDARY_WORDS.contains(&word.as_str()))
        {
            inventory.http = true;
        }
        if words
            .iter()
            .any(|word| CONFIG_BOUNDARY_WORDS.contains(&word.as_str()))
        {
            inventory.config = true;
        }
        if words
            .iter()
            .any(|word| FS_BOUNDARY_WORDS.contains(&word.as_str()))
        {
            inventory.filesystem = true;
        }
        if words
            .iter()
            .any(|word| LOG_BOUNDARY_WORDS.contains(&word.as_str()))
        {
            inventory.logging = true;
        }
    }

    inventory
}

pub(super) fn is_boundary_file(path: &Path, kind: BypassKind) -> bool {
    let words = path_words(path);
    match kind {
        BypassKind::Http => words
            .iter()
            .any(|word| HTTP_BOUNDARY_WORDS.contains(&word.as_str())),
        BypassKind::Config => words
            .iter()
            .any(|word| CONFIG_BOUNDARY_WORDS.contains(&word.as_str())),
        BypassKind::Filesystem => words
            .iter()
            .any(|word| FS_BOUNDARY_WORDS.contains(&word.as_str())),
        BypassKind::Logging => words
            .iter()
            .any(|word| LOG_BOUNDARY_WORDS.contains(&word.as_str())),
    }
}

impl BoundaryInventory {
    pub(super) fn has(self, kind: BypassKind) -> bool {
        match kind {
            BypassKind::Http => self.http,
            BypassKind::Config => self.config,
            BypassKind::Filesystem => self.filesystem,
            BypassKind::Logging => self.logging,
        }
    }
}

impl BypassKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            BypassKind::Http => "HTTP",
            BypassKind::Config => WORD_CONFIG,
            BypassKind::Filesystem => "filesystem",
            BypassKind::Logging => "logging",
        }
    }
}

pub(super) fn function_or_class_name(line: &str) -> Option<String> {
    let line = strip_line_comment(line)
        .trim()
        .trim_start_matches("export ");
    let line = line.trim_start_matches("pub ").trim_start_matches("async ");

    if let Some(rest) = line.strip_prefix("def ") {
        return identifier_before(rest, '(');
    }
    if let Some(rest) = line.strip_prefix("class ") {
        return identifier_before_any(rest, &['(', ':', '{']);
    }
    if let Some(rest) = line.strip_prefix("fn ") {
        return identifier_before(rest, '(');
    }
    if let Some(rest) = line.strip_prefix("func ") {
        let rest = if rest.trim_start().starts_with('(') {
            rest.find(')')
                .and_then(|position| rest.get(position + 1..))
                .unwrap_or(rest)
                .trim_start()
        } else {
            rest
        };
        return identifier_before(rest, '(');
    }
    if let Some(rest) = line.strip_prefix("function ") {
        return identifier_before(rest, '(');
    }

    for prefix in ["const ", "let ", "var "] {
        if let Some(rest) = line.strip_prefix(prefix) {
            let Some(name) = identifier_before_any(rest, &['=', ':']) else {
                continue;
            };
            if rest.contains("=>") || rest.contains("function") {
                return Some(name);
            }
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

pub(super) fn field_name_from_segment(segment: &str) -> Option<String> {
    let mut segment = segment.trim().trim_end_matches(',').trim();
    if let Some((_, after_brace)) = segment.rsplit_once('{') {
        segment = after_brace.trim();
    }
    if let Some((before_brace, _)) = segment.split_once('}') {
        segment = before_brace.trim();
    }

    if segment.is_empty()
        || segment.starts_with("fn ")
        || segment.starts_with("def ")
        || segment.starts_with("func ")
        || segment.starts_with("constructor")
        || segment.starts_with("return ")
    {
        return None;
    }

    if let Some(position) = segment.find("self.") {
        let rest = &segment[position + "self.".len()..];
        if let Some(name) = identifier_before_any(rest, &['=', ':', ' ', ')']) {
            return Some(normalize_word(&name));
        }
    }

    let cleaned = segment
        .trim_start_matches("pub ")
        .trim_start_matches("readonly ")
        .trim_start_matches("private ")
        .trim_start_matches("protected ")
        .trim_start_matches("public ")
        .trim_start_matches("static ")
        .trim();

    if let Some(name) = identifier_before_any(cleaned, &[':', '?'])
        && is_valid_field_name(&name)
        && cleaned.contains(':')
    {
        return Some(normalize_word(&name));
    }

    let mut parts = cleaned.split_whitespace();
    let first = parts.next()?;
    if is_valid_field_name(first) && parts.next().is_some() && !first.contains('(') {
        return Some(normalize_word(first));
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

pub(super) fn field_overlap(left: &BTreeSet<String>, right: &BTreeSet<String>) -> f64 {
    let common = left.intersection(right).count();
    if common < 3 {
        return 0.0;
    }
    common as f64 / left.len().max(right.len()) as f64
}

pub(super) fn shared_fields(group: &[TypeShape]) -> Vec<String> {
    let mut fields = group
        .first()
        .map(|shape| shape.fields.clone())
        .unwrap_or_default();
    for shape in group.iter().skip(1) {
        fields = fields
            .intersection(&shape.fields)
            .cloned()
            .collect::<BTreeSet<_>>();
    }
    fields.into_iter().collect()
}

pub(super) fn concept_key(words: &[String], stop_words: &[&str], max_words: usize) -> String {
    let mut concepts = Vec::new();
    for word in words {
        let normalized = normalize_word(word);
        if !is_useful_concept_word(&normalized) || stop_words.contains(&normalized.as_str()) {
            continue;
        }
        if !concepts.contains(&normalized) {
            concepts.push(normalized);
        }
        if concepts.len() == max_words {
            break;
        }
    }

    concepts.join(" ")
}

pub(super) fn split_identifier_words(identifier: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut previous_lowercase = false;

    for character in identifier.chars() {
        if character == '_' || character == '-' || character == '/' || character == '\\' {
            push_word(&mut words, &mut current);
            previous_lowercase = false;
            continue;
        }

        if character.is_ascii_uppercase() && previous_lowercase {
            push_word(&mut words, &mut current);
        }

        if character.is_ascii_alphanumeric() {
            previous_lowercase = character.is_ascii_lowercase() || character.is_ascii_digit();
            current.push(character.to_ascii_lowercase());
        } else {
            push_word(&mut words, &mut current);
            previous_lowercase = false;
        }
    }
    push_word(&mut words, &mut current);

    words
}

pub(super) fn push_word(words: &mut Vec<String>, current: &mut String) {
    if current.is_empty() {
        return;
    }
    let word = normalize_word(current);
    if !word.is_empty() {
        words.push(word);
    }
    current.clear();
}

pub(super) fn normalize_word(word: &str) -> String {
    let mut normalized = word
        .trim_matches(|character: char| !character.is_ascii_alphanumeric())
        .to_ascii_lowercase();
    if normalized.len() > 4 && normalized.ends_with("ies") {
        normalized.truncate(normalized.len() - 3);
        normalized.push('y');
    } else if normalized.len() > 4 && normalized.ends_with('s') {
        normalized.truncate(normalized.len() - 1);
    }
    normalized
}

pub(super) fn is_useful_concept_word(word: &str) -> bool {
    word.len() > 2
        && !STOP_WORDS.contains(&word)
        && !word.chars().all(|character| character.is_ascii_digit())
}

pub(super) fn path_words(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .flat_map(path_component_words_from_str)
        .collect()
}

pub(super) fn path_component_words(path: &Path) -> Vec<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(path_component_words_from_str)
        .unwrap_or_default()
}

pub(super) fn path_component_words_from_str(component: &str) -> Vec<String> {
    let without_extension = component
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(component);
    split_identifier_words(without_extension)
}

pub(super) fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn strip_line_comment(line: &str) -> &str {
    let slash = line.find("//");
    let hash = line.find('#');
    match (slash, hash) {
        (Some(left), Some(right)) => &line[..left.min(right)],
        (Some(index), None) | (None, Some(index)) => &line[..index],
        (None, None) => line,
    }
}

pub(super) fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

pub(super) fn leading_spaces(line: &str) -> usize {
    line.chars()
        .take_while(|character| character.is_ascii_whitespace())
        .count()
}

pub(super) fn brace_delta(line: &str) -> isize {
    let mut delta = 0;
    for character in strip_line_comment(line).chars() {
        match character {
            '{' => delta += 1,
            '}' => delta -= 1,
            _ => {}
        }
    }
    delta
}

pub(super) fn related_location(occurrence: &Occurrence) -> RelatedLocation {
    RelatedLocation {
        path: occurrence.path.clone(),
        line: occurrence.line,
        name: occurrence.name.clone(),
    }
}

pub(super) const WORD_CONFIG: &str = "config";
pub(super) const WORD_FACTORY: &str = "factory";

pub(super) const HTTP_BYPASS_PATTERNS: &[&str] = &[
    "fetch(",
    "axios.",
    "axios(",
    "requests.get(",
    "requests.post(",
    "reqwest::",
    "hyper::",
    "http.client",
];

pub(super) const CONFIG_BYPASS_PATTERNS: &[&str] = &[
    "process.env",
    "std::env::var",
    "env::var(",
    "os.environ",
    "os.getenv(",
    "getenv(",
];

pub(super) const FILESYSTEM_BYPASS_PATTERNS: &[&str] = &[
    "fs.readfile",
    "fs.writfile",
    "fs.writefile",
    "std::fs::read",
    "std::fs::write",
    "file::open",
    "read_to_string",
];

pub(super) const LOGGING_BYPASS_PATTERNS: &[&str] = &[
    "console.log(",
    "println!(",
    "dbg!(",
    "print(",
    "log.printf(",
    "log.println(",
];

pub(super) const BYPASS_RULES: &[BypassRule] = &[
    BypassRule {
        kind: BypassKind::Http,
        patterns: HTTP_BYPASS_PATTERNS,
        occurrence_name: "direct HTTP call",
    },
    BypassRule {
        kind: BypassKind::Config,
        patterns: CONFIG_BYPASS_PATTERNS,
        occurrence_name: "direct config read",
    },
    BypassRule {
        kind: BypassKind::Filesystem,
        patterns: FILESYSTEM_BYPASS_PATTERNS,
        occurrence_name: "direct filesystem call",
    },
    BypassRule {
        kind: BypassKind::Logging,
        patterns: LOGGING_BYPASS_PATTERNS,
        occurrence_name: "direct log call",
    },
];

pub(super) const PARALLEL_CAPABILITY_WORDS: &[&str] = &[
    "adapt",
    "adapter",
    "build",
    "cache",
    "client",
    WORD_CONFIG,
    "load",
    "logger",
    "map",
    "normalize",
    "parse",
    "retry",
    "validate",
];

pub(super) const PARALLEL_STOP_WORDS: &[&str] = &[
    "a", "and", "do", "for", "from", "get", "has", "is", "make", "new", "of", "set", "the", "to",
    "with",
];

pub(super) const SHADOW_HELPER_WORDS: &[&str] = &[
    "common",
    "helper",
    "helpers",
    "shared",
    "util",
    "utils",
    "adapter",
    "normalizer",
    "validator",
    "mapper",
    WORD_FACTORY,
];

pub(super) const SHADOW_STOP_WORDS: &[&str] = &[
    "common",
    "helper",
    "helpers",
    "shared",
    "util",
    "utils",
    WORD_FACTORY,
    "make",
    "create",
    "build",
    "get",
    "set",
];

pub(super) const FIXTURE_WORDS: &[&str] = &[
    "builder",
    "dummy",
    WORD_FACTORY,
    "fake",
    "fixture",
    "mock",
    "sample",
    "setup",
    "test",
];

pub(super) const GENERIC_BUCKET_WORDS: &[&str] = &[
    "common", "helper", "helpers", "lib", "misc", "shared", "util", "utils",
];

pub(super) const STOP_WORDS: &[&str] = &[
    "api", "app", "cmd", "for", "from", "get", "has", "impl", "index", "main", "mod", "new", "old",
    "src", "test", "tests", "the", "this", "type", "use", "with",
];

pub(super) const CONFIG_KEY_WORDS: &[&str] = &[
    "api",
    "auth",
    "base",
    "client",
    "code",
    WORD_CONFIG,
    "database",
    "db",
    "endpoint",
    "env",
    "error",
    "host",
    "key",
    "path",
    "port",
    "route",
    "secret",
    "service",
    "token",
    "url",
];

pub(super) const HTTP_BOUNDARY_WORDS: &[&str] = &[
    "adapter",
    "adapters",
    "api",
    "client",
    "clients",
    "gateway",
    "http",
    "request",
    "transport",
];

pub(super) const CONFIG_BOUNDARY_WORDS: &[&str] =
    &[WORD_CONFIG, "configuration", "env", "setting", "settings"];

pub(super) const FS_BOUNDARY_WORDS: &[&str] = &[
    "dao",
    "file",
    "filesystem",
    "persistence",
    "repository",
    "storage",
    "store",
];

pub(super) const LOG_BOUNDARY_WORDS: &[&str] =
    &["log", "logger", "logging", "telemetry", "trace", "tracing"];
