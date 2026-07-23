use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::detectors::similarity::SourceFile;
use crate::evidence_analysis::DetectedEvidenceInput;
use crate::model::{DetectedEvidence, DetectedMeasurement, RelatedLocation, Rule};
use crate::scan::is_test_source;

#[derive(Debug, Clone)]
pub struct ConceptDriftOptions {
    pub min_repeated_occurrences: usize,
    pub min_data_shape_occurrences: usize,
    pub max_dir_files: usize,
    pub include_test_structure: bool,
}

type Occurrence = RelatedLocation;

#[derive(Debug, Clone)]
struct FunctionSignal {
    occurrence: Occurrence,
    words: Vec<String>,
    file_words: Vec<String>,
    is_test: bool,
}

#[derive(Debug, Clone)]
struct TypeShape {
    occurrence: Occurrence,
    fields: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BypassKind {
    Http,
    Config,
    Filesystem,
    Logging,
}

#[derive(Debug, Default)]
struct DriftSignals {
    functions: Vec<FunctionSignal>,
    type_shapes: Vec<TypeShape>,
    config_keys: Vec<(String, Occurrence)>,
    fixture_factories: Vec<(String, Occurrence)>,
    generic_directories: BTreeMap<PathBuf, GenericDirectory>,
    generic_files: Vec<(String, Occurrence)>,
    bypasses: BTreeMap<BypassKind, Vec<Occurrence>>,
    compatibility_paths: BTreeMap<String, Vec<Occurrence>>,
}

#[derive(Debug, Default)]
struct GenericDirectory {
    display_path: String,
    files: BTreeSet<String>,
    concepts: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, Default)]
struct BoundaryInventory {
    http: bool,
    config: bool,
    filesystem: bool,
    logging: bool,
}

#[derive(Debug, Clone, Copy)]
struct BypassRule {
    kind: BypassKind,
    patterns: &'static [&'static str],
    occurrence_name: &'static str,
}

struct OccurrenceGroupSpec {
    threshold: usize,
    kind: Rule,
    message: fn(&str, usize) -> String,
    require_cross_file: bool,
}

pub fn scan_concept_drift(
    files: &[SourceFile],
    options: &ConceptDriftOptions,
) -> Vec<DetectedEvidence> {
    let mut signals = DriftSignals::default();

    for file in files {
        let boundaries = boundary_inventory(files, &file.path);
        collect_file_signals(file, options, boundaries, &mut signals);
    }

    let mut detections = Vec::new();
    detections.extend(parallel_implementation_detections(
        &signals.functions,
        options,
    ));
    detections.extend(shadowed_abstraction_detections(&signals.functions, options));
    detections.extend(duplicate_type_shape_detections(
        &signals.type_shapes,
        options,
    ));
    detections.extend(group_occurrences(
        signals.config_keys,
        OccurrenceGroupSpec {
            threshold: options.min_repeated_occurrences.max(2),
            kind: Rule::ConfigKeyDrift,
            message: config_key_drift_message,
            require_cross_file: true,
        },
    ));
    detections.extend(group_occurrences(
        signals.fixture_factories,
        OccurrenceGroupSpec {
            threshold: options.min_data_shape_occurrences.max(2),
            kind: Rule::FixtureFactoryDrift,
            message: fixture_factory_drift_message,
            require_cross_file: true,
        },
    ));
    detections.extend(generic_bucket_detections(
        &signals.generic_directories,
        &signals.generic_files,
        options,
    ));
    detections.extend(adapter_boundary_bypass_detections(&signals.bypasses));
    detections.extend(stale_compatibility_path_detections(
        &signals.compatibility_paths,
    ));

    detections.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then(left.path.cmp(&right.path))
            .then(left.line.cmp(&right.line))
            .then(left.message.cmp(&right.message))
    });
    detections
}

