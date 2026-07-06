use std::path::Path;

use tree_sitter::{Language, Node};

pub(crate) const BODY_FIELD: &str = "body";
pub(crate) const NAME_FIELD: &str = "name";
pub(crate) const PARAMETERS_FIELD: &str = "parameters";

pub(crate) const ARROW_FUNCTION: &str = "arrow_function";
pub(crate) const FUNCTION_DECLARATION: &str = "function_declaration";
pub(crate) const FUNCTION_DEFINITION: &str = "function_definition";
pub(crate) const FUNCTION_ITEM: &str = "function_item";
pub(crate) const GENERATOR_FUNCTION_DECLARATION: &str = "generator_function_declaration";
pub(crate) const METHOD_DECLARATION: &str = "method_declaration";
pub(crate) const METHOD_DEFINITION: &str = "method_definition";

pub(crate) const IDENTIFIER_KIND: &str = "identifier";
pub(crate) const FIELD_IDENTIFIER_KIND: &str = "field_identifier";
pub(crate) const PROPERTY_IDENTIFIER_KIND: &str = "property_identifier";
pub(crate) const SHORTHAND_PROPERTY_IDENTIFIER_KIND: &str = "shorthand_property_identifier";
pub(crate) const TYPE_IDENTIFIER_KIND: &str = "type_identifier";
pub(crate) const SCOPED_IDENTIFIER_KIND: &str = "scoped_identifier";
pub(crate) const SELF_KIND: &str = "self";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LanguageFamily {
    Rust,
    JavaScriptTypeScript,
    Python,
    Go,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LanguageAdapter {
    pub(crate) family: LanguageFamily,
    language: fn() -> Language,
}

impl LanguageAdapter {
    pub(crate) fn language(self) -> Language {
        (self.language)()
    }
}

pub(crate) fn adapter_for_path(path: &Path) -> Option<LanguageAdapter> {
    let extension = path.extension()?.to_str()?;

    match extension {
        "rs" => Some(LanguageAdapter {
            family: LanguageFamily::Rust,
            language: || tree_sitter_rust::LANGUAGE.into(),
        }),
        "js" | "jsx" => Some(LanguageAdapter {
            family: LanguageFamily::JavaScriptTypeScript,
            language: || tree_sitter_javascript::LANGUAGE.into(),
        }),
        "ts" => Some(LanguageAdapter {
            family: LanguageFamily::JavaScriptTypeScript,
            language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }),
        "tsx" => Some(LanguageAdapter {
            family: LanguageFamily::JavaScriptTypeScript,
            language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
        }),
        "py" => Some(LanguageAdapter {
            family: LanguageFamily::Python,
            language: || tree_sitter_python::LANGUAGE.into(),
        }),
        "go" => Some(LanguageAdapter {
            family: LanguageFamily::Go,
            language: || tree_sitter_go::LANGUAGE.into(),
        }),
        _ => None,
    }
}

pub(crate) fn is_binding_identifier_kind(kind: &str) -> bool {
    matches!(
        kind,
        IDENTIFIER_KIND
            | FIELD_IDENTIFIER_KIND
            | PROPERTY_IDENTIFIER_KIND
            | SHORTHAND_PROPERTY_IDENTIFIER_KIND
    )
}

pub(crate) fn is_identifier_like_kind(kind: &str) -> bool {
    is_binding_identifier_kind(kind)
        || matches!(
            kind,
            TYPE_IDENTIFIER_KIND | SCOPED_IDENTIFIER_KIND | SELF_KIND
        )
}

pub(crate) fn has_rust_cfg_test_attribute(node: Node<'_>, source: &str) -> bool {
    rust_attributes_before(node, source)
        .into_iter()
        .any(|attribute| is_rust_cfg_test_attribute(&attribute))
}

pub(crate) fn rust_attributes_before(node: Node<'_>, source: &str) -> Vec<String> {
    let mut attributes = Vec::new();
    let mut end = node.start_byte().min(source.len());
    let bytes = source.as_bytes();

    loop {
        while end > 0 && bytes[end - 1].is_ascii_whitespace() {
            end -= 1;
        }

        if end == 0 || bytes[end - 1] != b']' {
            return attributes;
        }

        let Some(start) = source[..end].rfind("#[") else {
            return attributes;
        };
        attributes.push(source[start..end].to_string());
        end = start;
    }
}

fn is_rust_cfg_test_attribute(attribute: &str) -> bool {
    let compact = attribute
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let Some(inner) = compact
        .strip_prefix("#[cfg(")
        .and_then(|value| value.strip_suffix(")]"))
    else {
        return false;
    };

    inner == "test"
        || inner.starts_with("any(test")
        || inner.starts_with("all(test")
        || inner.contains("(test,")
        || inner.contains(",test,")
        || inner.ends_with(",test")
        || inner.ends_with(",test)")
}
