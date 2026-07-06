use super::*;

pub(super) fn collect_compatibility_paths(
    file: &SourceFile,
    path_intent: usize,
    signals: &mut DriftSignals,
) {
    if has_compatibility_exit_boundary(&file.source) {
        return;
    }

    for (index, line) in file.source.lines().enumerate() {
        let code = mask_string_literals(strip_line_comment(line));
        let code_words = split_identifier_words(&code);
        let Some(occurrence_name) = compatibility_occurrence_name(line, &code_words, path_intent)
        else {
            continue;
        };

        let score = path_intent
            + compatibility_intent_score(&code_words)
            + compatibility_context_score(line, &code_words);
        if score < 2 {
            continue;
        }

        signals
            .compatibility_paths
            .entry(file.display_path.clone())
            .or_default()
            .push(Occurrence {
                path: file.display_path.clone(),
                line: index + 1,
                name: Some(occurrence_name),
            });
    }
}

pub(super) fn compatibility_path_intent(file_words: &[String]) -> usize {
    compatibility_intent_score(file_words)
}

pub(super) fn stale_compatibility_path_findings(
    paths: &BTreeMap<String, Vec<Occurrence>>,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for occurrences in paths.values() {
        let mut group = occurrences.clone();
        group.sort_by(|left, right| left.path.cmp(&right.path).then(left.line.cmp(&right.line)));
        group.dedup_by(|left, right| left.path == right.path && left.line == right.line);

        let threshold = 2;
        if group.len() < threshold {
            continue;
        }

        let Some(representative) = group.first() else {
            continue;
        };

        findings.push(crate::scanner::finding(
            FindingInput::new(
                FindingKind::StaleCompatibilityPath,
                representative.path.clone(),
                Some(representative.line),
                format!(
                    "compatibility path has {} markers without a clear sunset or migration boundary",
                    group.len()
                ),
                vec![FindingMetric::threshold(
                    "group_size",
                    group.len(),
                    threshold,
                    "markers",
                )],
            )
            .with_related_locations(group.iter().map(related_location).collect()),
        ));
    }

    findings
}

fn mask_string_literals(line: &str) -> String {
    let mut masked = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();

    while let Some(character) = chars.next() {
        if !matches!(character, '"' | '\'' | '`') {
            masked.push(character);
            continue;
        }

        let quote = character;
        masked.push(' ');
        let mut escaped = false;
        for current in chars.by_ref() {
            if escaped {
                escaped = false;
                continue;
            }
            if current == '\\' {
                escaped = true;
                continue;
            }
            if current == quote {
                break;
            }
        }
    }

    masked
}

fn compatibility_occurrence_name(
    line: &str,
    code_words: &[String],
    path_intent: usize,
) -> Option<String> {
    if let Some(name) = function_or_class_name(line) {
        let words = split_identifier_words(&name);
        if compatibility_intent_score(&words) > 0 {
            return Some(name);
        }
        if words.iter().any(|word| word == COMPATIBILITY_FALLBACK_WORD)
            && has_fallback_compatibility_context(path_intent, code_words)
        {
            return Some(name);
        }
    }

    let strong_intent = code_words
        .iter()
        .find(|word| COMPATIBILITY_STRONG_WORDS.contains(&word.as_str()));
    if let Some(word) = strong_intent {
        return Some(format!("{word} compatibility marker"));
    }

    if code_words
        .iter()
        .any(|word| word == COMPATIBILITY_FALLBACK_WORD)
        && has_fallback_compatibility_context(path_intent, code_words)
    {
        return Some(format!(
            "{} compatibility marker",
            COMPATIBILITY_FALLBACK_WORD
        ));
    }

    let weak_intent = code_words
        .iter()
        .find(|word| COMPATIBILITY_WEAK_WORDS.contains(&word.as_str()));
    weak_intent.map(|word| format!("{word} compatibility marker"))
}

fn compatibility_intent_score(words: &[String]) -> usize {
    let mut score = 0;
    if words
        .iter()
        .any(|word| COMPATIBILITY_STRONG_WORDS.contains(&word.as_str()))
    {
        score += 2;
    }
    if words
        .iter()
        .any(|word| COMPATIBILITY_WEAK_WORDS.contains(&word.as_str()))
    {
        score += 1;
    }
    if words.iter().any(|word| is_compatibility_version_word(word)) {
        score += 1;
    }
    score
}

fn compatibility_context_score(line: &str, words: &[String]) -> usize {
    let lowered = strip_line_comment(line).to_ascii_lowercase();
    let mut score = 0;

    if function_or_class_name(line).is_some() {
        score += 1;
    }
    if contains_any(
        &lowered,
        &[
            "if ", "if(", "match ", "switch ", "case ", "else ", "? ", "&&", "||",
        ],
    ) && words.iter().any(|word| {
        is_compatibility_version_word(word) || COMPATIBILITY_WEAK_WORDS.contains(&word.as_str())
    }) {
        score += 1;
    }
    if contains_any(&lowered, &["deprecated(", "@deprecated", "#[deprecated"]) {
        score += 1;
    }

    score
}

fn has_fallback_compatibility_context(path_intent: usize, words: &[String]) -> bool {
    path_intent > 0
        || words.iter().any(|word| {
            is_compatibility_version_word(word)
                || COMPATIBILITY_STRONG_WORDS.contains(&word.as_str())
                || COMPATIBILITY_WEAK_WORDS.contains(&word.as_str())
        })
}

fn has_compatibility_exit_boundary(source: &str) -> bool {
    let lowered = source.to_ascii_lowercase();
    contains_any(&lowered, COMPATIBILITY_EXIT_PATTERNS)
}

fn is_compatibility_version_word(word: &str) -> bool {
    let Some(rest) = word.strip_prefix('v') else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|character| character.is_ascii_digit())
}

const COMPATIBILITY_STRONG_WORDS: &[&str] =
    &["deprecated", "legacy", "obsolete", "polyfill", "shim"];

const COMPATIBILITY_WEAK_WORDS: &[&str] = &["compat", "compatibility"];

const COMPATIBILITY_FALLBACK_WORD: &str = "fallback";

const COMPATIBILITY_EXIT_PATTERNS: &[&str] = &[
    "delete after",
    "expires",
    "migration plan",
    "migrate after",
    "owner:",
    "remove after",
    "remove once",
    "sunset",
];
