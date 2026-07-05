use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tree_sitter::{Language, Node, Parser};

use crate::scanner::{Finding, FindingKind, RelatedLocation, Severity};

type TokenId = u32;

#[derive(Debug, Clone)]
pub struct SimilarFunctionOptions {
    pub min_group_size: usize,
    pub min_tokens: usize,
    pub threshold: f64,
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub display_path: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct SimilarFunctionScan {
    pub findings: Vec<Finding>,
    pub candidate_count: usize,
}

#[derive(Debug, Clone)]
struct FunctionCandidate {
    family: LanguageFamily,
    category: FunctionCategory,
    name: String,
    path: String,
    line: usize,
    tokens: Vec<TokenId>,
    token_counts: Vec<(TokenId, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LanguageFamily {
    Rust,
    JavaScriptTypeScript,
    Python,
    Go,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FunctionCategory {
    Function,
    Method,
}

#[derive(Debug, Clone, Copy)]
struct LanguageAdapter {
    family: LanguageFamily,
    language: fn() -> Language,
}

#[derive(Debug, Default)]
struct TokenInterner {
    ids_by_token: HashMap<String, TokenId>,
}

impl TokenInterner {
    fn intern(&mut self, token: &str) -> TokenId {
        if let Some(id) = self.ids_by_token.get(token) {
            return *id;
        }

        let id = self.ids_by_token.len() as TokenId;
        self.ids_by_token.insert(token.to_string(), id);
        id
    }
}

#[allow(dead_code)]
pub fn scan_similar_functions(
    files: &[SourceFile],
    options: &SimilarFunctionOptions,
) -> Result<Vec<Finding>> {
    Ok(scan_similar_functions_report(files, options)?.findings)
}

pub fn scan_similar_functions_report(
    files: &[SourceFile],
    options: &SimilarFunctionOptions,
) -> Result<SimilarFunctionScan> {
    validate_options(options)?;

    if options.min_group_size == 0 {
        return Ok(SimilarFunctionScan {
            findings: Vec::new(),
            candidate_count: 0,
        });
    }

    let mut candidates = Vec::new();
    let mut interner = TokenInterner::default();
    for file in files {
        if let Some(adapter) = adapter_for_path(&file.path) {
            candidates.extend(extract_candidates(
                file,
                adapter,
                options.min_tokens,
                &mut interner,
            )?);
        }
    }

    let candidate_count = candidates.len();
    Ok(SimilarFunctionScan {
        findings: group_candidates(&candidates, options),
        candidate_count,
    })
}

pub fn is_supported_similarity_source(path: &Path) -> bool {
    adapter_for_path(path).is_some()
}

fn validate_options(options: &SimilarFunctionOptions) -> Result<()> {
    if !(0.0..=1.0).contains(&options.threshold) {
        bail!("--function-similarity must be between 0.0 and 1.0");
    }

    Ok(())
}

fn adapter_for_path(path: &Path) -> Option<LanguageAdapter> {
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

fn extract_candidates(
    file: &SourceFile,
    adapter: LanguageAdapter,
    min_tokens: usize,
    interner: &mut TokenInterner,
) -> Result<Vec<FunctionCandidate>> {
    let mut parser = Parser::new();
    parser
        .set_language(&(adapter.language)())
        .with_context(|| format!("failed to load parser for {}", file.display_path))?;

    let Some(tree) = parser.parse(&file.source, None) else {
        return Ok(Vec::new());
    };

    if tree.root_node().has_error() {
        return Ok(Vec::new());
    }

    let mut candidates = Vec::new();
    collect_named_functions(
        tree.root_node(),
        &file.source,
        file,
        adapter.family,
        min_tokens,
        interner,
        &mut candidates,
    );
    Ok(candidates)
}

fn collect_named_functions(
    node: Node<'_>,
    source: &str,
    file: &SourceFile,
    family: LanguageFamily,
    min_tokens: usize,
    interner: &mut TokenInterner,
    candidates: &mut Vec<FunctionCandidate>,
) {
    if let Some((name, category, body)) = extract_function_parts(node, source, family) {
        let tokens = normalize_tokens(body, source.as_bytes(), interner);
        if tokens.len() >= min_tokens {
            let token_counts = token_counts(&tokens);
            candidates.push(FunctionCandidate {
                family,
                category,
                name,
                path: file.display_path.clone(),
                line: node.start_position().row + 1,
                tokens,
                token_counts,
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_named_functions(
            child, source, file, family, min_tokens, interner, candidates,
        );
    }
}

fn extract_function_parts<'tree>(
    node: Node<'tree>,
    source: &str,
    family: LanguageFamily,
) -> Option<(String, FunctionCategory, Node<'tree>)> {
    let kind = node.kind();

    match family {
        LanguageFamily::Rust if kind == "function_item" => {
            let name = node
                .child_by_field_name("name")?
                .utf8_text(source.as_bytes())
                .ok()?;
            let body = node.child_by_field_name("body")?;
            let category = if has_ancestor_kind(node, "impl_item") {
                FunctionCategory::Method
            } else {
                FunctionCategory::Function
            };
            Some((name.to_string(), category, body))
        }
        LanguageFamily::JavaScriptTypeScript
            if matches!(
                kind,
                "function_declaration" | "generator_function_declaration" | "method_definition"
            ) =>
        {
            let name = node
                .child_by_field_name("name")?
                .utf8_text(source.as_bytes())
                .ok()?;
            let body = node.child_by_field_name("body")?;
            let category = if kind == "method_definition" {
                FunctionCategory::Method
            } else {
                FunctionCategory::Function
            };
            Some((name.to_string(), category, body))
        }
        LanguageFamily::Python if kind == "function_definition" => {
            let name = node
                .child_by_field_name("name")?
                .utf8_text(source.as_bytes())
                .ok()?;
            let body = node.child_by_field_name("body")?;
            Some((name.to_string(), FunctionCategory::Function, body))
        }
        LanguageFamily::Go if matches!(kind, "function_declaration" | "method_declaration") => {
            let name = node
                .child_by_field_name("name")?
                .utf8_text(source.as_bytes())
                .ok()?;
            let body = node.child_by_field_name("body")?;
            let category = if kind == "method_declaration" {
                FunctionCategory::Method
            } else {
                FunctionCategory::Function
            };
            Some((name.to_string(), category, body))
        }
        _ => None,
    }
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

fn normalize_tokens(node: Node<'_>, source: &[u8], interner: &mut TokenInterner) -> Vec<TokenId> {
    let mut tokens = Vec::new();
    normalize_node(node, source, interner, &mut tokens);
    tokens
}

fn normalize_node(
    node: Node<'_>,
    source: &[u8],
    interner: &mut TokenInterner,
    tokens: &mut Vec<TokenId>,
) {
    let kind = node.kind();

    if is_comment_kind(kind) {
        return;
    }

    if is_identifier_kind(kind) {
        tokens.push(interner.intern("ID"));
        return;
    }

    if is_string_kind(kind) {
        tokens.push(interner.intern("STR"));
        return;
    }

    if is_number_kind(kind) {
        tokens.push(interner.intern("NUM"));
        return;
    }

    if node.child_count() == 0 {
        if node.is_named() {
            tokens.push(interner.intern(kind));
        } else if let Ok(text) = node.utf8_text(source) {
            if !text.trim().is_empty() {
                tokens.push(interner.intern(text));
            }
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        normalize_node(child, source, interner, tokens);
    }
}

fn token_counts(tokens: &[TokenId]) -> Vec<(TokenId, usize)> {
    let mut counts = BTreeMap::new();
    for token in tokens {
        *counts.entry(*token).or_insert(0) += 1;
    }
    counts.into_iter().collect()
}

fn is_comment_kind(kind: &str) -> bool {
    kind.contains("comment")
}

fn is_identifier_kind(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "field_identifier"
            | "property_identifier"
            | "shorthand_property_identifier"
            | "type_identifier"
            | "scoped_identifier"
            | "self"
    )
}

fn is_string_kind(kind: &str) -> bool {
    kind.contains("string") || matches!(kind, "raw_string_literal" | "interpreted_string_literal")
}

fn is_number_kind(kind: &str) -> bool {
    kind.contains("number")
        || kind.contains("integer")
        || kind.contains("float")
        || kind == "imaginary_literal"
}

fn group_candidates(
    candidates: &[FunctionCandidate],
    options: &SimilarFunctionOptions,
) -> Vec<Finding> {
    let mut buckets: BTreeMap<(LanguageFamily, FunctionCategory), Vec<usize>> = BTreeMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        buckets
            .entry((candidate.family, candidate.category))
            .or_default()
            .push(index);
    }

    let mut findings = Vec::new();
    for indexes in buckets.values() {
        let mut used = vec![false; candidates.len()];

        for &representative_index in indexes {
            if used[representative_index] {
                continue;
            }

            let representative = &candidates[representative_index];
            let mut group = vec![representative_index];

            for &candidate_index in indexes {
                if candidate_index == representative_index || used[candidate_index] {
                    continue;
                }

                let candidate = &candidates[candidate_index];
                if length_ratio(representative.tokens.len(), candidate.tokens.len())
                    < options.threshold
                {
                    continue;
                }

                if multiset_dice_upper_bound(representative, candidate) < options.threshold {
                    continue;
                }

                if token_similarity_reaches(
                    &representative.tokens,
                    &candidate.tokens,
                    options.threshold,
                ) {
                    group.push(candidate_index);
                }
            }

            if group.len() >= options.min_group_size {
                for &index in &group {
                    used[index] = true;
                }
                findings.push(similar_function_finding(
                    candidates,
                    &group,
                    options.threshold,
                ));
            }
        }
    }

    findings
}

fn length_ratio(left_len: usize, right_len: usize) -> f64 {
    let shorter = left_len.min(right_len) as f64;
    let longer = left_len.max(right_len) as f64;

    if longer == 0.0 { 1.0 } else { shorter / longer }
}

fn multiset_dice_upper_bound(left: &FunctionCandidate, right: &FunctionCandidate) -> f64 {
    if left.tokens.is_empty() && right.tokens.is_empty() {
        return 1.0;
    }

    let mut overlap = 0;
    let mut left_index = 0;
    let mut right_index = 0;

    while left_index < left.token_counts.len() && right_index < right.token_counts.len() {
        let (left_token, left_count) = left.token_counts[left_index];
        let (right_token, right_count) = right.token_counts[right_index];

        match left_token.cmp(&right_token) {
            std::cmp::Ordering::Less => left_index += 1,
            std::cmp::Ordering::Greater => right_index += 1,
            std::cmp::Ordering::Equal => {
                overlap += left_count.min(right_count);
                left_index += 1;
                right_index += 1;
            }
        }
    }

    (2.0 * overlap as f64) / (left.tokens.len() as f64 + right.tokens.len() as f64)
}

fn token_similarity_reaches(left: &[TokenId], right: &[TokenId], threshold: f64) -> bool {
    if left.is_empty() && right.is_empty() {
        return true;
    }

    let required_lcs = required_lcs_len(left.len(), right.len(), threshold);
    longest_common_subsequence_reaches(left, right, required_lcs)
}

fn required_lcs_len(left_len: usize, right_len: usize, threshold: f64) -> usize {
    ((threshold * (left_len + right_len) as f64) / 2.0).ceil() as usize
}

fn longest_common_subsequence_reaches(
    left: &[TokenId],
    right: &[TokenId],
    required_lcs: usize,
) -> bool {
    if required_lcs == 0 {
        return true;
    }

    if left.len().min(right.len()) < required_lcs {
        return false;
    }

    let mut previous = vec![0; right.len() + 1];
    let mut current = vec![0; right.len() + 1];

    for (left_index, left_token) in left.iter().enumerate() {
        for (right_index, right_token) in right.iter().enumerate() {
            current[right_index + 1] = if left_token == right_token {
                previous[right_index] + 1
            } else {
                previous[right_index + 1].max(current[right_index])
            };
        }
        std::mem::swap(&mut previous, &mut current);
        current.fill(0);

        if previous[right.len()] >= required_lcs {
            return true;
        }

        let remaining_left = left.len() - left_index - 1;
        if previous[right.len()] + remaining_left < required_lcs {
            return false;
        }
    }

    previous[right.len()] >= required_lcs
}

fn similar_function_finding(
    candidates: &[FunctionCandidate],
    group: &[usize],
    threshold: f64,
) -> Finding {
    let representative = &candidates[group[0]];
    let related_locations = group
        .iter()
        .map(|&index| {
            let candidate = &candidates[index];
            RelatedLocation {
                path: candidate.path.clone(),
                line: candidate.line,
                name: Some(candidate.name.clone()),
            }
        })
        .collect::<Vec<_>>();

    Finding {
        kind: FindingKind::SimilarFunctions,
        severity: Severity::Warning,
        path: representative.path.clone(),
        line: Some(representative.line),
        magnitude: Some(group.len()),
        message: format!(
            "{} structurally similar functions/methods found at similarity >= {:.2}",
            group.len(),
            threshold
        ),
        related_locations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_file(path: &str, source: &str) -> SourceFile {
        SourceFile {
            path: PathBuf::from(path),
            display_path: path.to_string(),
            source: source.to_string(),
        }
    }

    fn options() -> SimilarFunctionOptions {
        SimilarFunctionOptions {
            min_group_size: 3,
            min_tokens: 12,
            threshold: 0.80,
        }
    }

    #[test]
    fn detects_similar_rust_functions() -> Result<()> {
        let source = r#"
fn alpha(items: &[i32]) -> i32 {
    let mut total = 0;
    for item in items {
        if *item > 10 {
            total += *item * 2;
        } else {
            total += *item;
        }
    }
    total
}

fn beta(values: &[i32]) -> i32 {
    let mut sum = 0;
    for value in values {
        if *value > 20 {
            sum += *value * 2;
        } else {
            sum += *value;
        }
    }
    sum
}

fn gamma(numbers: &[i32]) -> i32 {
    let mut acc = 0;
    for number in numbers {
        if *number > 30 {
            acc += *number * 2;
        } else {
            acc += *number;
        }
    }
    acc
}
"#;

        let findings = scan_similar_functions(&[source_file("src/lib.rs", source)], &options())?;

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].magnitude, Some(3));
        Ok(())
    }

    #[test]
    fn ignores_short_trivial_functions() -> Result<()> {
        let source = "fn a() { 1 }\nfn b() { 2 }\nfn c() { 3 }\n";

        let findings = scan_similar_functions(&[source_file("src/lib.rs", source)], &options())?;

        assert!(findings.is_empty());
        Ok(())
    }

    #[test]
    fn ignores_anonymous_javascript_callbacks() -> Result<()> {
        let source = r#"
items.map(function (item) {
  const total = item.value + 10;
  if (total > 20) {
    return total * 2;
  }
  return total;
});
items.map((entry) => {
  const sum = entry.value + 10;
  if (sum > 20) {
    return sum * 2;
  }
  return sum;
});
"#;

        let findings = scan_similar_functions(&[source_file("src/app.js", source)], &options())?;

        assert!(findings.is_empty());
        Ok(())
    }

    #[test]
    fn detects_similar_javascript_functions_with_normalized_names_and_literals() -> Result<()> {
        let source = javascript_three_similar_functions();

        let findings = scan_similar_functions(&[source_file("src/app.js", source)], &options())?;

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].magnitude, Some(3));
        assert_eq!(findings[0].related_locations.len(), 3);
        Ok(())
    }

    #[test]
    fn detects_similar_typescript_functions() -> Result<()> {
        let source = r#"
function alpha(items: Item[]): number {
  let total = 0;
  for (const item of items) {
    if (item.score > 10) {
      total += item.score * 2;
    } else {
      total += item.score;
    }
  }
  return total;
}

function beta(records: Item[]): number {
  let sum = 1;
  for (const record of records) {
    if (record.score > 20) {
      sum += record.score * 2;
    } else {
      sum += record.score;
    }
  }
  return sum;
}

function gamma(rows: Item[]): number {
  let acc = 2;
  for (const row of rows) {
    if (row.score > 30) {
      acc += row.score * 2;
    } else {
      acc += row.score;
    }
  }
  return acc;
}
"#;

        let findings = scan_similar_functions(&[source_file("src/app.ts", source)], &options())?;

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].magnitude, Some(3));
        Ok(())
    }

    #[test]
    fn detects_similar_python_functions() -> Result<()> {
        let source = r#"
def alpha(items):
    total = 0
    for item in items:
        if item.score > 10:
            total += item.score * 2
        else:
            total += item.score
    return total

async def beta(records):
    sum_value = 1
    for record in records:
        if record.score > 20:
            sum_value += record.score * 2
        else:
            sum_value += record.score
    return sum_value

def gamma(rows):
    acc = 2
    for row in rows:
        if row.score > 30:
            acc += row.score * 2
        else:
            acc += row.score
    return acc
"#;

        let findings = scan_similar_functions(&[source_file("src/app.py", source)], &options())?;

        assert_eq!(findings.len(), 1);
        Ok(())
    }

    #[test]
    fn detects_similar_go_functions() -> Result<()> {
        let source = r#"
package app

func Alpha(items []Item) int {
    total := 0
    for _, item := range items {
        if item.Score > 10 {
            total += item.Score * 2
        } else {
            total += item.Score
        }
    }
    return total
}

func Beta(records []Item) int {
    sum := 1
    for _, record := range records {
        if record.Score > 20 {
            sum += record.Score * 2
        } else {
            sum += record.Score
        }
    }
    return sum
}

func Gamma(rows []Item) int {
    acc := 2
    for _, row := range rows {
        if row.Score > 30 {
            acc += row.Score * 2
        } else {
            acc += row.Score
        }
    }
    return acc
}
"#;

        let findings = scan_similar_functions(&[source_file("src/app.go", source)], &options())?;

        assert_eq!(findings.len(), 1);
        Ok(())
    }

    #[test]
    fn requires_minimum_group_size() -> Result<()> {
        let source = r#"
function alpha(items) {
  let total = 0;
  for (const item of items) {
    total += item.score;
  }
  return total;
}

function beta(records) {
  let sum = 1;
  for (const record of records) {
    sum += record.score;
  }
  return sum;
}
"#;

        let findings = scan_similar_functions(&[source_file("src/app.js", source)], &options())?;

        assert!(findings.is_empty());
        Ok(())
    }

    #[test]
    fn same_names_with_different_structure_are_not_grouped() -> Result<()> {
        let files = vec![
            source_file(
                "src/a.js",
                r#"
function process(items) {
  let total = 0;
  for (const item of items) {
    total += item.score;
  }
  return total;
}
"#,
            ),
            source_file(
                "src/b.js",
                r#"
function process(items) {
  const names = [];
  for (const item of items) {
    names.push(item.name.toUpperCase());
  }
  return names.join(",");
}
"#,
            ),
            source_file(
                "src/c.js",
                r#"
function process(items) {
  const map = new Map();
  for (const item of items) {
    map.set(item.id, item);
  }
  return map;
}
"#,
            ),
        ];

        let mut strict_options = options();
        strict_options.threshold = 0.95;
        let findings = scan_similar_functions(&files, &strict_options)?;

        assert!(findings.is_empty());
        Ok(())
    }

    #[test]
    fn custom_threshold_changes_detection() -> Result<()> {
        let source = r#"
function alpha(items) {
  let total = 0;
  for (const item of items) {
    if (item.score > 10) total += item.score * 2;
  }
  return total;
}
function beta(items) {
  let total = 0;
  for (const item of items) {
    if (item.score > 10) total += item.score * 2;
  }
  return total;
}
function gamma(items) {
  let total = 0;
  for (const item of items) {
    while (item.active) {
      total += item.score * 2;
      break;
    }
  }
  return total;
}
"#;

        let mut relaxed = options();
        relaxed.threshold = 0.60;
        let mut strict = options();
        strict.threshold = 0.95;

        assert_eq!(
            scan_similar_functions(&[source_file("src/app.js", source)], &relaxed)?.len(),
            1
        );
        assert!(scan_similar_functions(&[source_file("src/app.js", source)], &strict)?.is_empty());
        Ok(())
    }

    #[test]
    fn reports_candidate_count() -> Result<()> {
        let scan = scan_similar_functions_report(
            &[source_file(
                "src/app.js",
                javascript_three_similar_functions(),
            )],
            &options(),
        )?;

        assert_eq!(scan.candidate_count, 3);
        assert_eq!(scan.findings.len(), 1);
        Ok(())
    }

    #[test]
    fn length_ratio_pruning_keeps_matching_group() -> Result<()> {
        let source = javascript_three_similar_functions();

        let findings = scan_similar_functions(&[source_file("src/app.js", source)], &options())?;

        assert_eq!(findings.len(), 1);
        Ok(())
    }

    #[test]
    fn multiset_pruning_keeps_matching_group() -> Result<()> {
        let source = r#"
function alpha(items) {
  let total = 0;
  for (const item of items) {
    total += item.score;
    total += item.weight;
  }
  return total;
}
function beta(records) {
  let sum = 1;
  for (const record of records) {
    sum += record.score;
    sum += record.weight;
  }
  return sum;
}
function gamma(rows) {
  let acc = 2;
  for (const row of rows) {
    acc += row.score;
    acc += row.weight;
  }
  return acc;
}
"#;

        let findings = scan_similar_functions(&[source_file("src/app.js", source)], &options())?;

        assert_eq!(findings.len(), 1);
        Ok(())
    }

    fn javascript_three_similar_functions() -> &'static str {
        r#"
function alpha(items) {
  let total = 0;
  for (const item of items) {
    if (item.score > 10) {
      total += item.score * 2;
    } else {
      total += item.score;
    }
  }
  return total;
}

function beta(records) {
  let sum = 1;
  for (const record of records) {
    if (record.score > 20) {
      sum += record.score * 2;
    } else {
      sum += record.score;
    }
  }
  return sum;
}

function gamma(rows) {
  let acc = 2;
  for (const row of rows) {
    if (row.score > 30) {
      acc += row.score * 2;
    } else {
      acc += row.score;
    }
  }
  return acc;
}
"#
    }
}
