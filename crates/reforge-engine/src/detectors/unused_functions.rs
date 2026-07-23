use std::collections::BTreeMap;

use anyhow::Result;
use tree_sitter::Node;

use crate::detectors::similarity::{ParsedSourceFile, SourceFile, parse_source_files};
use crate::evidence_analysis::DetectedEvidenceInput;
use crate::lang::{
    FUNCTION_DECLARATION, FUNCTION_DEFINITION, FUNCTION_ITEM, GENERATOR_FUNCTION_DECLARATION,
    IDENTIFIER_KIND, LanguageFamily, NAME_FIELD, has_rust_cfg_test_attribute,
    rust_attributes_before,
};
use crate::model::{DetectedEvidence, DetectedMeasurement, MetricId, Rule};
use crate::scan::is_test_source;

#[derive(Debug, Clone)]
pub struct UnusedFunctionOptions {
    pub include_tests: bool,
}

#[derive(Debug, Clone)]
struct FunctionDefinition {
    name: String,
    path: String,
    line: usize,
    start_byte: usize,
    end_byte: usize,
}

#[derive(Debug, Clone)]
struct IdentifierReference {
    path: String,
    byte: usize,
}

#[derive(Debug, Clone, Copy)]
struct UnusedFunctionContext<'a> {
    source: &'a str,
    file: &'a SourceFile,
    family: LanguageFamily,
    collect_candidates: bool,
}

#[allow(dead_code)]
pub fn scan_unused_functions(
    files: &[SourceFile],
    options: &UnusedFunctionOptions,
) -> Result<Vec<DetectedEvidence>> {
    let parsed_files = parse_source_files(files)?;
    Ok(scan_parsed_unused_functions(&parsed_files, options))
}

pub(crate) fn scan_parsed_unused_functions(
    files: &[ParsedSourceFile],
    options: &UnusedFunctionOptions,
) -> Vec<DetectedEvidence> {
    let mut definitions = Vec::new();
    let mut references = BTreeMap::<String, Vec<IdentifierReference>>::new();

    for file in files {
        let context = UnusedFunctionContext {
            source: &file.file.source,
            file: &file.file,
            family: file.family,
            collect_candidates: options.include_tests || !is_test_source(&file.file.path),
        };
        collect_unused_function_inputs(
            file.tree.root_node(),
            context,
            &mut definitions,
            &mut references,
        );
    }

    definitions
        .into_iter()
        .filter(|definition| !has_external_reference(definition, &references))
        .map(unused_function_detection)
        .collect()
}

fn collect_unused_function_inputs(
    node: Node<'_>,
    context: UnusedFunctionContext<'_>,
    definitions: &mut Vec<FunctionDefinition>,
    references: &mut BTreeMap<String, Vec<IdentifierReference>>,
) {
    if context.family == LanguageFamily::Rust && node.kind() == "attribute_item" {
        for name in rust_serde_callback_names(node, context.source) {
            references
                .entry(name)
                .or_default()
                .push(IdentifierReference {
                    path: context.file.display_path.clone(),
                    byte: node.start_byte(),
                });
        }
    }
    if let Some(name) = identifier_text(node, context.source) {
        references
            .entry(name)
            .or_default()
            .push(IdentifierReference {
                path: context.file.display_path.clone(),
                byte: node.start_byte(),
            });
    }

    if context.collect_candidates
        && !is_inside_rust_test_module(node, context)
        && let Some(definition) = function_definition(node, context)
    {
        definitions.push(definition);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_unused_function_inputs(child, context, definitions, references);
    }
}

fn rust_serde_callback_names(node: Node<'_>, source: &str) -> Vec<String> {
    let Ok(attribute) = node.utf8_text(source.as_bytes()) else {
        return Vec::new();
    };
    if !attribute.contains("serde") {
        return Vec::new();
    }
    const KEYS: [&str; 5] = [
        "serialize_with",
        "deserialize_with",
        "default",
        "skip_serializing_if",
        "getter",
    ];
    let mut names = Vec::new();
    for key in KEYS {
        let mut remaining = attribute;
        while let Some(index) = remaining.find(key) {
            remaining = &remaining[index + key.len()..];
            let Some(equals) = remaining.find('=') else {
                break;
            };
            let value = remaining[equals + 1..].trim_start();
            let Some(value) = value.strip_prefix('"') else {
                continue;
            };
            let Some(end) = value.find('"') else { break };
            let path = &value[..end];
            if let Some(name) = path
                .rsplit("::")
                .next()
                .filter(|name| is_reference_name(name))
            {
                names.push(name.to_string());
            }
            remaining = &value[end + 1..];
        }
    }
    names.sort();
    names.dedup();
    names
}

fn identifier_text(node: Node<'_>, source: &str) -> Option<String> {
    if node.kind() != IDENTIFIER_KIND {
        return None;
    }

    node.utf8_text(source.as_bytes())
        .ok()
        .filter(|name| is_reference_name(name))
        .map(ToString::to_string)
}

fn is_reference_name(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
}

