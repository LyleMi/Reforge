const STOP_WORDS: &[&str] = &[
    "api", "app", "cmd", "for", "from", "get", "has", "impl", "index", "main", "mod", "new", "old",
    "src", "test", "tests", "the", "this", "type", "use", "with",
];

pub(crate) fn split_identifier_words(identifier: &str) -> Vec<String> {
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

fn push_word(words: &mut Vec<String>, current: &mut String) {
    if current.is_empty() {
        return;
    }
    let word = normalize_word(current);
    if !word.is_empty() {
        words.push(word);
    }
    current.clear();
}

pub(crate) fn normalize_word(word: &str) -> String {
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

pub(crate) fn is_useful_concept_word(word: &str) -> bool {
    word.len() > 2
        && !STOP_WORDS.contains(&word)
        && !word.chars().all(|character| character.is_ascii_digit())
}

pub(crate) fn identifier_concepts(identifier: &str) -> std::collections::BTreeSet<String> {
    split_identifier_words(identifier)
        .into_iter()
        .filter(|word| is_useful_concept_word(word))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_and_normalizes_identifier_concepts() {
        assert_eq!(
            identifier_concepts("renderUserEntries"),
            std::collections::BTreeSet::from([
                "entry".to_string(),
                "render".to_string(),
                "user".to_string(),
            ])
        );
    }
}
