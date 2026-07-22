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
                    ,
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
                ,
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
