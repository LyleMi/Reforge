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

#[derive(Debug, Clone)]
struct Occurrence {
    path: String,
    line: usize,
    name: Option<String>,
}

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
        options.min_repeated_occurrences.max(2),
        FindingKind::ConfigKeyDrift,
        Severity::Info,
        |key, count| format!("config or route key {key:?} appears in {count} locations"),
        true,
    ));
    findings.extend(group_occurrences(
        signals.fixture_factories,
        options.min_data_shape_occurrences.max(2),
        FindingKind::FixtureFactoryDrift,
        Severity::Info,
        |key, count| format!("test fixture factory concept `{key}` appears in {count} locations"),
        true,
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
    for (index, line) in file.source.lines().enumerate() {
        let line_number = index + 1;
        let stripped = strip_line_comment(line);
        let lowered = stripped.to_ascii_lowercase();

        if boundaries.http
            && !is_boundary_file(&file.path, BypassKind::Http)
            && contains_any(
                &lowered,
                &[
                    "fetch(",
                    "axios.",
                    "axios(",
                    "requests.get(",
                    "requests.post(",
                    "reqwest::",
                    "hyper::",
                    "http.client",
                ],
            )
        {
            push_bypass(
                signals,
                BypassKind::Http,
                file,
                line_number,
                "direct HTTP call",
            );
        }

        if boundaries.config
            && !is_boundary_file(&file.path, BypassKind::Config)
            && contains_any(
                &lowered,
                &[
                    "process.env",
                    "std::env::var",
                    "env::var(",
                    "os.environ",
                    "os.getenv(",
                    "getenv(",
                ],
            )
        {
            push_bypass(
                signals,
                BypassKind::Config,
                file,
                line_number,
                "direct config read",
            );
        }

        if boundaries.filesystem
            && !is_boundary_file(&file.path, BypassKind::Filesystem)
            && contains_any(
                &lowered,
                &[
                    "fs.readfile",
                    "fs.writfile",
                    "fs.writefile",
                    "std::fs::read",
                    "std::fs::write",
                    "file::open",
                    "read_to_string",
                ],
            )
        {
            push_bypass(
                signals,
                BypassKind::Filesystem,
                file,
                line_number,
                "direct filesystem call",
            );
        }

        if boundaries.logging
            && !is_boundary_file(&file.path, BypassKind::Logging)
            && contains_any(
                &lowered,
                &[
                    "console.log(",
                    "println!(",
                    "dbg!(",
                    "print(",
                    "log.printf(",
                    "log.println(",
                ],
            )
        {
            push_bypass(
                signals,
                BypassKind::Logging,
                file,
                line_number,
                "direct log call",
            );
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
        threshold,
        FindingKind::ParallelImplementation,
        Severity::Warning,
        |key, count| format!("capability `{key}` has {count} parallel-looking implementations"),
        true,
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
        threshold,
        FindingKind::ShadowedAbstraction,
        Severity::Info,
        |key, count| format!("helper abstraction `{key}` is shadowed in {count} locations"),
        true,
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
    threshold: usize,
    kind: FindingKind,
    severity: Severity,
    message: impl Fn(&str, usize) -> String,
    require_cross_file: bool,
) -> Vec<Finding> {
    let mut groups = BTreeMap::<String, Vec<Occurrence>>::new();
    for (key, occurrence) in occurrences {
        groups.entry(key).or_default().push(occurrence);
    }

    groups_to_findings(
        groups,
        threshold,
        kind,
        severity,
        message,
        require_cross_file,
    )
}

fn groups_to_findings(
    groups: BTreeMap<String, Vec<Occurrence>>,
    threshold: usize,
    kind: FindingKind,
    severity: Severity,
    message: impl Fn(&str, usize) -> String,
    require_cross_file: bool,
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
        if group.len() < threshold || (require_cross_file && unique_files < 2) {
            continue;
        }

        let representative = &group[0];
        findings.push(Finding {
            kind,
            severity,
            path: representative.path.clone(),
            line: Some(representative.line),
            magnitude: Some(group.len()),
            message: message(&key, group.len()),
            related_locations: group.iter().map(related_location).collect(),
        });
    }

    findings
}

fn boundary_inventory(files: &[SourceFile]) -> BoundaryInventory {
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

fn is_boundary_file(path: &Path, kind: BypassKind) -> bool {
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

impl BypassKind {
    fn label(self) -> &'static str {
        match self {
            BypassKind::Http => "HTTP",
            BypassKind::Config => "config",
            BypassKind::Filesystem => "filesystem",
            BypassKind::Logging => "logging",
        }
    }
}

fn function_or_class_name(line: &str) -> Option<String> {
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

fn type_start(line: &str) -> Option<(String, bool)> {
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

fn field_names_from_line(line: &str) -> Vec<String> {
    strip_line_comment(line)
        .split([',', ';'])
        .filter_map(field_name_from_segment)
        .collect()
}

fn field_name_from_segment(segment: &str) -> Option<String> {
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

fn identifier_before(rest: &str, delimiter: char) -> Option<String> {
    rest.split(delimiter)
        .next()
        .map(str::trim)
        .filter(|name| is_valid_identifier(name))
        .map(ToString::to_string)
}

fn identifier_before_any(rest: &str, delimiters: &[char]) -> Option<String> {
    let position = rest
        .char_indices()
        .find_map(|(index, character)| delimiters.contains(&character).then_some(index))
        .unwrap_or(rest.len());
    let name = rest[..position].trim();
    is_valid_identifier(name).then(|| name.to_string())
}

fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && chars.all(is_identifier_char)
}

fn is_identifier_char(character: char) -> bool {
    character == '_' || character == '-' || character.is_ascii_alphanumeric()
}

fn is_valid_field_name(name: &str) -> bool {
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

fn string_literals(line: &str) -> Vec<String> {
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

fn constant_keys(line: &str) -> Vec<String> {
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

fn is_config_key(value: &str) -> bool {
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

fn field_overlap(left: &BTreeSet<String>, right: &BTreeSet<String>) -> f64 {
    let common = left.intersection(right).count();
    if common < 3 {
        return 0.0;
    }
    common as f64 / left.len().max(right.len()) as f64
}

fn shared_fields(group: &[TypeShape]) -> Vec<String> {
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

fn concept_key(words: &[String], stop_words: &[&str], max_words: usize) -> String {
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

fn split_identifier_words(identifier: &str) -> Vec<String> {
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

fn normalize_word(word: &str) -> String {
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

fn is_useful_concept_word(word: &str) -> bool {
    word.len() > 2
        && !STOP_WORDS.contains(&word)
        && !word.chars().all(|character| character.is_ascii_digit())
}

fn path_words(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .flat_map(path_component_words_from_str)
        .collect()
}

fn path_component_words(path: &Path) -> Vec<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(path_component_words_from_str)
        .unwrap_or_default()
}

fn path_component_words_from_str(component: &str) -> Vec<String> {
    let without_extension = component
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(component);
    split_identifier_words(without_extension)
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn strip_line_comment(line: &str) -> &str {
    let slash = line.find("//");
    let hash = line.find('#');
    match (slash, hash) {
        (Some(left), Some(right)) => &line[..left.min(right)],
        (Some(index), None) | (None, Some(index)) => &line[..index],
        (None, None) => line,
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn leading_spaces(line: &str) -> usize {
    line.chars()
        .take_while(|character| character.is_ascii_whitespace())
        .count()
}

fn brace_delta(line: &str) -> isize {
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

fn related_location(occurrence: &Occurrence) -> RelatedLocation {
    RelatedLocation {
        path: occurrence.path.clone(),
        line: occurrence.line,
        name: occurrence.name.clone(),
    }
}

const PARALLEL_CAPABILITY_WORDS: &[&str] = &[
    "adapt",
    "adapter",
    "build",
    "cache",
    "client",
    "config",
    "load",
    "logger",
    "map",
    "normalize",
    "parse",
    "retry",
    "validate",
];

const PARALLEL_STOP_WORDS: &[&str] = &[
    "a", "and", "do", "for", "from", "get", "has", "is", "make", "new", "of", "set", "the", "to",
    "with",
];

const SHADOW_HELPER_WORDS: &[&str] = &[
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
    "factory",
];

const SHADOW_STOP_WORDS: &[&str] = &[
    "common", "helper", "helpers", "shared", "util", "utils", "factory", "make", "create", "build",
    "get", "set",
];

const FIXTURE_WORDS: &[&str] = &[
    "builder", "dummy", "factory", "fake", "fixture", "mock", "sample", "setup", "test",
];

const GENERIC_BUCKET_WORDS: &[&str] = &[
    "common", "helper", "helpers", "lib", "misc", "shared", "util", "utils",
];

const STOP_WORDS: &[&str] = &[
    "api", "app", "cmd", "for", "from", "get", "has", "impl", "index", "main", "mod", "new", "old",
    "src", "test", "tests", "the", "this", "type", "use", "with",
];

const CONFIG_KEY_WORDS: &[&str] = &[
    "api", "auth", "base", "client", "code", "config", "database", "db", "endpoint", "env",
    "error", "host", "key", "path", "port", "route", "secret", "service", "token", "url",
];

const HTTP_BOUNDARY_WORDS: &[&str] = &[
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

const CONFIG_BOUNDARY_WORDS: &[&str] = &["config", "configuration", "env", "setting", "settings"];

const FS_BOUNDARY_WORDS: &[&str] = &[
    "dao",
    "file",
    "filesystem",
    "persistence",
    "repository",
    "storage",
    "store",
];

const LOG_BOUNDARY_WORDS: &[&str] = &["log", "logger", "logging", "telemetry", "trace", "tracing"];

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn source_file(path: &str, source: &str) -> SourceFile {
        SourceFile {
            path: PathBuf::from(path),
            display_path: path.to_string(),
            source: source.to_string(),
        }
    }

    fn options() -> AgentDriftOptions {
        AgentDriftOptions {
            min_repeated_occurrences: 3,
            min_data_shape_occurrences: 2,
            max_dir_files: 16,
            include_test_structure: false,
        }
    }

    fn has_kind(findings: &[Finding], kind: FindingKind) -> bool {
        findings.iter().any(|finding| finding.kind == kind)
    }

    #[test]
    fn detects_parallel_implementations_and_shadowed_helpers() {
        let files = vec![
            source_file(
                "src/feature_a/helpers.ts",
                "export function normalizePattern(input: string) { return input.trim().toLowerCase(); }",
            ),
            source_file(
                "src/feature_b/helpers.ts",
                "export function normalizePattern(value: string) { return value.trim().toLowerCase(); }",
            ),
        ];

        let findings = scan_agent_drift(&files, &options());

        let parallel = findings
            .iter()
            .find(|finding| finding.kind == FindingKind::ParallelImplementation)
            .expect("parallel implementation finding");
        assert_eq!(parallel.magnitude, Some(2));
        assert_eq!(parallel.related_locations.len(), 2);
        assert!(has_kind(&findings, FindingKind::ShadowedAbstraction));
    }

    #[test]
    fn ignores_function_like_text_inside_string_literals() {
        let files = vec![
            source_file(
                "src/examples.rs",
                r#"
fn example() {
    let source = "export function normalizePattern(input: string) { return input; }";
}
"#,
            ),
            source_file(
                "src/normalizer.rs",
                "fn normalize_pattern(input: &str) -> &str { input }",
            ),
        ];

        let findings = scan_agent_drift(&files, &options());

        assert!(
            findings
                .iter()
                .all(|finding| finding.kind != FindingKind::ParallelImplementation),
            "{findings:#?}"
        );
    }

    #[test]
    fn reports_two_cross_file_parallel_implementations_at_default_thresholds() {
        let files = vec![
            source_file(
                "src/similar_functions.rs",
                "fn adapter_for_path(path: &Path) -> Option<LanguageAdapter> { None }",
            ),
            source_file(
                "src/structural.rs",
                "fn adapter_for_path(path: &Path) -> Option<LanguageAdapter> { None }",
            ),
        ];
        let mut opts = options();
        opts.min_repeated_occurrences = 4;

        let findings = scan_agent_drift(&files, &opts);

        let finding = findings
            .iter()
            .find(|finding| finding.kind == FindingKind::ParallelImplementation)
            .expect("parallel implementation finding");
        assert_eq!(finding.magnitude, Some(2));
        assert_eq!(finding.related_locations.len(), 2);
        assert!(
            findings
                .iter()
                .all(|finding| finding.kind != FindingKind::ShadowedAbstraction),
            "{findings:#?}"
        );
    }

    #[test]
    fn detects_duplicate_type_shapes() {
        let files = vec![
            source_file(
                "src/api/user.ts",
                "interface UserPayload {\n  id: string;\n  email: string;\n  name: string;\n  status: string;\n}",
            ),
            source_file(
                "src/jobs/user.rs",
                "struct UserRecord {\n    id: String,\n    email: String,\n    name: String,\n    status: String,\n}",
            ),
        ];

        let findings = scan_agent_drift(&files, &options());

        let finding = findings
            .iter()
            .find(|finding| finding.kind == FindingKind::DuplicateTypeShape)
            .expect("duplicate type shape finding");
        assert_eq!(finding.magnitude, Some(2));
        assert!(finding.message.contains("email"));
    }

    #[test]
    fn detects_duplicate_single_line_type_shapes() {
        let files = vec![
            source_file(
                "src/api/user.ts",
                "interface UserPayload { id: string; email: string; name: string; status: string }",
            ),
            source_file(
                "src/jobs/user.rs",
                "struct UserRecord { id: String, email: String, name: String, status: String }",
            ),
        ];

        let findings = scan_agent_drift(&files, &options());

        let finding = findings
            .iter()
            .find(|finding| finding.kind == FindingKind::DuplicateTypeShape)
            .expect("duplicate type shape finding");
        assert_eq!(finding.magnitude, Some(2));
        assert!(finding.message.contains("email"));
    }

    #[test]
    fn detects_config_key_drift() {
        let files = vec![
            source_file(
                "src/auth.ts",
                "const AUTH_TOKEN_URL = \"AUTH_TOKEN_URL\";\nconst route = \"/api/login\";",
            ),
            source_file(
                "src/client.ts",
                "const tokenUrl = process.env.AUTH_TOKEN_URL;\nfetch(\"/api/login\");",
            ),
            source_file(
                "src/job.ts",
                "let key = \"AUTH_TOKEN_URL\";\nlet route = \"/api/login\";",
            ),
        ];

        let findings = scan_agent_drift(&files, &options());

        let finding = findings
            .iter()
            .find(|finding| finding.kind == FindingKind::ConfigKeyDrift)
            .expect("config key drift finding");
        assert_eq!(finding.magnitude, Some(3));
        assert!(finding.related_locations.len() >= 3);
    }

    #[test]
    fn ignores_config_keys_inside_comments() {
        let files = vec![
            source_file("src/auth.ts", "// const token = \"AUTH_TOKEN_URL\";"),
            source_file("src/client.py", "# route = \"/api/login\""),
            source_file("src/job.ts", "// fetch(\"/api/login\");"),
        ];

        let findings = scan_agent_drift(&files, &options());

        assert!(
            findings
                .iter()
                .all(|finding| finding.kind != FindingKind::ConfigKeyDrift),
            "{findings:#?}"
        );
    }

    #[test]
    fn detects_fixture_factory_drift_in_tests() {
        let files = vec![
            source_file(
                "tests/user_a.test.ts",
                "function makeUserFixture() { return { id: \"1\" }; }",
            ),
            source_file(
                "tests/user_b.test.ts",
                "function makeUserFixture() { return { id: \"2\" }; }",
            ),
        ];

        let findings = scan_agent_drift(&files, &options());

        let finding = findings
            .iter()
            .find(|finding| finding.kind == FindingKind::FixtureFactoryDrift)
            .expect("fixture factory drift finding");
        assert_eq!(finding.magnitude, Some(2));
        assert_eq!(finding.related_locations.len(), 2);
    }

    #[test]
    fn detects_generic_bucket_directories() {
        let files = vec![
            source_file(
                "src/utils/auth_token.ts",
                "export function parseAuthToken() {}",
            ),
            source_file(
                "src/utils/cache_store.ts",
                "export function buildCacheStore() {}",
            ),
            source_file(
                "src/utils/retry_policy.ts",
                "export function validateRetryPolicy() {}",
            ),
            source_file(
                "src/utils/route_mapper.ts",
                "export function mapRoutePattern() {}",
            ),
        ];

        let findings = scan_agent_drift(&files, &options());

        let finding = findings
            .iter()
            .find(|finding| finding.kind == FindingKind::GenericBucketDrift)
            .expect("generic bucket finding");
        assert_eq!(finding.path, "src/utils");
        assert!(finding.magnitude.unwrap_or_default() >= 4);
        assert_eq!(finding.related_locations.len(), 4);
    }

    #[test]
    fn skips_generic_bucket_drift_in_tests_by_default() {
        let files = vec![
            source_file(
                "tests/utils/auth_token.ts",
                "export function parseAuthToken() {}",
            ),
            source_file(
                "tests/utils/cache_store.ts",
                "export function buildCacheStore() {}",
            ),
            source_file(
                "tests/utils/retry_policy.ts",
                "export function validateRetryPolicy() {}",
            ),
            source_file(
                "tests/utils/route_mapper.ts",
                "export function mapRoutePattern() {}",
            ),
        ];

        let default_findings = scan_agent_drift(&files, &options());
        let mut included_options = options();
        included_options.include_test_structure = true;
        let included_findings = scan_agent_drift(&files, &included_options);

        assert!(
            default_findings
                .iter()
                .all(|finding| finding.kind != FindingKind::GenericBucketDrift),
            "{default_findings:#?}"
        );
        assert!(has_kind(
            &included_findings,
            FindingKind::GenericBucketDrift
        ));
    }

    #[test]
    fn detects_adapter_boundary_bypasses_when_boundary_exists() {
        let files = vec![
            source_file(
                "src/http/client.ts",
                "export function request() { return fetch('/api/users'); }",
            ),
            source_file(
                "src/features/users.ts",
                "export function loadUsers() { return fetch('/api/users'); }",
            ),
            source_file(
                "src/jobs/sync.ts",
                "export function syncUsers() { return axios.get('/api/users'); }",
            ),
        ];

        let findings = scan_agent_drift(&files, &options());

        let finding = findings
            .iter()
            .find(|finding| finding.kind == FindingKind::AdapterBoundaryBypass)
            .expect("adapter bypass finding");
        assert_eq!(finding.magnitude, Some(2));
        assert_eq!(finding.related_locations.len(), 2);
    }
}
