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
