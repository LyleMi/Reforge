use super::*;

pub(super) fn binding_name(node: Node<'_>, file: &ParsedSourceFile) -> Option<String> {
    if node.kind() == "identifier" {
        node_text(node, file)
    } else {
        None
    }
}

pub(super) fn find_node(root: Node<'_>, start: usize) -> Option<Node<'_>> {
    if root.start_byte() == start {
        return Some(root);
    }
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.start_byte() <= start
            && child.end_byte() >= start
            && let Some(found) = find_node(child, start)
        {
            return Some(found);
        }
    }
    None
}

pub(super) fn node_text(node: Node<'_>, file: &ParsedSourceFile) -> Option<String> {
    node.utf8_text(file.file.source.as_bytes())
        .ok()
        .map(str::to_owned)
}

pub(super) fn add_location(graph: &mut FlowGraph, location: FlowLocation) -> NodeId {
    let id = graph.nodes.len();
    graph.nodes.push(location);
    id
}

pub(super) fn stable_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn language(file: &ParsedSourceFile) -> &'static str {
    match file.family {
        LanguageFamily::JavaScriptTypeScript => match file
            .file
            .path
            .extension()
            .and_then(|extension| extension.to_str())
        {
            Some("ts" | "mts" | "cts") => TYPESCRIPT_LANGUAGE,
            Some("tsx" | "vue") => "tsx",
            _ => JAVASCRIPT_LANGUAGE,
        },
        LanguageFamily::Python => "python",
        _ => "unsupported",
    }
}

pub(super) fn is_literal(kind: &str) -> bool {
    kind.contains("string")
        || kind.contains("number")
        || kind.contains("integer")
        || kind.contains("float")
        || matches!(kind, "true" | "false" | "none" | "null" | "undefined")
}
