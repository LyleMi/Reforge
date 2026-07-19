struct UnityFindingInput {
    kind: FindingKind,
    path: String,
    line: usize,
    message: String,
    value: usize,
    threshold: usize,
    related: Vec<RelatedLocation>,
    reliability: f64,
}

fn package_analysis_roots(packages: &Path) -> Result<Vec<PathBuf>> {
    let mut roots = embedded_package_roots(packages)?;
    roots.extend(local_package_roots(packages));
    Ok(roots)
}

fn embedded_package_roots(packages: &Path) -> Result<Vec<PathBuf>> {
    if !packages.is_dir() {
        return Ok(Vec::new());
    }
    fs::read_dir(packages)?
        .map(|entry| entry.map(|entry| entry.path()))
        .filter(|path| path.as_ref().is_ok_and(|path| path.is_dir()))
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn local_package_roots(packages: &Path) -> Vec<PathBuf> {
    let dependencies = fs::read_to_string(packages.join("manifest.json"))
        .ok()
        .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
        .and_then(|value| value.get("dependencies").and_then(|value| value.as_object()).cloned())
        .unwrap_or_default();
    dependencies
        .into_values()
        .filter_map(|value| value.as_str().and_then(|value| value.strip_prefix("file:")).map(str::to_string))
        .map(|relative| packages.join(relative))
        .map(|path| path.canonicalize().unwrap_or(path))
        .filter(|path| path.is_dir())
        .collect()
}

fn included_build_scenes(build_settings: &str) -> BTreeSet<String> {
    let mut included = BTreeSet::new();
    let mut enabled = None;
    for line in build_settings.lines() {
        let trimmed = line.trim().trim_start_matches("- ");
        match (
            trimmed.strip_prefix("enabled:"),
            trimmed.strip_prefix("path:"),
        ) {
            (Some(value), _) => enabled = Some(value.trim() == "1"),
            (_, Some(path)) => {
                if enabled.unwrap_or(true) {
                    included.insert(path.trim().to_string());
                }
                enabled = None;
            }
            _ => {}
        }
    }
    included
}

struct CSharpRecord {
    path: PathBuf,
    source: String,
    declared_types: Vec<String>,
}

fn load_csharp_records(
    files: &[PathBuf],
) -> (BTreeMap<String, String>, Vec<CSharpRecord>) {
    let mut base_by_type = BTreeMap::new();
    let records = files
        .iter()
        .map(|path| {
            let source = fs::read_to_string(path).unwrap_or_default();
            let declarations = class_declarations(&source);
            base_by_type.extend(declarations.iter().cloned());
            CSharpRecord {
                path: path.clone(),
                source,
                declared_types: declarations.into_iter().map(|(name, _)| name).collect(),
            }
        })
        .collect();
    (base_by_type, records)
}

fn is_editor_path(path: &str) -> bool {
    path.split('/')
        .any(|part| part.eq_ignore_ascii_case("editor"))
}

fn contains_test_attribute(source: &str) -> bool {
    source.contains("[UnityTest]") || source.contains("[Test]")
}

impl UnityFindingInput {
    fn new(
        kind: FindingKind,
        path: String,
        line: usize,
        message: String,
        metric: (usize, usize),
    ) -> Self {
        Self {
            kind,
            path,
            line,
            message,
            value: metric.0,
            threshold: metric.1,
            related: Vec::new(),
            reliability: 1.0,
        }
    }
    fn with_related(mut self, related: Vec<RelatedLocation>) -> Self {
        self.related = related;
        self
    }
    fn with_reliability(mut self, reliability: f64) -> Self {
        self.reliability = reliability;
        self
    }
}

fn unity_finding(input: UnityFindingInput) -> Finding {
    Finding::from(
        FindingInput::new(
            input.kind,
            input.path,
            Some(input.line),
            input.message,
            vec![FindingMetric::threshold(
                MetricId::GroupSize,
                input.value,
                input.threshold,
                "items",
            )],
        )
        .with_related_locations(input.related)
        .with_detection_reliability(input.reliability),
    )
}

fn collect_files(root: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory)
            .with_context(|| format!("failed to read Unity directory {}", directory.display()))?
        {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                output.push(path);
            }
        }
    }
    Ok(())
}

