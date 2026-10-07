//! Cleanup grammars are independent of analyzer/dataflow language coverage.
use std::path::Path;

use tree_sitter::Language;

pub(super) fn language(path: &Path) -> Option<Language> {
    let extension = path
        .extension()
        .and_then(|part| part.to_str())
        .unwrap_or("");
    let name = path
        .file_name()
        .and_then(|part| part.to_str())
        .unwrap_or("");
    let language = match extension {
        "c" | "h" => tree_sitter_c::LANGUAGE.into(),
        "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" | "C" | "H" => tree_sitter_cpp::LANGUAGE.into(),
        "dart" => tree_sitter_dart::LANGUAGE.into(),
        "swift" => tree_sitter_swift::LANGUAGE.into(),
        "lua" => tree_sitter_lua::LANGUAGE.into(),
        "scala" | "sc" => tree_sitter_scala::LANGUAGE.into(),
        "r" | "R" => tree_sitter_r::LANGUAGE.into(),
        "html" | "htm" => tree_sitter_html::LANGUAGE.into(),
        "css" => tree_sitter_css::LANGUAGE.into(),
        "json" | "jsonc" => tree_sitter_json::LANGUAGE.into(),
        "yaml" | "yml" => tree_sitter_yaml::LANGUAGE.into(),
        "toml" => tree_sitter_toml_ng::LANGUAGE.into(),
        "sql" => tree_sitter_sequel::LANGUAGE.into(),
        "pyi" | "pyw" => tree_sitter_python::LANGUAGE.into(),
        "kts" => tree_sitter_kotlin_ng::LANGUAGE.into(),
        "phtml" => tree_sitter_php::LANGUAGE_PHP.into(),
        "rake" | "gemspec" => tree_sitter_ruby::LANGUAGE.into(),
        "psd1" => tree_sitter_powershell::LANGUAGE.into(),
        // Vue needs embedded-language parsing; never treat a component as TSX.
        "vue" => return None,
        _ if matches!(name, "Gemfile" | "Rakefile" | "Vagrantfile") => {
            tree_sitter_ruby::LANGUAGE.into()
        }
        _ => return crate::lang::adapter_for_path(path).map(|adapter| adapter.language()),
    };
    Some(language)
}
