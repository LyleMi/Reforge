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
    Java,
    CSharp,
    Kotlin,
    Php,
    Ruby,
    Bash,
    PowerShell,
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
        "js" | "jsx" | "mjs" | "cjs" => Some(LanguageAdapter {
            family: LanguageFamily::JavaScriptTypeScript,
            language: || tree_sitter_javascript::LANGUAGE.into(),
        }),
        "ts" | "mts" | "cts" => Some(LanguageAdapter {
            family: LanguageFamily::JavaScriptTypeScript,
            language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }),
        "tsx" | "vue" => Some(LanguageAdapter {
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
        "java" => Some(LanguageAdapter {
            family: LanguageFamily::Java,
            language: || tree_sitter_java::LANGUAGE.into(),
        }),
        "cs" | "csx" => Some(LanguageAdapter {
            family: LanguageFamily::CSharp,
            language: || tree_sitter_c_sharp::LANGUAGE.into(),
        }),
        "kt" => Some(LanguageAdapter {
            family: LanguageFamily::Kotlin,
            language: || tree_sitter_kotlin_ng::LANGUAGE.into(),
        }),
        "php" => Some(LanguageAdapter {
            family: LanguageFamily::Php,
            language: || tree_sitter_php::LANGUAGE_PHP.into(),
        }),
        "rb" => Some(LanguageAdapter {
            family: LanguageFamily::Ruby,
            language: || tree_sitter_ruby::LANGUAGE.into(),
        }),
        "sh" | "bash" => Some(LanguageAdapter {
            family: LanguageFamily::Bash,
            language: || tree_sitter_bash::LANGUAGE.into(),
        }),
        "ps1" | "psm1" => Some(LanguageAdapter {
            family: LanguageFamily::PowerShell,
            language: || tree_sitter_powershell::LANGUAGE.into(),
        }),
        _ => None,
    }
}

pub(crate) fn vue_script_source(path: &Path, source: &str) -> Option<String> {
    (path.extension().and_then(|extension| extension.to_str()) == Some("vue"))
        .then(|| extract_vue_scripts(source))
}

fn extract_vue_scripts(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut extracted = bytes
        .iter()
        .map(|byte| {
            if matches!(byte, b'\r' | b'\n') {
                *byte
            } else {
                b' '
            }
        })
        .collect::<Vec<_>>();
    let mut search_start = 0;

    while let Some(tag_start) = find_ascii_case_insensitive(bytes, b"<script", search_start) {
        let Some(content_start) = find_tag_end(bytes, tag_start + b"<script".len()) else {
            break;
        };
        let Some(content_end) = find_ascii_case_insensitive(bytes, b"</script", content_start)
        else {
            break;
        };
        extracted[content_start..content_end].copy_from_slice(&bytes[content_start..content_end]);
        search_start = find_tag_end(bytes, content_end + b"</script".len()).unwrap_or(bytes.len());
    }

    String::from_utf8(extracted).expect("masking Vue source preserves UTF-8")
}

fn find_ascii_case_insensitive(haystack: &[u8], needle: &[u8], start: usize) -> Option<usize> {
    haystack
        .get(start..)?
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
        .map(|offset| start + offset)
}

fn find_tag_end(source: &[u8], start: usize) -> Option<usize> {
    let mut quote = None;
    for (offset, byte) in source.get(start..)?.iter().copied().enumerate() {
        match (quote, byte) {
            (None, b'\'' | b'"') => quote = Some(byte),
            (Some(opening), closing) if opening == closing => quote = None,
            (None, b'>') => return Some(start + offset + 1),
            _ => {}
        }
    }
    None
}

pub(crate) fn is_binding_identifier_kind(kind: &str) -> bool {
    matches!(
        kind,
        IDENTIFIER_KIND
            | FIELD_IDENTIFIER_KIND
            | PROPERTY_IDENTIFIER_KIND
            | SHORTHAND_PROPERTY_IDENTIFIER_KIND
            | "constant"
            | "name"
            | "variable_name"
    )
}

pub(crate) fn is_identifier_like_kind(kind: &str) -> bool {
    is_binding_identifier_kind(kind)
        || matches!(
            kind,
            TYPE_IDENTIFIER_KIND | SCOPED_IDENTIFIER_KIND | SELF_KIND
        )
}

pub(crate) fn child_by_kind<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == kind)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_modern_javascript_typescript_and_csharp_extensions() {
        for path in ["app.mjs", "app.cjs", "app.mts", "app.cts"] {
            assert_eq!(
                adapter_for_path(Path::new(path)).map(|adapter| adapter.family),
                Some(LanguageFamily::JavaScriptTypeScript),
                "{path}"
            );
        }
        assert_eq!(
            adapter_for_path(Path::new("script.csx")).map(|adapter| adapter.family),
            Some(LanguageFamily::CSharp)
        );
        assert_eq!(
            adapter_for_path(Path::new("Component.vue")).map(|adapter| adapter.family),
            Some(LanguageFamily::JavaScriptTypeScript)
        );
    }

    #[test]
    fn recognizes_bash_and_powershell_script_extensions() {
        for path in ["build.sh", "install.bash"] {
            assert_eq!(
                adapter_for_path(Path::new(path)).map(|adapter| adapter.family),
                Some(LanguageFamily::Bash),
                "{path}"
            );
        }
        for path in ["deploy.ps1", "module.psm1"] {
            assert_eq!(
                adapter_for_path(Path::new(path)).map(|adapter| adapter.family),
                Some(LanguageFamily::PowerShell),
                "{path}"
            );
        }
        assert_eq!(
            adapter_for_path(Path::new("module.psd1")).map(|a| a.family),
            None
        );
    }

    #[test]
    fn extracts_vue_scripts_while_preserving_offsets_and_lines() {
        let source = "<template><div>ignored</div></template>\n<script setup lang=\"ts\">\nfunction helper() { return 1; }\n</script>\n<style>.x { color: red }</style>";
        let extracted = vue_script_source(Path::new("Component.vue"), source).unwrap();

        assert_eq!(extracted.len(), source.len());
        assert_eq!(extracted.lines().count(), source.lines().count());
        assert!(extracted.contains("function helper()"));
        assert!(!extracted.contains("ignored"));
    }
}
