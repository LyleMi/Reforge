use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use tree_sitter::{Node, Parser, Tree};

use super::{Comment, CommentKind, hash};

pub(crate) fn supported(path: &Path) -> bool {
    super::languages::language(path).is_some()
}

pub(super) fn parse(path: &Path, source: &str) -> Result<Tree> {
    if !supported(path) {
        bail!("unsupported comment language");
    }
    let language = super::languages::language(path).context("unsupported comment language")?;
    let mut parser = Parser::new();
    parser.set_language(&language)?;
    let tree = parser
        .parse(source, None)
        .context("parser returned no tree")?;
    if tree.root_node().has_error() {
        bail!("parse error; comment edits require a complete syntax tree");
    }
    Ok(tree)
}

pub(super) fn comments(
    path: &Path,
    relative: &str,
    source: &str,
    source_hash: &str,
) -> Result<Vec<Comment>> {
    let tree = parse(path, source)?;
    Ok(comments_from_tree(&tree, relative, source, source_hash))
}

pub(super) fn comments_from_tree(
    tree: &Tree,
    relative: &str,
    source: &str,
    source_hash: &str,
) -> Vec<Comment> {
    let mut nodes = Vec::new();
    collect(tree.root_node(), &mut nodes);
    let mut result = Vec::new();
    for node in nodes {
        let comment = raw_comment(node, relative, source, source_hash);
        if let Some(previous) = result.last_mut()
            && merge_lines(previous, &comment, source)
        {
            continue;
        }
        result.push(comment);
    }
    annotate_comments(&mut result, source);
    result
}

fn raw_comment(node: Node<'_>, path: &str, source: &str, source_hash: &str) -> Comment {
    let text = &source[node.byte_range()];
    let kind = if ["///", "//!", "/**", "/*!"]
        .iter()
        .any(|prefix| text.starts_with(prefix))
    {
        CommentKind::Documentation
    } else if !is_block(text) {
        CommentKind::Line
    } else {
        CommentKind::Block
    };
    Comment {
        id: String::new(),
        path: path.into(),
        source_hash: source_hash.into(),
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        line: node.start_position().row + 1,
        end_line: node.end_position().row + 1 - usize::from(text.ends_with('\n')),
        kind,
        text: text.into(),
        symbol: symbol(node, source),
        context_start_line: 0,
        context: String::new(),
        protected: false,
        reasons: vec![],
        suggestions: vec![],
    }
}

fn merge_lines(previous: &mut Comment, comment: &Comment, source: &str) -> bool {
    // Group standalone continuation lines so protection covers the whole note.
    if comment.kind != CommentKind::Line
        || previous.kind != CommentKind::Line
        || comment.line != previous.end_line + 1
        || !source[previous.end_byte..comment.start_byte]
            .chars()
            .all(char::is_whitespace)
        || !standalone(source, previous.start_byte)
        || !standalone(source, comment.start_byte)
    {
        return false;
    }
    previous.end_byte = comment.end_byte;
    previous.end_line = comment.end_line;
    previous.text = source[previous.start_byte..previous.end_byte].into();
    true
}

fn annotate_comments(comments: &mut [Comment], source: &str) {
    let mut counts = BTreeMap::<String, usize>::new();
    for comment in comments.iter() {
        *counts.entry(body(&comment.text)).or_default() += 1;
    }
    let lines: Vec<_> = source.lines().collect();
    for comment in comments {
        comment.id = format!(
            "comment-{}",
            hash(
                format!(
                    "{}\0{}\0{}\0{}",
                    comment.path, comment.source_hash, comment.start_byte, comment.end_byte
                )
                .as_bytes()
            )
        );
        comment.context_start_line = comment.line.saturating_sub(2).max(1);
        comment.context = lines
            [comment.context_start_line - 1..(comment.end_line + 2).min(lines.len())]
            .join("\n");
        comment.reasons = super::classify::protections(comment.kind, &comment.text);
        comment.protected = !comment.reasons.is_empty();
        if !comment.protected {
            let content = body(&comment.text);
            comment.suggestions = super::classify::suggestions(
                &content,
                counts.get(&content).copied().unwrap_or_default(),
            );
        }
    }
}