fn unity_metric_manifest() -> Vec<UnityMetricManifestEntry> {
    vec![
        UnityMetricManifestEntry {
            name: "unity.assembly.dependencies".into(),
            entity: "assembly".into(),
            unit: "assemblies".into(),
            description: "Resolved and declared direct asmdef dependencies.".into(),
        },
        UnityMetricManifestEntry {
            name: "unity.asset.objects".into(),
            entity: "scene_or_prefab".into(),
            unit: "objects".into(),
            description: "Serialized Unity YAML object records.".into(),
        },
        UnityMetricManifestEntry {
            name: "unity.type.serialized_fields".into(),
            entity: "type".into(),
            unit: "fields".into(),
            description: "Unity-serializable fields on MonoBehaviour and ScriptableObject types."
                .into(),
        },
        UnityMetricManifestEntry {
            name: "unity.type.lifecycle_methods".into(),
            entity: "type".into(),
            unit: "methods".into(),
            description: "Implemented Unity lifecycle methods.".into(),
        },
    ]
}

fn extension(path: &Path) -> Option<&str> {
    path.extension().and_then(|value| value.to_str())
}

fn is_scannable_unity_asset(path: &Path) -> bool {
    !matches!(extension(path), Some("meta" | "asmdef" | "cs")) && path.is_file()
}
fn is_predefined_assembly(reference: &str) -> bool {
    reference.starts_with("UnityEngine")
        || reference.starts_with("UnityEditor")
        || reference.starts_with("Unity.")
        || reference.starts_with("System")
        || reference.starts_with("Microsoft")
        || reference.starts_with("nunit")
        || matches!(reference, "Assembly-CSharp" | "Assembly-CSharp-Editor")
}

fn load_assembly_record(root: &Path, path: &Path) -> Option<AssemblyRecord> {
    let contents = fs::read_to_string(path).ok()?;
    let asmdef = serde_json::from_str::<AsmdefFile>(&contents).ok()?;
    if asmdef.name.is_empty() {
        return None;
    }
    let display = display_path(root, path);
    let editor_only = asmdef
        .include_platforms
        .iter()
        .any(|platform| platform.eq_ignore_ascii_case("editor"))
        || is_editor_path(&display);
    let test_assembly = asmdef
        .optional_unity_references
        .iter()
        .any(|reference| reference == "TestAssemblies")
        || display.contains("/Tests/");
    let guid = fs::read_to_string(format!("{}.meta", path.display()))
        .ok()
        .and_then(|contents| meta_guid(&contents));
    Some(AssemblyRecord {
        node: UnityAssemblyNode {
            name: asmdef.name,
            path: display,
            editor_only,
            test_assembly,
            predefined: false,
        },
        references: asmdef.references,
        guid,
    })
}