fn collect_file_signals(
    file: &SourceFile,
    options: &ConceptDriftOptions,
    boundaries: BoundaryInventory,
    signals: &mut DriftSignals,
) {
    let is_test = is_test_source(&file.path);
    let file_words = path_words(&file.path);

    if is_test {
        collect_fixture_factories(file, &file_words, signals);
    }

    if !is_test || options.include_test_structure {
        collect_generic_bucket_signals(file, &file_words, signals);
        collect_function_signals(file, &file_words, is_test, signals);
        collect_type_shapes(file, signals);
        collect_config_keys(file, signals);
        collect_compatibility_paths(file, compatibility_path_intent(&file_words), signals);
    }

    if !is_test && !is_operational_entrypoint_source(&file_words) {
        collect_boundary_bypasses(file, boundaries, signals);
    }
}

fn is_operational_entrypoint_source(file_words: &[String]) -> bool {
    file_words.iter().any(|word| {
        matches!(
            word.as_str(),
            "bin"
                | "cli"
                | "cmd"
                | "command"
                | "commands"
                | "script"
                | "scripts"
                | "tool"
                | "tools"
                | "bench"
                | "benches"
                | "benchmark"
                | "benchmarks"
                | "differential"
                | "oracle"
        )
    })
}

fn collect_function_signals(
    file: &SourceFile,
    file_words: &[String],
    is_test: bool,
    signals: &mut DriftSignals,
) {
    for (index, line) in file.source.lines().enumerate() {
        let Some(name) = function_or_class_name(line) else {
            continue;
        };
        let words = split_identifier_words(&name);
        if words.is_empty() {
            continue;
        }

        signals.functions.push(FunctionSignal {
            occurrence: Occurrence {
                path: file.display_path.clone(),
                line: index + 1,
                name: Some(name.clone()),
            },
            words,
            file_words: file_words.to_vec(),
            is_test,
        });
    }
}

fn collect_fixture_factories(file: &SourceFile, file_words: &[String], signals: &mut DriftSignals) {
    for (index, line) in file.source.lines().enumerate() {
        let Some(name) = function_or_class_name(line) else {
            continue;
        };

        let mut words = split_identifier_words(&name);
        if !words
            .iter()
            .any(|word| FIXTURE_WORDS.contains(&word.as_str()))
        {
            continue;
        }

        words.extend(
            file_words
                .iter()
                .filter(|word| !GENERIC_BUCKET_WORDS.contains(&word.as_str()))
                .cloned(),
        );
        let key = concept_key(&words, FIXTURE_WORDS, 3);
        if key.is_empty() {
            continue;
        }

        signals.fixture_factories.push((
            key,
            Occurrence {
                path: file.display_path.clone(),
                line: index + 1,
                name: Some(name),
            },
        ));
    }
}

fn collect_type_shapes(file: &SourceFile, signals: &mut DriftSignals) {
    let lines = file.source.lines().collect::<Vec<_>>();
    let mut index = 0;

    while index < lines.len() {
        let Some((name, braced)) = type_start(lines[index]) else {
            index += 1;
            continue;
        };

        let start_line = index + 1;
        let (fields, next_index) = if braced {
            braced_type_fields(&lines, index)
        } else {
            indented_type_fields(&lines, index)
        };
        index = next_index;

        if fields.len() >= 3 {
            signals.type_shapes.push(TypeShape {
                occurrence: Occurrence {
                    path: file.display_path.clone(),
                    line: start_line,
                    name: Some(name),
                },
                fields,
            });
        }
    }
}

fn braced_type_fields(lines: &[&str], index: usize) -> (BTreeSet<String>, usize) {
    let mut fields: BTreeSet<String> = field_names_from_line(lines[index]).into_iter().collect();
    let mut depth = brace_delta(lines[index]);
    let mut scan_index = index + 1;
    while scan_index < lines.len() {
        fields.extend(field_names_from_line(lines[scan_index]));
        depth += brace_delta(lines[scan_index]);
        scan_index += 1;
        if depth <= 0 {
            break;
        }
    }
    (fields, scan_index)
}

fn indented_type_fields(lines: &[&str], index: usize) -> (BTreeSet<String>, usize) {
    let class_indent = leading_spaces(lines[index]);
    let mut fields = BTreeSet::new();
    let mut scan_index = index + 1;
    while scan_index < lines.len() {
        let line = lines[scan_index];
        if !line.trim().is_empty() && leading_spaces(line) <= class_indent {
            break;
        }
        fields.extend(field_names_from_line(line));
        scan_index += 1;
    }
    (fields, scan_index)
}

