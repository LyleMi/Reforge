use super::*;

pub(super) fn parameter_names(
    parameters: Option<Node<'_>>,
    source: &str,
    family: LanguageFamily,
) -> Vec<String> {
    let Some(parameters) = parameters else {
        return Vec::new();
    };

    match family {
        LanguageFamily::Rust => rust_parameter_names(parameters, source),
        LanguageFamily::Go => go_parameter_names(parameters, source),
        LanguageFamily::JavaScriptTypeScript => {
            javascript_typescript_parameter_names(parameters, source)
        }
        _ => {
            let mut names = Vec::new();
            let mut cursor = parameters.walk();
            for child in parameters.named_children(&mut cursor) {
                collect_parameter_name(child, source, &mut names);
            }
            names
        }
    }
}

fn rust_parameter_names(parameters: Node<'_>, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = parameters.walk();
    for child in parameters.named_children(&mut cursor) {
        match child.kind() {
            "self_parameter" => {}
            "parameter" => {
                if let Some(pattern) = child.child_by_field_name("pattern") {
                    collect_parameter_name(pattern, source, &mut names);
                } else {
                    collect_parameter_name(child, source, &mut names);
                }
            }
            _ => collect_parameter_name(child, source, &mut names),
        }
    }
    names
}

fn javascript_typescript_parameter_names(parameters: Node<'_>, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = parameters.walk();
    for child in parameters.named_children(&mut cursor) {
        let Some(name) = javascript_typescript_parameter_name(child, source) else {
            continue;
        };
        if name != "this" {
            names.push(name);
        }
    }
    names
}

fn javascript_typescript_parameter_name(parameter: Node<'_>, source: &str) -> Option<String> {
    let mut names = Vec::new();
    collect_javascript_typescript_parameter_binding(parameter, source, &mut names);
    names.into_iter().next().or_else(|| {
        is_javascript_typescript_parameter_node(parameter.kind()).then(|| "value".to_string())
    })
}

fn is_javascript_typescript_parameter_node(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "field_identifier"
            | "property_identifier"
            | "shorthand_property_identifier"
            | "required_parameter"
            | "optional_parameter"
            | "assignment_pattern"
            | "rest_pattern"
            | "object_pattern"
            | "array_pattern"
    )
}

fn collect_javascript_typescript_parameter_binding(
    node: Node<'_>,
    source: &str,
    names: &mut Vec<String>,
) {
    match node.kind() {
        "type_annotation" | "return_type" => {}
        "required_parameter" | "optional_parameter" => {
            if let Some(pattern) = node
                .child_by_field_name("pattern")
                .or_else(|| node.child_by_field_name("name"))
            {
                collect_javascript_typescript_parameter_binding(pattern, source, names);
            } else {
                collect_javascript_typescript_parameter_children(node, source, names);
            }
        }
        "assignment_pattern" => {
            if let Some(left) = node.child_by_field_name("left") {
                collect_javascript_typescript_parameter_binding(left, source, names);
            } else {
                collect_javascript_typescript_parameter_children(node, source, names);
            }
        }
        "rest_pattern" => {
            if let Some(argument) = node.child_by_field_name("argument") {
                collect_javascript_typescript_parameter_binding(argument, source, names);
            } else {
                collect_javascript_typescript_parameter_children(node, source, names);
            }
        }
        "identifier"
        | "field_identifier"
        | "property_identifier"
        | "shorthand_property_identifier" => {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                let name = normalize_identifier(text);
                if !name.is_empty() {
                    names.push(name);
                }
            }
        }
        _ => collect_javascript_typescript_parameter_children(node, source, names),
    }
}

fn collect_javascript_typescript_parameter_children(
    node: Node<'_>,
    source: &str,
    names: &mut Vec<String>,
) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_javascript_typescript_parameter_binding(child, source, names);
    }
}

fn go_parameter_names(parameters: Node<'_>, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = parameters.walk();
    for child in parameters.named_children(&mut cursor) {
        if child.kind() != "parameter_declaration" {
            continue;
        }

        let before_type = child
            .children_by_field_name("name", &mut child.walk())
            .filter_map(|name| name.utf8_text(source.as_bytes()).ok())
            .map(normalize_identifier)
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>();

        if before_type.is_empty() {
            names.push("value".to_string());
        } else {
            names.extend(before_type);
        }
    }
    names
}

fn collect_parameter_name(node: Node<'_>, source: &str, names: &mut Vec<String>) {
    match node.kind() {
        "identifier" | "field_identifier" | "shorthand_property_identifier" => {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                let name = normalize_identifier(text);
                if !name.is_empty() && name != "self" {
                    names.push(name);
                }
            }
        }
        "type_identifier" | "primitive_type" => {}
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_parameter_name(child, source, names);
            }
        }
    }
}

pub(super) fn collect_data_clumps(
    file: &SourceFile,
    function: &FunctionMetric,
    options: &StructureOptions,
    signals: &mut FileSignals,
) {
    let names = function
        .parameter_names
        .iter()
        .filter(|name| name.len() > 1)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    if names.len() < 3 || options.min_data_clump_occurrences == 0 {
        return;
    }

    signals.data_clumps.push((
        names.join(", "),
        Occurrence {
            path: file.display_path.clone(),
            line: function.line,
            name: Some(function.name.clone()),
        },
    ));
}