fn assembly_edges(assemblies: &[AssemblyRecord]) -> Vec<UnityAssemblyEdge> {
    let name_index = assemblies
        .iter()
        .map(|assembly| (assembly.node.name.clone(), assembly.node.name.clone()))
        .collect::<BTreeMap<_, _>>();
    let guid_index = assemblies
        .iter()
        .filter_map(|assembly| {
            assembly
                .guid
                .as_ref()
                .map(|guid| (guid.clone(), assembly.node.name.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    assemblies
        .iter()
        .flat_map(|assembly| {
            assembly.references.iter().map(|reference| {
                let target = reference
                    .strip_prefix("GUID:")
                    .and_then(|guid| guid_index.get(&guid.to_ascii_lowercase()))
                    .or_else(|| name_index.get(reference));
                UnityAssemblyEdge {
                    from: assembly.node.name.clone(),
                    to: target.cloned().unwrap_or_else(|| reference.clone()),
                    reference: reference.clone(),
                    resolved: target.is_some() || is_predefined_assembly(reference),
                }
            })
        })
        .collect()
}

fn local_assembly_names(assemblies: &[AssemblyRecord]) -> BTreeSet<String> {
    assemblies
        .iter()
        .filter(|assembly| !assembly.node.path.starts_with("Library/PackageCache/"))
        .map(|assembly| assembly.node.name.clone())
        .collect()
}

fn assembly_cycles(edges: &[UnityAssemblyEdge]) -> Vec<Vec<String>> {
    let adjacency = edges.iter().filter(|edge| edge.resolved).fold(
        BTreeMap::<String, Vec<String>>::new(),
        |mut map, edge| {
            map.entry(edge.from.clone())
                .or_default()
                .push(edge.to.clone());
            map
        },
    );
    let mut collector = AssemblyCycleCollector {
        adjacency: &adjacency,
        emitted: BTreeSet::new(),
        cycles: Vec::new(),
    };
    for start in adjacency.keys() {
        collector.visit(start, start, vec![start.clone()]);
    }
    collector.cycles
}

struct AssemblyCycleCollector<'a> {
    adjacency: &'a BTreeMap<String, Vec<String>>,
    emitted: BTreeSet<String>,
    cycles: Vec<Vec<String>>,
}

impl AssemblyCycleCollector<'_> {
    fn visit(&mut self, start: &str, node: &str, path: Vec<String>) {
        let next_nodes = self.adjacency.get(node).cloned().unwrap_or_default();
        for next in next_nodes {
            if next == start && path.len() > 1 {
                let mut members = path.clone();
                members.sort();
                members.dedup();
                if self.emitted.insert(members.join("|")) {
                    self.cycles.push(members);
                }
            } else if !path.contains(&next) && path.len() <= self.adjacency.len() {
                let mut extended = path.clone();
                extended.push(next.clone());
                self.visit(start, &next, extended);
            }
        }
    }
}

fn assembly_cycle_finding(members: &[String], assemblies: &[AssemblyRecord]) -> Finding {
    let assembly_path = |name: &str| {
        assemblies
            .iter()
            .find(|assembly| assembly.node.name == name)
            .map(|assembly| assembly.node.path.clone())
            .unwrap_or_else(|| name.to_string())
    };
    let related = members
        .iter()
        .map(|name| RelatedLocation {
            path: assembly_path(name),
            line: 1,
            name: Some(name.clone()),
        })
        .collect();
    unity_finding(
        UnityFindingInput::new(
            FindingKind::UnityAssemblyCycle,
            assembly_path(&members[0]),
            1,
            format!("Unity assembly cycle spans {} assemblies", members.len()), (
            members.len(),
            2))
        .with_related(related),
    )
}
fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
fn meta_guid(contents: &str) -> Option<String> {
    contents
        .lines()
        .find_map(|line| line.trim().strip_prefix("guid:").map(str::trim))
        .filter(|guid| guid.len() == 32 && guid.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
}
fn guids_in_line(line: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut rest = line;
    while let Some(index) = rest.find("guid:") {
        rest = &rest[index + 5..];
        let guid = rest
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect::<String>();
        if guid.len() == 32 {
            output.push(guid.to_ascii_lowercase());
        }
        rest = rest.get(guid.len()..).unwrap_or_default();
    }
    output
}
fn file_id_in_line(line: &str) -> Option<String> {
    let rest = line.split_once("fileID:")?.1.trim_start();
    let value = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect::<String>();
    (!value.is_empty()).then_some(value)
}

fn class_declarations(source: &str) -> Vec<(String, String)> {
    source
        .lines()
        .filter_map(|line| {
            let tokens = line
                .replace(['{', ','], " ")
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>();
            let index = tokens.iter().position(|token| token == "class")?;
            let name = tokens.get(index + 1)?.trim_matches(':').to_string();
            let base = tokens
                .iter()
                .position(|token| token == ":")
                .and_then(|colon| tokens.get(colon + 1))
                .cloned()
                .or_else(|| {
                    line.split_once(':').and_then(|(_, rest)| {
                        rest.split(|c: char| c == ',' || c == '{' || c.is_whitespace())
                            .find(|value| !value.is_empty())
                            .map(str::to_string)
                    })
                })
                .unwrap_or_default();
            Some((name, base.trim().to_string()))
        })
        .collect()
}

fn inherits_unity(
    name: &str,
    bases: &BTreeMap<String, String>,
    visiting: &mut BTreeSet<String>,
) -> bool {
    if name == "MonoBehaviour" || name == "ScriptableObject" {
        return true;
    }
    if !visiting.insert(name.to_string()) {
        return false;
    }
    let Some(base) = bases.get(name) else {
        return false;
    };
    base.ends_with("MonoBehaviour")
        || base.ends_with("ScriptableObject")
        || inherits_unity(base, bases, visiting)
}

fn serialized_field_count(source: &str) -> usize {
    let mut serialized_attribute = false;
    let mut count = 0;
    for line in source.lines() {
        let classification = classify_serialized_field(line.trim(), serialized_attribute);
        count += usize::from(classification.counted);
        serialized_attribute = classification.next_attribute;
    }
    count
}

struct SerializedFieldClassification {
    counted: bool,
    next_attribute: bool,
}

fn classify_serialized_field(line: &str, prior_attribute: bool) -> SerializedFieldClassification {
    let has_attribute = prior_attribute
        || line.contains("[SerializeField]")
        || line.contains("[SerializeReference]");
    let declaration = line.ends_with(';') && !line.contains('(');
    let excluded = line.contains(" const ")
        || line.starts_with("const ")
        || line.contains(" static ")
        || line.starts_with("static ");
    let non_serialized = line.contains("[NonSerialized]");
    SerializedFieldClassification {
        counted: declaration
            && !excluded
            && !non_serialized
            && (line.starts_with("public ") || has_attribute),
        next_attribute: has_attribute && !declaration && !non_serialized,
    }
}

fn lifecycle_method_count(source: &str) -> usize {
    LIFECYCLE_METHODS
        .iter()
        .filter(|name| {
            source.lines().any(|line| {
                line.contains(&format!(" {name}("))
                    || line.trim_start().starts_with(&format!("{name}("))
            })
        })
        .count()
}

fn scan_frame_calls(source: &str, path: &str, findings: &mut Vec<Finding>) {
    let methods = csharp_methods(source);
    for name in reachable_frame_methods(&methods) {
        let Some((start_line, body)) = methods.get(&name) else {
            continue;
        };
        findings.extend(expensive_frame_call_findings(path, &name, *start_line, body));
    }
}

fn reachable_frame_methods(methods: &BTreeMap<String, (usize, String)>) -> BTreeSet<String> {
    let mut reachable = ["Update", "FixedUpdate", "LateUpdate"]
        .into_iter()
        .filter(|name| methods.contains_key(*name))
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let mut pending = reachable.iter().cloned().collect::<Vec<_>>();
    while let Some(name) = pending.pop() {
        let Some((_, body)) = methods.get(&name) else {
            continue;
        };
        for candidate in called_local_methods(body, methods) {
            if reachable.insert(candidate.clone()) {
                pending.push(candidate.clone());
            }
        }
    }
    reachable
}

fn called_local_methods<'a>(
    body: &'a str,
    methods: &'a BTreeMap<String, (usize, String)>,
) -> impl Iterator<Item = &'a String> {
    methods
        .keys()
        .filter(|candidate| body.contains(&format!("{candidate}(")))
}

fn expensive_frame_call_findings(
    path: &str,
    method_name: &str,
    start_line: usize,
    body: &str,
) -> Vec<Finding> {
    body.lines()
        .enumerate()
        .filter_map(|(offset, text)| {
            let expensive = contains_expensive_lookup(text);
            let component = text.contains("GetComponent") || text.contains("TryGetComponent");
            (expensive || component).then(|| {
                unity_finding(
                    UnityFindingInput::new(
                        FindingKind::UnityExpensiveFrameCall,
                        path.into(),
                        start_line + offset,
                        format!("Unity frame-loop call path through {method_name} performs a repeated object or resource lookup"), (
                        1,
                        1))
                    .with_reliability(if expensive { 0.9 } else { 0.7 }),
                )
            })
        })
        .collect()
}

fn contains_expensive_lookup(text: &str) -> bool {
    [
        "GameObject.Find",
        "FindObjectOfType",
        "FindFirstObjectByType",
        "Resources.Load",
    ]
    .iter()
    .any(|call| text.contains(call))
}

fn csharp_methods(source: &str) -> BTreeMap<String, (usize, String)> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut methods = BTreeMap::new();
    let mut index = 0;
    while index < lines.len() {
        let Some(name) = csharp_method_name(lines[index]) else {
            index += 1;
            continue;
        };
        let Some((body, next_index)) = csharp_method_body(&lines, index) else {
            index += 1;
            continue;
        };
        methods.insert(name, (index + 1, body));
        index = next_index;
    }
    methods
}

