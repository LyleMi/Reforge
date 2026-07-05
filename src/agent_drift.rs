use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::scanner::{Finding, FindingKind, RelatedLocation, Severity, is_test_source};
use crate::similar_functions::SourceFile;

#[derive(Debug, Clone)]
pub struct AgentDriftOptions {
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
    kind: FindingKind,
    severity: Severity,
    message: fn(&str, usize) -> String,
    require_cross_file: bool,
}

pub fn scan_agent_drift(files: &[SourceFile], options: &AgentDriftOptions) -> Vec<Finding> {
    let boundaries = boundary_inventory(files);
    let mut signals = DriftSignals::default();

    for file in files {
        collect_file_signals(file, options, boundaries, &mut signals);
    }

    let mut findings = Vec::new();
    findings.extend(parallel_implementation_findings(
        &signals.functions,
        options,
    ));
    findings.extend(shadowed_abstraction_findings(&signals.functions, options));
    findings.extend(duplicate_type_shape_findings(&signals.type_shapes, options));
    findings.extend(group_occurrences(
        signals.config_keys,
        OccurrenceGroupSpec {
            threshold: options.min_repeated_occurrences.max(2),
            kind: FindingKind::ConfigKeyDrift,
            severity: Severity::Info,
            message: config_key_drift_message,
            require_cross_file: true,
        },
    ));
    findings.extend(group_occurrences(
        signals.fixture_factories,
        OccurrenceGroupSpec {
            threshold: options.min_data_shape_occurrences.max(2),
            kind: FindingKind::FixtureFactoryDrift,
            severity: Severity::Info,
            message: fixture_factory_drift_message,
            require_cross_file: true,
        },
    ));
    findings.extend(generic_bucket_findings(
        &signals.generic_directories,
        &signals.generic_files,
        options,
    ));
    findings.extend(adapter_boundary_bypass_findings(&signals.bypasses));

    findings.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then(left.path.cmp(&right.path))
            .then(left.line.cmp(&right.line))
            .then(left.message.cmp(&right.message))
    });
    findings
}

fn collect_file_signals(
    file: &SourceFile,
    options: &AgentDriftOptions,
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
    }

    if !is_test {
        collect_boundary_bypasses(file, boundaries, signals);
    }
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
        let mut fields = BTreeSet::new();
        if braced {
            let mut depth = brace_delta(lines[index]);
            for field in field_names_from_line(lines[index]) {
                fields.insert(field);
            }
            let mut scan_index = index + 1;
            while scan_index < lines.len() {
                for field in field_names_from_line(lines[scan_index]) {
                    fields.insert(field);
                }
                depth += brace_delta(lines[scan_index]);
                scan_index += 1;
                if depth <= 0 {
                    break;
                }
            }
            index = scan_index;
        } else {
            let class_indent = leading_spaces(lines[index]);
            let mut scan_index = index + 1;
            while scan_index < lines.len() {
                let line = lines[scan_index];
                let trimmed = line.trim();
                if !trimmed.is_empty() && leading_spaces(line) <= class_indent {
                    break;
                }
                for field in field_names_from_line(line) {
                    fields.insert(field);
                }
                scan_index += 1;
            }
            index = scan_index;
        }

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
    let lowered = strip_line_comment(line).to_ascii_lowercase();

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

fn parallel_implementation_findings(
    functions: &[FunctionSignal],
    _options: &AgentDriftOptions,
) -> Vec<Finding> {
    let threshold = 2;
    let mut groups = BTreeMap::<String, Vec<Occurrence>>::new();

    for function in functions.iter().filter(|function| !function.is_test) {
        if !function
            .words
            .iter()
            .any(|word| PARALLEL_CAPABILITY_WORDS.contains(&word.as_str()))
        {
            continue;
        }

        let key = concept_key(&function.words, PARALLEL_STOP_WORDS, 4);
        if key.split(' ').count() < 2 {
            continue;
        }
        groups
            .entry(key)
            .or_default()
            .push(function.occurrence.clone());
    }

    groups_to_findings(
        groups,
        OccurrenceGroupSpec {
            threshold,
            kind: FindingKind::ParallelImplementation,
            severity: Severity::Warning,
            message: parallel_implementation_message,
            require_cross_file: true,
        },
    )
}

fn shadowed_abstraction_findings(
    functions: &[FunctionSignal],
    _options: &AgentDriftOptions,
) -> Vec<Finding> {
    let threshold = 2;
    let mut groups = BTreeMap::<String, Vec<Occurrence>>::new();

    for function in functions.iter().filter(|function| !function.is_test) {
        let has_helper_signal = function
            .file_words
            .iter()
            .any(|word| SHADOW_HELPER_WORDS.contains(&word.as_str()));
        if !has_helper_signal {
            continue;
        }

        let key = concept_key(&function.words, SHADOW_STOP_WORDS, 3);
        if key.is_empty() {
            continue;
        }
        groups
            .entry(key)
            .or_default()
            .push(function.occurrence.clone());
    }

    groups_to_findings(
        groups,
        OccurrenceGroupSpec {
            threshold,
            kind: FindingKind::ShadowedAbstraction,
            severity: Severity::Info,
            message: shadowed_abstraction_message,
            require_cross_file: true,
        },
    )
}

