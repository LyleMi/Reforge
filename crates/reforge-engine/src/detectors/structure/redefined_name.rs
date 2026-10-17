use super::*;

/// Only direct lexical children are compared. Conditional definitions and
/// decorated names are deliberately excluded: their binding semantics differ.
pub(super) fn scan(file: &ParsedSourceFile, detections: &mut Vec<DetectedEvidence>) {
    let mut scopes = vec![file.tree.root_node()];
    while let Some(scope) = scopes.pop() {
        let definitions = scope_definitions(scope, &file.file.source, &mut scopes);
        for (name, lines) in definitions {
            if lines.len() < 2 {
                continue;
            }
            detections.push(DetectedEvidenceInput::new(
                Rule::RedefinedName,
                &file.file.display_path,
                Some(lines[0]),
                format!("name `{name}` is defined {} times in the same direct Python scope; review whether later bindings intentionally replace earlier definitions", lines.len()),
                vec![DetectedMeasurement::measurement(MetricId::GroupSize, lines.len(), "definitions")],
            ).with_group_subject().with_related_locations(lines.into_iter().map(|line| RelatedLocation {
                path: file.file.display_path.clone(),
                line,
                name: Some(name.into()),
                entity_key: None,
            }).collect()).into());
        }
    }
}

fn scope_definitions<'tree, 'source>(
    scope: Node<'tree>,
    source: &'source str,
    scopes: &mut Vec<Node<'tree>>,
) -> BTreeMap<&'source str, Vec<usize>> {
    let mut definitions: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut cursor = scope.walk();
    for child in scope.named_children(&mut cursor) {
        let is_decorated = child.kind() == "decorated_definition";
        let definition = if is_decorated {
            child.child_by_field_name("definition")
        } else {
            Some(child)
        };
        let Some(definition) = definition
            .filter(|node| matches!(node.kind(), "function_definition" | "class_definition"))
        else {
            continue;
        };
        if let Some(body) = definition.child_by_field_name("body") {
            scopes.push(body);
        }
        let Some(name) = definition
            .child_by_field_name("name")
            .and_then(|node| node.utf8_text(source.as_bytes()).ok())
        else {
            continue;
        };
        if !is_decorated {
            definitions
                .entry(name)
                .or_default()
                .push(definition.start_position().row + 1);
        }
    }
    definitions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detect(source: &str) -> Vec<DetectedEvidence> {
        let files = parse_source_files(&[SourceFile {
            path: "sample.py".into(),
            display_path: "sample.py".into(),
            source: source.into(),
        }])
        .unwrap();
        let mut detections = Vec::new();
        scan(&files[0], &mut detections);
        detections
    }

    #[test]
    fn reports_definitions_with_both_locations_in_each_direct_scope() {
        let findings = detect(
            "def check(): pass\nasync def check(): pass\nclass A:\n def run(self): pass\n def run(self): pass\ndef outer():\n def inner(): pass\n class inner: pass\n",
        );
        assert_eq!(findings.len(), 3);
        assert_eq!(
            findings[0]
                .related_locations
                .iter()
                .map(|l| l.line)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(
            findings
                .iter()
                .all(|f| f.kind == Rule::RedefinedName && f.related_locations.len() == 2)
        );
    }

    #[test]
    fn ignores_distinct_scopes_decorators_and_conditional_alternatives() {
        assert!(detect("class A:\n def run(self): pass\nclass B:\n def run(self): pass\nif flag:\n def check(): pass\nelse:\n def check(): pass\n@overload\ndef parse(x): ...\n@overload\ndef parse(x, y): ...\ndef parse(*args): pass\nclass C:\n @property\n def value(self): pass\n @value.setter\n def value(self, x): pass\n").is_empty());
    }

    #[test]
    fn decorated_definition_does_not_hide_plain_redefinitions() {
        let findings =
            detect("@registered\ndef check(): pass\ndef check(): pass\ndef check(): pass\n");
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0]
                .related_locations
                .iter()
                .map(|location| location.line)
                .collect::<Vec<_>>(),
            [3, 4]
        );
    }
}