fn function_definition(
    node: Node<'_>,
    context: UnusedFunctionContext<'_>,
) -> Option<FunctionDefinition> {
    let name_node = candidate_name_node(node, context)?;
    let name = name_node.utf8_text(context.source.as_bytes()).ok()?;
    if should_skip_function_name(name) || is_public_or_exported_function(node, name, context) {
        return None;
    }

    Some(FunctionDefinition {
        name: name.to_string(),
        path: context.file.display_path.clone(),
        line: node.start_position().row + 1,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    })
}

fn candidate_name_node<'tree>(
    node: Node<'tree>,
    context: UnusedFunctionContext<'_>,
) -> Option<Node<'tree>> {
    match context.family {
        LanguageFamily::Rust
            if node.kind() == FUNCTION_ITEM
                && !has_ancestor_kind(node, "impl_item")
                && !has_ancestor_kind(node, "trait_item")
                && !has_rust_test_attribute(node, context.source) =>
        {
            node.child_by_field_name(NAME_FIELD)
        }
        LanguageFamily::JavaScriptTypeScript
            if matches!(
                node.kind(),
                FUNCTION_DECLARATION | GENERATOR_FUNCTION_DECLARATION
            ) =>
        {
            node.child_by_field_name(NAME_FIELD)
        }
        LanguageFamily::Python
            if node.kind() == FUNCTION_DEFINITION
                && !has_ancestor_kind(node, "class_definition") =>
        {
            node.child_by_field_name(NAME_FIELD)
        }
        LanguageFamily::Go if node.kind() == FUNCTION_DECLARATION => {
            node.child_by_field_name(NAME_FIELD)
        }
        LanguageFamily::CSharp if node.kind() == "local_function_statement" => {
            node.child_by_field_name(NAME_FIELD)
        }
        LanguageFamily::Java
        | LanguageFamily::Kotlin
        | LanguageFamily::Php
        | LanguageFamily::Ruby
        | LanguageFamily::Bash
        | LanguageFamily::PowerShell => None,
        _ => None,
    }
}

fn should_skip_function_name(name: &str) -> bool {
    name == "main"
        || name == "init"
        || name == "new"
        || name == "default"
        || name == "setup"
        || name == "teardown"
        || name.starts_with("test_")
        || (name.starts_with("__") && name.ends_with("__"))
}

fn is_public_or_exported_function(
    node: Node<'_>,
    name: &str,
    context: UnusedFunctionContext<'_>,
) -> bool {
    match context.family {
        LanguageFamily::Rust => rust_function_is_public(node, context.source),
        LanguageFamily::JavaScriptTypeScript => has_ancestor_kind(node, "export_statement"),
        LanguageFamily::Python => !name.starts_with('_'),
        LanguageFamily::Go => name
            .chars()
            .next()
            .is_some_and(|character| character.is_uppercase()),
        LanguageFamily::CSharp => false,
        LanguageFamily::Java
        | LanguageFamily::Kotlin
        | LanguageFamily::Php
        | LanguageFamily::Ruby
        | LanguageFamily::Bash
        | LanguageFamily::PowerShell => true,
    }
}

fn rust_function_is_public(node: Node<'_>, source: &str) -> bool {
    node.child_by_field_name("visibility").is_some()
        || node
            .utf8_text(source.as_bytes())
            .ok()
            .is_some_and(|text| text.trim_start().starts_with("pub"))
}

fn has_external_reference(
    definition: &FunctionDefinition,
    references: &BTreeMap<String, Vec<IdentifierReference>>,
) -> bool {
    references.get(&definition.name).is_some_and(|references| {
        references.iter().any(|reference| {
            reference.path != definition.path
                || reference.byte < definition.start_byte
                || reference.byte >= definition.end_byte
        })
    })
}

fn unused_function_detection(definition: FunctionDefinition) -> DetectedEvidence {
    crate::model::DetectedEvidence::from(DetectedEvidenceInput::new(
        Rule::UnusedFunction,
        definition.path,
        Some(definition.line),
        format!(
            "function `{}` has no references outside its own body",
            definition.name
        ),
        vec![DetectedMeasurement::threshold(
            MetricId::FunctionReferences,
            0,
            1,
            "references",
        )],
    ))
}

fn is_inside_rust_test_module(node: Node<'_>, context: UnusedFunctionContext<'_>) -> bool {
    if context.family != LanguageFamily::Rust {
        return false;
    }

    let mut current = Some(node);
    while let Some(candidate) = current {
        if candidate.kind() == "mod_item" && has_rust_cfg_test_attribute(candidate, context.source)
        {
            return true;
        }
        current = candidate.parent();
    }
    false
}

fn has_ancestor_kind(mut node: Node<'_>, kind: &str) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == kind {
            return true;
        }
        node = parent;
    }

    false
}

fn has_rust_test_attribute(node: Node<'_>, source: &str) -> bool {
    has_prefixed_attribute(node, source, "#[test]")
        || has_prefixed_attribute(node, source, "#[tokio::test")
        || has_prefixed_attribute(node, source, "#[async_std::test")
        || has_rust_cfg_test_attribute(node, source)
}

fn has_prefixed_attribute(node: Node<'_>, source: &str, prefix: &str) -> bool {
    rust_attributes_before(node, source)
        .into_iter()
        .any(|attribute| attribute.starts_with(prefix))
}

#[cfg(test)]
#[path = "../unused_functions_tests.rs"]
mod tests;