fn standalone(source: &str, start: usize) -> bool {
    let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
    source[line_start..start].trim().is_empty()
}

fn is_comment(node: Node<'_>) -> bool {
    if !matches!(
        node.kind(),
        "comment"
            | "line_comment"
            | "block_comment"
            | "multiline_comment"
            | "documentation_block_comment"
            | "html_comment"
            | "js_comment"
            | "marginalia"
    ) {
        return false;
    }
    // HTML's external scanner can emit comment extras inside quoted attributes.
    let mut parent = node.parent();
    while let Some(ancestor) = parent {
        if matches!(
            ancestor.kind(),
            "quoted_attribute_value" | "attribute_value"
        ) {
            return false;
        }
        parent = ancestor.parent();
    }
    true
}

fn collect<'a>(node: Node<'a>, result: &mut Vec<Node<'a>>) {
    if is_comment(node) {
        result.push(node);
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(child, result);
    }
}

fn symbol(node: Node<'_>, source: &str) -> Option<String> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind().contains("function") || parent.kind().contains("method") {
            if let Some(name) = parent.child_by_field_name("name") {
                return Some(source[name.byte_range()].into());
            }
            if let Some(owner) = parent.parent().and_then(|p| p.child_by_field_name("name")) {
                return Some(source[owner.byte_range()].into());
            }
        }
        current = parent.parent();
    }
    let mut sibling = node.next_named_sibling();
    while let Some(next) = sibling {
        if !is_comment(next) {
            return next
                .child_by_field_name("name")
                .map(|name| source[name.byte_range()].into());
        }
        sibling = next.next_named_sibling();
    }
    None
}

fn block_body(text: &str) -> Option<&str> {
    for (open, close) in [
        ("/*", "*/"),
        ("<#", "#>"),
        ("<!--", "-->"),
        ("=begin", "=end"),
    ] {
        if let Some(body) = text.strip_prefix(open).and_then(|s| s.strip_suffix(close)) {
            return Some(body);
        }
    }
    // Lua long comments may use any number of '=' characters.
    if let Some(rest) = text.strip_prefix("--[") {
        let count = rest.bytes().take_while(|byte| *byte == b'=').count();
        if let Some(rest) = rest[count..].strip_prefix('[') {
            return rest.strip_suffix(&format!("]{}]", "=".repeat(count)));
        }
    }
    None
}

fn is_block(text: &str) -> bool {
    block_body(text.trim()).is_some()
}

pub(super) fn body(text: &str) -> String {
    let text = text.trim();
    if let Some(block) = block_body(text) {
        return block
            .lines()
            .map(|line| line.trim().trim_start_matches('*').trim())
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .into();
    }
    text.lines()
        .map(|line| {
            let line = line.trim();
            ["//", "#", "--"]
                .iter()
                .find_map(|prefix| line.strip_prefix(prefix))
                .unwrap_or(line)
                .trim()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Include punctuation and tree shape, not just named nodes. This catches ASI
/// changes and token concatenation while excluding comment trivia and positions.
pub(super) fn syntax_signature(tree: &Tree, source: &str) -> Vec<String> {
    fn visit(node: Node<'_>, source: &str, output: &mut Vec<String>) {
        if is_comment(node) {
            return;
        }
        output.push(format!("({}", node.kind()));
        if node.child_count() == 0 {
            if node.parent().is_some() {
                output.push(source[node.byte_range()].into());
            }
        } else {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                visit(child, source, output);
            }
        }
        output.push(")".into());
    }
    let mut output = Vec::new();
    visit(tree.root_node(), source, &mut output);
    output
}