fn collect_config_keys(file: &SourceFile, signals: &mut DriftSignals) {
    for (index, line) in file.source.lines().enumerate() {
        let line_number = index + 1;
        let active_line = strip_line_comment(line);
        for literal in string_literals(active_line) {
            if is_config_key(&literal) {
                signals.config_keys.push((
                    literal.clone(),
                    Occurrence {
                        path: file.display_path.clone(),
                        line: line_number,
                        name: Some(literal),
                    },
                ));
            }
        }

        for key in constant_keys(active_line) {
            signals.config_keys.push((
                key.clone(),
                Occurrence {
                    path: file.display_path.clone(),
                    line: line_number,
                    name: Some(key),
                },
            ));
        }
    }
}

fn collect_generic_bucket_signals(
    file: &SourceFile,
    file_words: &[String],
    signals: &mut DriftSignals,
) {
    let Some(parent) = file.path.parent() else {
        return;
    };

    let parent_words = path_component_words(parent);
    if let Some(generic) = parent_words
        .iter()
        .find(|word| GENERIC_BUCKET_WORDS.contains(&word.as_str()))
    {
        let entry = signals
            .generic_directories
            .entry(parent.to_path_buf())
            .or_insert_with(|| GenericDirectory {
                display_path: normalize_path(parent),
                files: BTreeSet::new(),
                concepts: BTreeSet::new(),
            });
        entry.files.insert(file.display_path.clone());
        for word in file_words {
            if is_useful_concept_word(word) && word != generic {
                entry.concepts.insert(word.clone());
            }
        }
    }

    let Some(stem) = file.path.file_stem().and_then(|stem| stem.to_str()) else {
        return;
    };
    let stem_words = split_identifier_words(stem);
    if stem_words
        .iter()
        .any(|word| GENERIC_BUCKET_WORDS.contains(&word.as_str()))
    {
        let concepts = file
            .source
            .lines()
            .filter_map(function_or_class_name)
            .flat_map(|name| split_identifier_words(&name))
            .filter(|word| is_useful_concept_word(word))
            .collect::<BTreeSet<_>>();
        if concepts.len() >= 4 {
            signals.generic_files.push((
                concepts.iter().cloned().collect::<Vec<_>>().join(", "),
                Occurrence {
                    path: file.display_path.clone(),
                    line: 1,
                    name: Some(stem.to_string()),
                },
            ));
        }
    }
}

fn collect_boundary_bypasses(
    file: &SourceFile,
    boundaries: BoundaryInventory,
    signals: &mut DriftSignals,
) {
    if is_test_source(&file.path) {
        return;
    }
    let rules = active_bypass_rules(boundaries, &file.path);
    if rules.is_empty() {
        return;
    }

    for (index, line) in file.source.lines().enumerate() {
        let line_number = index + 1;
        collect_line_boundary_bypasses(file, signals, &rules, line, line_number);
    }
}

fn active_bypass_rules(boundaries: BoundaryInventory, path: &Path) -> Vec<BypassRule> {
    BYPASS_RULES
        .iter()
        .copied()
        .filter(|rule| boundaries.has(rule.kind) && !is_boundary_file(path, rule.kind))
        .collect()
}

fn collect_line_boundary_bypasses(
    file: &SourceFile,
    signals: &mut DriftSignals,
    rules: &[BypassRule],
    line: &str,
    line_number: usize,
) {
    let lowered = code_without_quoted_literals(strip_line_comment(line)).to_ascii_lowercase();

    for rule in rules {
        if contains_any(&lowered, rule.patterns) {
            push_bypass(signals, rule.kind, file, line_number, rule.occurrence_name);
        }
    }
}

fn push_bypass(
    signals: &mut DriftSignals,
    kind: BypassKind,
    file: &SourceFile,
    line: usize,
    name: &str,
) {
    signals.bypasses.entry(kind).or_default().push(Occurrence {
        path: file.display_path.clone(),
        line,
        name: Some(name.to_string()),
    });
}

include!("detections.rs");

mod analysis;
mod compatibility;

use analysis::*;
use compatibility::*;

#[cfg(test)]
#[path = "../../concept_drift_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../concept_drift_boundary_tests.rs"]
mod boundary_tests;
