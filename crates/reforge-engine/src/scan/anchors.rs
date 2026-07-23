use std::collections::BTreeMap;

use crate::detectors::manifest::{observation_source, subject_kind};
use crate::detectors::similarity::{ParsedSourceFile, SourceFile};
use crate::model::{
    DetectedEvidence, ObservationSource, RawMetrics, Rule, SubjectKind, serialized_rule,
};

pub(super) fn assign_stable_anchors(
    detections: &mut [DetectedEvidence],
    raw_metrics: &RawMetrics,
    sources: &[SourceFile],
    parsed_sources: &[ParsedSourceFile],
) {
    let source_by_path = sources
        .iter()
        .map(|source| (source.display_path.as_str(), source.source.as_ref()))
        .collect::<BTreeMap<_, _>>();
    let qualified_symbols = qualified_symbol_index(parsed_sources);
    let lookups = AnchorLookups {
        sources: &source_by_path,
        qualified_symbols: &qualified_symbols,
    };
    for detection in detections {
        detection.semantic_anchor = detection_anchor(
            detection,
            raw_metrics,
            lookups.sources,
            lookups.qualified_symbols,
        );
        detection.normalize_flow_anchor();
    }
}

fn detection_anchor(
    detection: &DetectedEvidence,
    raw_metrics: &RawMetrics,
    sources: &BTreeMap<&str, &str>,
    qualified_symbols: &BTreeMap<String, String>,
) -> String {
    if let Some(witness) = &detection.flow_witness {
        return format!(
            "flow:{}:{}:{}",
            witness.policy, witness.source.id, witness.sink.id
        );
    }
    if detection.kind == Rule::DebtMarker {
        return text_anchor(&detection.path, detection.line, sources);
    }
    match subject_kind(detection.kind) {
        SubjectKind::Repository => format!("repository:{}", serialized_rule(detection.kind)),
        SubjectKind::Directory => format!("directory:{}", normalize_anchor_path(&detection.path)),
        SubjectKind::File => format!("file:{}", normalize_anchor_path(&detection.path)),
        SubjectKind::Symbol if observation_source(detection.kind) == ObservationSource::Types => {
            symbol_anchor(
                "type",
                &detection.path,
                detection.line,
                raw_metrics.types.iter().map(|metric| SymbolMetric {
                    path: &metric.path,
                    name: &metric.name,
                    line: metric.line,
                    loc: metric.loc,
                }),
                AnchorLookups {
                    sources,
                    qualified_symbols,
                },
            )
        }
        SubjectKind::Symbol => symbol_anchor(
            "function",
            &detection.path,
            detection.line,
            raw_metrics.functions.iter().map(|metric| SymbolMetric {
                path: &metric.path,
                name: &metric.name,
                line: metric.line,
                loc: metric.loc,
            }),
            AnchorLookups {
                sources,
                qualified_symbols,
            },
        ),
        SubjectKind::Group => group_anchor(detection, raw_metrics, sources, qualified_symbols),
    }
}

#[derive(Clone, Copy)]
struct SymbolMetric<'a> {
    path: &'a str,
    name: &'a str,
    line: usize,
    loc: usize,
}

#[derive(Clone, Copy)]
struct AnchorLookups<'a> {
    sources: &'a BTreeMap<&'a str, &'a str>,
    qualified_symbols: &'a BTreeMap<String, String>,
}

fn symbol_anchor<'a>(
    scope: &str,
    path: &str,
    line: Option<usize>,
    metrics: impl Iterator<Item = SymbolMetric<'a>>,
    lookups: AnchorLookups<'_>,
) -> String {
    let metrics = metrics
        .filter(|metric| metric.path == path)
        .collect::<Vec<_>>();
    let selected = line.and_then(|line| {
        metrics
            .iter()
            .filter(|metric| metric.line <= line && line < metric.line + metric.loc.max(1))
            .max_by_key(|metric| metric.line)
            .copied()
            .or_else(|| metrics.iter().find(|metric| metric.line == line).copied())
    });
    let Some(selected) = selected else {
        return format!(
            "{scope}:{}:{}",
            normalize_anchor_path(path),
            text_anchor(path, line, lookups.sources)
        );
    };
    let qualified = qualified_name(
        scope,
        selected.path,
        selected.line,
        selected.name,
        lookups.qualified_symbols,
    );
    let ordinal = metrics
        .iter()
        .filter(|metric| {
            metric.line <= selected.line
                && qualified_name(
                    scope,
                    metric.path,
                    metric.line,
                    metric.name,
                    lookups.qualified_symbols,
                ) == qualified
        })
        .count();
    format!(
        "{scope}:{}::{}#{ordinal}",
        normalize_anchor_path(path),
        qualified
    )
}

fn group_anchor(
    detection: &DetectedEvidence,
    raw_metrics: &RawMetrics,
    sources: &BTreeMap<&str, &str>,
    qualified_symbols: &BTreeMap<String, String>,
) -> String {
    let mut members = std::iter::once((detection.path.as_str(), detection.line))
        .chain(
            detection
                .related_locations
                .iter()
                .map(|location| (location.path.as_str(), Some(location.line))),
        )
        .map(|(path, line)| location_anchor(path, line, raw_metrics, sources, qualified_symbols))
        .collect::<Vec<_>>();
    members.sort();
    members.dedup();
    format!(
        "group:{}:{}",
        serialized_rule(detection.kind),
        members.join("|")
    )
}