fn csharp_method_name(line: &str) -> Option<String> {
    let paren = line.find('(')?;
    let name = line[..paren]
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '_');
    (!name.is_empty() && !matches!(name, "if" | "for" | "while" | "switch" | "catch"))
        .then(|| name.to_string())
}

fn csharp_method_body(lines: &[&str], start: usize) -> Option<(String, usize)> {
    let mut body = String::new();
    let mut depth = 0isize;
    let mut opened = false;
    let mut index = start;
    while index < lines.len() {
        let current = lines[index];
        depth += current.matches('{').count() as isize;
        opened |= current.contains('{');
        depth -= current.matches('}').count() as isize;
        body.push_str(current);
        body.push('\n');
        index += 1;
        if opened && depth <= 0 {
            break;
        }
    }
    opened.then_some((body, index))
}

fn scan_editor_api(source: &str, path: &str, findings: &mut Vec<Finding>) {
    let mut editor_only_branches = Vec::new();
    for (line, text) in source.lines().enumerate() {
        let trimmed = text.trim();
        if update_editor_branch_state(trimmed, &mut editor_only_branches) {
            continue;
        }
        if is_unguarded_editor_api(trimmed, &editor_only_branches) {
            findings.push(unity_finding(
                UnityFindingInput::new(
                    FindingKind::UnityEditorApiInRuntime,
                    path.into(),
                    line + 1,
                    "UnityEditor API is reachable from runtime code without a UNITY_EDITOR guard"
                        .into(), (
                    1,
                    1))
                .with_reliability(0.95),
            ));
        }
    }
}

