use super::*;

pub(in crate::detectors::structure) fn collect_file_naming_style(
    file: &SourceFile,
    signals: &mut FileSignals,
) {
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

pub(in crate::detectors::structure) fn file_naming_drift_findings(
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
            FindingInput::new(
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
                    crate::model::MetricId::GroupSize,
                    styles.len(),
                    2,
                    "naming styles",
                )],
            )
            .with_related_locations(related_locations),
        ));
    }

    findings
}

pub(in crate::detectors::structure) fn effective_naming_styles(
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

pub(in crate::detectors::structure) fn naming_drift_locations(
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

pub(in crate::detectors::structure) fn normalized_naming_stem(path: &Path) -> Option<String> {
    let mut stem = path.file_stem()?.to_str()?.to_string();

    while let Some(stripped) = test_file_suffix_base(&stem) {
        stem = stripped.to_string();
    }

    if stem.is_empty()
        || matches!(
            stem.as_str(),
            "mod"
                | "lib"
                | "main"
                | "index"
                | "__init__"
                | "package"
                | "package-info"
                | "module-info"
        )
    {
        None
    } else {
        Some(stem)
    }
}

pub(in crate::detectors::structure) fn test_file_suffix_base(stem: &str) -> Option<&str> {
    stem.strip_suffix(".test")
        .or_else(|| stem.strip_suffix(".spec"))
        .or_else(|| stem.strip_suffix("_test"))
        .or_else(|| stem.strip_suffix("_tests"))
        .or_else(|| stem.strip_suffix("-test"))
        .or_else(|| stem.strip_suffix("-spec"))
}

pub(in crate::detectors::structure) fn classify_file_naming_style(
    stem: &str,
) -> Option<FileNamingStyle> {
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

pub(in crate::detectors::structure) fn separated_words_are_lowercase(
    stem: &str,
    separator: char,
) -> bool {
    stem.split(separator).all(|part| {
        !part.is_empty()
            && part
                .chars()
                .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
    })
}

impl FileNamingStyle {
    pub(in crate::detectors::structure) fn label(self) -> &'static str {
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

pub(in crate::detectors::structure) fn collect_directory_concepts(
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

pub(in crate::detectors::structure) fn directory_drift_findings(
    directories: &BTreeMap<PathBuf, BTreeSet<String>>,
    options: &StructureOptions,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (directory, concepts) in directories {
        let threshold = options.max_dir_files.max(4);
        if concepts.len() > threshold {
            findings.push(crate::scanner::finding(FindingInput::new(
                FindingKind::DirectoryDrift,
                directory.to_string_lossy().replace('\\', "/"),
                None,
                format!(
                    "directory mixes {} naming/language concepts; consider grouping cohesive responsibilities",
                    concepts.len()
                ),
                vec![FindingMetric::threshold(
                    crate::model::MetricId::GroupSize,
                    concepts.len(),
                    threshold,
                    "concepts",
                )],
            )));
        }
    }
    findings
}

pub(in crate::detectors::structure) fn group_occurrences(
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
            crate::model::MetricId::GroupSize,
            group.len(),
            min_occurrences,
            "occurrences",
        )];
        let finding = if kind == FindingKind::RepeatedLiteral {
            crate::scanner::scored_finding(
                FindingInput::new(
                    kind,
                    representative.path.clone(),
                    Some(representative.line),
                    message(&key, group.len()),
                    metrics,
                )
                .with_detection_reliability(repeated_literal_confidence(&key, &group))
                .with_related_locations(related_locations),
            )
        } else {
            crate::scanner::finding(
                FindingInput::new(
                    kind,
                    representative.path.clone(),
                    Some(representative.line),
                    message(&key, group.len()),
                    metrics,
                )
                .with_related_locations(related_locations),
            )
        };
        findings.push(finding);
    }

    findings
}

pub(in crate::detectors::structure) fn count_named_descendants(
    node: Node<'_>,
    kind: &str,
) -> usize {
    let mut count = usize::from(node.kind() == kind);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        count += count_named_descendants(child, kind);
    }
    count
}

pub(in crate::detectors::structure) fn node_line_span(node: Node<'_>) -> usize {
    node.end_position()
        .row
        .saturating_sub(node.start_position().row)
        + 1
}

pub(in crate::detectors::structure) fn normalize_identifier(text: &str) -> String {
    text.trim_matches(|character: char| !character.is_alphanumeric() && character != '_')
        .to_ascii_lowercase()
}

pub(in crate::detectors::structure) fn normalize_pattern(text: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_space = false;
    for character in text.chars() {
        if let Some(character) = normalized_pattern_char(character, &mut previous_was_space) {
            normalized.push(character);
        }
    }
    normalized.trim().to_string()
}

pub(in crate::detectors::structure) fn normalized_pattern_char(
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

pub(in crate::detectors::structure) fn split_directory_concept_words(text: &str) -> Vec<String> {
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