fn duplicate_type_shape_findings(
    shapes: &[TypeShape],
    options: &AgentDriftOptions,
) -> Vec<Finding> {
    let threshold = options.min_data_shape_occurrences.max(2);
    let mut ordered = shapes.to_vec();
    ordered.sort_by(|left, right| {
        left.occurrence
            .path
            .cmp(&right.occurrence.path)
            .then(left.occurrence.line.cmp(&right.occurrence.line))
    });

    let mut used = vec![false; ordered.len()];
    let mut findings = Vec::new();

    for index in 0..ordered.len() {
        if used[index] {
            continue;
        }

        let mut group = vec![ordered[index].clone()];
        for candidate_index in index + 1..ordered.len() {
            if used[candidate_index] {
                continue;
            }
            if field_overlap(&ordered[index].fields, &ordered[candidate_index].fields) >= 0.75 {
                group.push(ordered[candidate_index].clone());
            }
        }

        let unique_files = group
            .iter()
            .map(|shape| shape.occurrence.path.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        if group.len() < threshold || unique_files < 2 {
            continue;
        }

        for shape in &group {
            if let Some(position) = ordered.iter().position(|item| {
                item.occurrence.path == shape.occurrence.path
                    && item.occurrence.line == shape.occurrence.line
            }) {
                used[position] = true;
            }
        }

        let fields = shared_fields(&group);
        let representative = &group[0].occurrence;
        findings.push(Finding {
            kind: FindingKind::DuplicateTypeShape,
            severity: Severity::Info,
            path: representative.path.clone(),
            line: Some(representative.line),
            magnitude: Some(group.len()),
            message: format!(
                "{} type shapes share fields: {}",
                group.len(),
                fields.into_iter().take(6).collect::<Vec<_>>().join(", ")
            ),
            related_locations: group
                .iter()
                .map(|shape| related_location(&shape.occurrence))
                .collect(),
        });
    }

    findings
}

fn generic_bucket_findings(
    directories: &BTreeMap<PathBuf, GenericDirectory>,
    generic_files: &[(String, Occurrence)],
    options: &AgentDriftOptions,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let concept_threshold = (options.max_dir_files / 4).clamp(4, 12);

    for directory in directories.values() {
        if directory.files.len() < 3 || directory.concepts.len() < concept_threshold {
            continue;
        }

        findings.push(Finding {
            kind: FindingKind::GenericBucketDrift,
            severity: Severity::Info,
            path: directory.display_path.clone(),
            line: None,
            magnitude: Some(directory.concepts.len()),
            message: format!(
                "generic bucket mixes {} source concepts across {} files",
                directory.concepts.len(),
                directory.files.len()
            ),
            related_locations: directory
                .files
                .iter()
                .map(|path| RelatedLocation {
                    path: path.clone(),
                    line: 1,
                    name: None,
                })
                .collect(),
        });
    }

    for (concepts, occurrence) in generic_files {
        findings.push(Finding {
            kind: FindingKind::GenericBucketDrift,
            severity: Severity::Info,
            path: occurrence.path.clone(),
            line: Some(occurrence.line),
            magnitude: Some(concepts.split(", ").count()),
            message: format!("generic file accumulates unrelated concepts: {concepts}"),
            related_locations: vec![related_location(occurrence)],
        });
    }

    findings
}

fn adapter_boundary_bypass_findings(
    bypasses: &BTreeMap<BypassKind, Vec<Occurrence>>,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for (kind, occurrences) in bypasses {
        let mut group = occurrences.clone();
        group.sort_by(|left, right| left.path.cmp(&right.path).then(left.line.cmp(&right.line)));
        group.dedup_by(|left, right| left.path == right.path && left.line == right.line);

        let unique_files = group
            .iter()
            .map(|occurrence| occurrence.path.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        if group.len() < 2 || unique_files < 2 {
            continue;
        }

        let representative = &group[0];
        findings.push(Finding {
            kind: FindingKind::AdapterBoundaryBypass,
            severity: Severity::Warning,
            path: representative.path.clone(),
            line: Some(representative.line),
            magnitude: Some(group.len()),
            message: format!(
                "{} direct {} calls bypass existing boundary files",
                group.len(),
                kind.label()
            ),
            related_locations: group.iter().map(related_location).collect(),
        });
    }

    findings
}

fn group_occurrences(
    occurrences: Vec<(String, Occurrence)>,
    spec: OccurrenceGroupSpec,
) -> Vec<Finding> {
    let mut groups = BTreeMap::<String, Vec<Occurrence>>::new();
    for (key, occurrence) in occurrences {
        groups.entry(key).or_default().push(occurrence);
    }

    groups_to_findings(groups, spec)
}

fn groups_to_findings(
    groups: BTreeMap<String, Vec<Occurrence>>,
    spec: OccurrenceGroupSpec,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for (key, mut group) in groups {
        group.sort_by(|left, right| left.path.cmp(&right.path).then(left.line.cmp(&right.line)));
        group.dedup_by(|left, right| {
            left.path == right.path && left.line == right.line && left.name == right.name
        });

        let unique_files = group
            .iter()
            .map(|occurrence| occurrence.path.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        if group.len() < spec.threshold || (spec.require_cross_file && unique_files < 2) {
            continue;
        }

        let representative = &group[0];
        findings.push(Finding {
            kind: spec.kind,
            severity: spec.severity,
            path: representative.path.clone(),
            line: Some(representative.line),
            magnitude: Some(group.len()),
            message: (spec.message)(&key, group.len()),
            related_locations: group.iter().map(related_location).collect(),
        });
    }

    findings
}

include!("agent_drift_analysis.rs");