fn update_editor_branch_state(line: &str, branches: &mut Vec<(bool, bool)>) -> bool {
    match editor_directive(line) {
        Some(EditorDirective::If(condition)) => {
            branches.push((editor_only_condition(condition), false));
        }
        Some(EditorDirective::ElseIf(condition)) => update_editor_elif(condition, branches),
        Some(EditorDirective::Else) => update_editor_else(branches),
        Some(EditorDirective::EndIf) => {
            branches.pop();
        }
        None => return false,
    }
    true
}

enum EditorDirective<'a> {
    If(&'a str),
    ElseIf(&'a str),
    Else,
    EndIf,
}

fn editor_directive(line: &str) -> Option<EditorDirective<'_>> {
    match line.split_whitespace().next()? {
        "#elif" => Some(EditorDirective::ElseIf(
            line.strip_prefix("#elif").unwrap_or_default(),
        )),
        "#if" => Some(EditorDirective::If(
            line.strip_prefix("#if").unwrap_or_default(),
        )),
        "#else" => Some(EditorDirective::Else),
        "#endif" => Some(EditorDirective::EndIf),
        _ => None,
    }
}

fn update_editor_elif(condition: &str, branches: &mut [(bool, bool)]) {
    if let Some((editor_only, has_elif)) = branches.last_mut() {
        *editor_only = editor_only_condition(condition);
        *has_elif = true;
    }
}

fn update_editor_else(branches: &mut [(bool, bool)]) {
    if let Some((editor_only, has_elif)) = branches.last_mut() {
        *editor_only = !*has_elif && !*editor_only;
    }
}

fn is_unguarded_editor_api(line: &str, branches: &[(bool, bool)]) -> bool {
    !branches.iter().any(|(editor_only, _)| *editor_only)
        && (line.starts_with("using UnityEditor") || line.contains("UnityEditor."))
}

fn editor_only_condition(condition: &str) -> bool {
    condition
        .split("||")
        .all(|branch| branch.contains("UNITY_EDITOR") && !branch.contains("!UNITY_EDITOR"))
}

fn scan_event_balance(source: &str, path: &str, findings: &mut Vec<Finding>) {
    let subscriptions = source.matches("+=").count();
    let unsubscriptions = source.matches("-=").count();
    if subscriptions > unsubscriptions {
        findings.push(unity_finding(
            UnityFindingInput::new(
                FindingKind::UnityUnbalancedEventSubscription,
                path.into(),
                1,
                format!("Unity type has {subscriptions} event subscriptions but only {unsubscriptions} unsubscriptions"), (
                subscriptions,
                unsubscriptions.max(1))).with_reliability(0.6),
        ));
    }
}