fn location_anchor(
    path: &str,
    line: Option<usize>,
    raw_metrics: &RawMetrics,
    sources: &BTreeMap<&str, &str>,
    qualified_symbols: &BTreeMap<String, String>,
) -> String {
    let functions = raw_metrics
        .functions
        .iter()
        .map(|metric| SymbolMetric {
            path: &metric.path,
            name: &metric.name,
            line: metric.line,
            loc: metric.loc,
        })
        .collect::<Vec<_>>();
    if line.is_some_and(|line| {
        functions.iter().any(|metric| {
            metric.path == path && metric.line <= line && line < metric.line + metric.loc.max(1)
        })
    }) {
        return symbol_anchor(
            "function",
            path,
            line,
            functions.into_iter(),
            AnchorLookups {
                sources,
                qualified_symbols,
            },
        );
    }
    let types = raw_metrics
        .types
        .iter()
        .map(|metric| SymbolMetric {
            path: &metric.path,
            name: &metric.name,
            line: metric.line,
            loc: metric.loc,
        })
        .collect::<Vec<_>>();
    if line.is_some_and(|line| {
        types.iter().any(|metric| {
            metric.path == path && metric.line <= line && line < metric.line + metric.loc.max(1)
        })
    }) {
        return symbol_anchor(
            "type",
            path,
            line,
            types.into_iter(),
            AnchorLookups {
                sources,
                qualified_symbols,
            },
        );
    }
    text_anchor(path, line, sources)
}

fn text_anchor(path: &str, line: Option<usize>, sources: &BTreeMap<&str, &str>) -> String {
    let Some(source) = sources.get(path) else {
        return format!("text:{}:unavailable", normalize_anchor_path(path));
    };
    let line_index = line.unwrap_or(1).saturating_sub(1);
    let normalized_lines = source.lines().map(normalize_text).collect::<Vec<_>>();
    let content = normalized_lines
        .get(line_index)
        .filter(|value| !value.is_empty())
        .cloned()
        .unwrap_or_else(|| "empty".into());
    let ordinal = normalized_lines
        .iter()
        .take(line_index + 1)
        .filter(|candidate| **candidate == content)
        .count();
    format!("text:{}:{content}#{ordinal}", normalize_anchor_path(path))
}

fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_symbol(symbol: &str) -> String {
    symbol.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn qualified_name(
    scope: &str,
    path: &str,
    line: usize,
    name: &str,
    qualified_symbols: &BTreeMap<String, String>,
) -> String {
    qualified_symbols
        .get(&symbol_key(scope, path, line, name))
        .cloned()
        .unwrap_or_else(|| normalize_symbol(name))
}

fn qualified_symbol_index(parsed_sources: &[ParsedSourceFile]) -> BTreeMap<String, String> {
    let mut index = BTreeMap::new();
    for parsed in parsed_sources {
        collect_qualified_symbols(
            parsed.tree.root_node(),
            parsed.file.source.as_ref(),
            &parsed.file.display_path,
            &mut index,
        );
    }
    index
}

fn collect_qualified_symbols(
    node: tree_sitter::Node<'_>,
    source: &str,
    path: &str,
    index: &mut BTreeMap<String, String>,
) {
    if let Some(scope) = symbol_scope(node.kind())
        && let Some(name) = node
            .child_by_field_name("name")
            .and_then(|name| name.utf8_text(source.as_bytes()).ok())
    {
        let mut segments = Vec::new();
        let mut parent = node.parent();
        while let Some(container) = parent {
            if is_symbol_container(container.kind())
                && let Some(segment) = container
                    .child_by_field_name("name")
                    .or_else(|| container.child_by_field_name("type"))
                    .and_then(|name| name.utf8_text(source.as_bytes()).ok())
            {
                segments.push(normalize_symbol(segment));
            }
            parent = container.parent();
        }
        segments.reverse();
        segments.push(normalize_symbol(name));
        index.insert(
            symbol_key(scope, path, node.start_position().row + 1, name),
            segments.join("::"),
        );
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_qualified_symbols(child, source, path, index);
    }
}

fn symbol_scope(kind: &str) -> Option<&'static str> {
    if matches!(
        kind,
        "function_item"
            | "function_declaration"
            | "function_definition"
            | "method_definition"
            | "method_declaration"
            | "constructor_declaration"
            | "local_function_statement"
            | "singleton_method"
            | "method"
            | "function_statement"
    ) {
        Some("function")
    } else if kind.contains("class")
        || kind.contains("struct")
        || kind.contains("interface")
        || kind.contains("enum")
        || kind.contains("trait")
        || kind == "type_declaration"
    {
        Some("type")
    } else {
        None
    }
}

fn is_symbol_container(kind: &str) -> bool {
    kind.contains("class")
        || kind.contains("struct")
        || kind.contains("interface")
        || kind.contains("trait")
        || kind.contains("namespace")
        || kind.contains("module")
        || kind.contains("object")
        || kind == "impl_item"
}

fn symbol_key(scope: &str, path: &str, line: usize, name: &str) -> String {
    format!(
        "{scope}\0{}\0{line}\0{}",
        normalize_anchor_path(path),
        normalize_symbol(name)
    )
}

fn normalize_anchor_path(path: &str) -> String {
    crate::pathing::normalize_path_text(path)
        .trim_start_matches("./")
        .to_string()
}
