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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum VueScriptLanguage {
    JavaScript,
    TypeScript,
    Tsx,
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

pub(crate) fn adapter_for_source(path: &Path, source: &str) -> Option<LanguageAdapter> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("vue") {
        return adapter_for_path(path);
    }

    Some(match vue_script_language(source) {
        VueScriptLanguage::JavaScript => LanguageAdapter {
            family: LanguageFamily::JavaScriptTypeScript,
            language: || tree_sitter_javascript::LANGUAGE.into(),
        },
        VueScriptLanguage::TypeScript => LanguageAdapter {
            family: LanguageFamily::JavaScriptTypeScript,
            language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        },
        VueScriptLanguage::Tsx => LanguageAdapter {
            family: LanguageFamily::JavaScriptTypeScript,
            language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
        },
    })
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

    while let Some(tag_start) = find_tag_start(bytes, b"<script", search_start) {
        let Some(content_start) = find_tag_end(bytes, tag_start + b"<script".len()) else {
            break;
        };
        let Some(content_end) = find_tag_start(bytes, b"</script", content_start) else {
            break;
        };
        extracted[content_start..content_end].copy_from_slice(&bytes[content_start..content_end]);
        search_start = find_tag_end(bytes, content_end + b"</script".len()).unwrap_or(bytes.len());
    }

    String::from_utf8(extracted).expect("masking Vue source preserves UTF-8")
}

fn vue_script_language(source: &str) -> VueScriptLanguage {
    let bytes = source.as_bytes();
    let mut language = VueScriptLanguage::JavaScript;
    let mut search_start = 0;

    while let Some(tag_start) = find_tag_start(bytes, b"<script", search_start) {
        let Some(tag_end) = find_tag_end(bytes, tag_start + b"<script".len()) else {
            break;
        };
        if let Some(value) = tag_attribute_value(&source[tag_start..tag_end], "lang") {
            language = language.max(match value.trim().to_ascii_lowercase().as_str() {
                "ts" | "typescript" => VueScriptLanguage::TypeScript,
                "tsx" => VueScriptLanguage::Tsx,
                _ => VueScriptLanguage::JavaScript,
            });
        }
        search_start = tag_end;
    }

    language
}

fn tag_attribute_value<'a>(tag: &'a str, attribute: &str) -> Option<&'a str> {
    let mut remaining = tag.get(b"<script".len()..)?;

    loop {
        remaining = remaining.trim_start_matches(is_tag_separator);
        if remaining.is_empty() || remaining.starts_with('>') {
            return None;
        }

        let name_end = remaining
            .find(|character| !is_attribute_name_character(character))
            .unwrap_or(remaining.len());
        if name_end == 0 {
            remaining = &remaining[1..];
            continue;
        }
        let name = &remaining[..name_end];
        remaining = remaining[name_end..].trim_start();

        let Some(after_equals) = remaining.strip_prefix('=') else {
            continue;
        };
        let (value, rest) = split_tag_attribute_value(after_equals.trim_start())?;

        if name.eq_ignore_ascii_case(attribute) {
            return Some(value);
        }
        remaining = rest;
    }
}

fn is_tag_separator(character: char) -> bool {
    character.is_ascii_whitespace() || character == '/'
}

fn is_attribute_name_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':' | '.')
}

fn split_tag_attribute_value(value: &str) -> Option<(&str, &str)> {
    let Some(quote @ ('\'' | '"')) = value.chars().next() else {
        let end = value
            .find(|character: char| character.is_ascii_whitespace() || character == '>')
            .unwrap_or(value.len());
        return Some((&value[..end], &value[end..]));
    };
    let content = &value[quote.len_utf8()..];
    let end = content.find(quote)?;
    Some((&content[..end], &content[end + quote.len_utf8()..]))
}

fn find_tag_start(haystack: &[u8], needle: &[u8], mut start: usize) -> Option<usize> {
    while let Some(candidate) = find_ascii_case_insensitive(haystack, needle, start) {
        let after_name = candidate + needle.len();
        if haystack
            .get(after_name)
            .is_none_or(|byte| byte.is_ascii_whitespace() || matches!(byte, b'>' | b'/'))
        {
            return Some(candidate);
        }
        start = after_name;
    }
    None
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

    #[test]
    fn selects_vue_parser_from_script_language() {
        let cases = [
            ("<script>const value = 1</script>", "javascript"),
            ("<script setup lang='TS'></script>", "typescript"),
            ("<SCRIPT LANG = \"tsx\"></SCRIPT>", "tsx"),
        ];

        for (source, expected) in cases {
            let actual = match vue_script_language(source) {
                VueScriptLanguage::JavaScript => "javascript",
                VueScriptLanguage::TypeScript => "typescript",
                VueScriptLanguage::Tsx => "tsx",
            };
            assert_eq!(actual, expected, "{source}");
        }
    }

    #[test]
    fn parses_vue_typescript_generic_arrows_as_typescript() {
        let source = r#"<template><div>{{ identity('value') }}</div></template>
<script setup lang="ts">
const identity = <T>(value: T): T => value;
</script>"#;
        let extracted = vue_script_source(Path::new("Component.vue"), source).unwrap();
        let adapter = adapter_for_source(Path::new("Component.vue"), source).unwrap();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&adapter.language()).unwrap();
        let tree = parser.parse(&extracted, None).unwrap();

        assert!(
            !tree.root_node().has_error(),
            "{}",
            tree.root_node().to_sexp()
        );
    }

    #[test]
    fn ignores_elements_whose_names_only_start_with_script() {
        let source =
            "<template><scripture>text</scripture></template>\n<script>const value = 1</script>";
        let extracted = vue_script_source(Path::new("Component.vue"), source).unwrap();

        assert!(!extracted.contains("text"));
        assert!(extracted.contains("const value = 1"));
    }
}
