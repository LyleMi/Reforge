use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tree_sitter::{Node, Parser};

use crate::language::{
    BODY_FIELD, FUNCTION_DECLARATION, FUNCTION_DEFINITION, FUNCTION_ITEM,
    GENERATOR_FUNCTION_DECLARATION, LanguageAdapter, LanguageFamily, METHOD_DECLARATION,
    METHOD_DEFINITION, NAME_FIELD, adapter_for_path,
};
use crate::scanner::{Finding, FindingKind, RelatedLocation, severity_for_threshold};

type TokenId = u32;

#[derive(Debug, Clone)]
pub struct SimilarFunctionOptions {
    pub min_group_size: usize,
    pub min_tokens: usize,
    pub threshold: f64,
    pub include_test_similarity: bool,
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

pub trait SimilarFunctionProgress {
    fn report_extract_progress(&mut self, _completed: usize, _total: usize, _path: &str) {}

    fn report_compare_progress(&mut self, _completed: usize, _total: usize) {}
}

struct NoopSimilarityProgress;

impl SimilarFunctionProgress for NoopSimilarityProgress {}

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
enum FunctionCategory {
    Function,
    Method,
}

#[derive(Debug, Clone, Copy)]
struct CandidateExtraction<'a> {
    source: &'a str,
    file: &'a SourceFile,
    family: LanguageFamily,
    min_tokens: usize,
    include_test_similarity: bool,
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
    let mut progress = NoopSimilarityProgress;
    scan_similar_functions_report_with_progress(files, options, &mut progress)
}

pub fn scan_similar_functions_report_with_progress(
    files: &[SourceFile],
    options: &SimilarFunctionOptions,
    progress: &mut dyn SimilarFunctionProgress,
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
    for (index, file) in files.iter().enumerate() {
        if let Some(adapter) = adapter_for_path(&file.path) {
            candidates.extend(extract_candidates(
                file,
                adapter,
                options.min_tokens,
                options.include_test_similarity,
                &mut interner,
            )?);
        }
        progress.report_extract_progress(index + 1, files.len(), &file.display_path);
    }

    let candidate_count = candidates.len();
    Ok(SimilarFunctionScan {
        findings: group_candidates(&candidates, options, progress),
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

fn extract_candidates(
    file: &SourceFile,
    adapter: LanguageAdapter,
    min_tokens: usize,
    include_test_similarity: bool,
    interner: &mut TokenInterner,
) -> Result<Vec<FunctionCandidate>> {
    let mut parser = Parser::new();
    parser
        .set_language(&adapter.language())
        .with_context(|| format!("failed to load parser for {}", file.display_path))?;

    let Some(tree) = parser.parse(&file.source, None) else {
        return Ok(Vec::new());
    };

    if tree.root_node().has_error() {
        return Ok(Vec::new());
    }

    let extraction = CandidateExtraction {
        source: &file.source,
        file,
        family: adapter.family,
        min_tokens,
        include_test_similarity,
    };
    let mut candidates = Vec::new();
    collect_named_functions(tree.root_node(), extraction, interner, &mut candidates);
    Ok(candidates)
}

fn collect_named_functions(
    node: Node<'_>,
    extraction: CandidateExtraction<'_>,
    interner: &mut TokenInterner,
    candidates: &mut Vec<FunctionCandidate>,
) {
    if should_skip_rust_test_module(node, extraction) {
        return;
    }

    if let Some((name, category, body)) = extract_function_parts(node, extraction) {
        let tokens = normalize_tokens(body, extraction.source.as_bytes(), interner);
        if tokens.len() >= extraction.min_tokens {
            let token_counts = token_counts(&tokens);
            candidates.push(FunctionCandidate {
                family: extraction.family,
                category,
                name,
                path: extraction.file.display_path.clone(),
                line: node.start_position().row + 1,
                tokens,
                token_counts,
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_named_functions(child, extraction, interner, candidates);
    }
}

fn extract_function_parts<'tree>(
    node: Node<'tree>,
    extraction: CandidateExtraction<'_>,
) -> Option<(String, FunctionCategory, Node<'tree>)> {
    let kind = node.kind();
    let source = extraction.source;

    match extraction.family {
        LanguageFamily::Rust if kind == FUNCTION_ITEM => {
            let name = node
                .child_by_field_name(NAME_FIELD)?
                .utf8_text(source.as_bytes())
                .ok()?;
            let body = node.child_by_field_name(BODY_FIELD)?;
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
                FUNCTION_DECLARATION | GENERATOR_FUNCTION_DECLARATION | METHOD_DEFINITION
            ) =>
        {
            let name = node
                .child_by_field_name(NAME_FIELD)?
                .utf8_text(source.as_bytes())
                .ok()?;
            let body = node.child_by_field_name(BODY_FIELD)?;
            let category = if kind == METHOD_DEFINITION {
                FunctionCategory::Method
            } else {
                FunctionCategory::Function
            };
            Some((name.to_string(), category, body))
        }
        LanguageFamily::Python if kind == FUNCTION_DEFINITION => {
            let name = node
                .child_by_field_name(NAME_FIELD)?
                .utf8_text(source.as_bytes())
                .ok()?;
            let body = node.child_by_field_name(BODY_FIELD)?;
            Some((name.to_string(), FunctionCategory::Function, body))
        }
        LanguageFamily::Go if matches!(kind, FUNCTION_DECLARATION | METHOD_DECLARATION) => {
            let name = node
                .child_by_field_name(NAME_FIELD)?
                .utf8_text(source.as_bytes())
                .ok()?;
            let body = node.child_by_field_name(BODY_FIELD)?;
            let category = if kind == METHOD_DECLARATION {
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

fn should_skip_rust_test_module(node: Node<'_>, extraction: CandidateExtraction<'_>) -> bool {
    extraction.family == LanguageFamily::Rust
        && !extraction.include_test_similarity
        && node.kind() == "mod_item"
        && has_cfg_test_attribute(node, extraction.source)
}

fn has_cfg_test_attribute(node: Node<'_>, source: &str) -> bool {
    let mut end = node.start_byte().min(source.len());
    let bytes = source.as_bytes();

    loop {
        while end > 0 && bytes[end - 1].is_ascii_whitespace() {
            end -= 1;
        }

        if end == 0 || bytes[end - 1] != b']' {
            return false;
        }

        let Some(start) = source[..end].rfind("#[") else {
            return false;
        };
        let attribute = &source[start..end];
        if is_cfg_test_attribute(attribute) {
            return true;
        }

        end = start;
    }
}

fn is_cfg_test_attribute(attribute: &str) -> bool {
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
        } else if let Ok(text) = node.utf8_text(source)
            && !text.trim().is_empty()
        {
            tokens.push(interner.intern(text));
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
    progress: &mut dyn SimilarFunctionProgress,
) -> Vec<Finding> {
    let buckets = candidate_buckets(candidates);
    let mut comparison_progress = ComparisonProgress::new(progress, total_comparisons(&buckets));
    let mut findings = Vec::new();

    for indexes in buckets.values() {
        findings.extend(group_candidate_bucket(
            candidates,
            indexes,
            options,
            &mut comparison_progress,
        ));
    }

    findings
}

fn candidate_buckets(
    candidates: &[FunctionCandidate],
) -> BTreeMap<(LanguageFamily, FunctionCategory), Vec<usize>> {
    let mut buckets: BTreeMap<(LanguageFamily, FunctionCategory), Vec<usize>> = BTreeMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        buckets
            .entry((candidate.family, candidate.category))
            .or_default()
            .push(index);
    }
    buckets
}

fn total_comparisons(buckets: &BTreeMap<(LanguageFamily, FunctionCategory), Vec<usize>>) -> usize {
    buckets.values().map(|indexes| pair_count(indexes)).sum()
}

fn pair_count(indexes: &[usize]) -> usize {
    indexes
        .len()
        .saturating_mul(indexes.len().saturating_sub(1))
        / 2
}

fn group_candidate_bucket(
    candidates: &[FunctionCandidate],
    indexes: &[usize],
    options: &SimilarFunctionOptions,
    progress: &mut ComparisonProgress<'_>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut used = vec![false; candidates.len()];
    let bucket = CandidateBucket {
        candidates,
        indexes,
        threshold: options.threshold,
    };

    for (representative_position, &representative_index) in indexes.iter().enumerate() {
        if used[representative_index] {
            progress.advance_by(indexes.len().saturating_sub(representative_position + 1));
            continue;
        }

        let group = collect_similar_group(
            &bucket,
            &used,
            representative_position,
            representative_index,
            progress,
        );

        if group.len() >= options.min_group_size {
            for &index in &group {
                used[index] = true;
            }
            findings.push(similar_function_finding(
                candidates,
                &group,
                options.min_group_size,
                options.threshold,
            ));
        }
    }

    findings
}

struct CandidateBucket<'a> {
    candidates: &'a [FunctionCandidate],
    indexes: &'a [usize],
    threshold: f64,
}

fn collect_similar_group(
    bucket: &CandidateBucket<'_>,
    used: &[bool],
    representative_position: usize,
    representative_index: usize,
    progress: &mut ComparisonProgress<'_>,
) -> Vec<usize> {
    let representative = &bucket.candidates[representative_index];
    let mut group = vec![representative_index];

    for &candidate_index in bucket.indexes.iter().skip(representative_position + 1) {
        if !used[candidate_index]
            && candidates_are_similar(
                representative,
                &bucket.candidates[candidate_index],
                bucket.threshold,
            )
        {
            group.push(candidate_index);
        }
        progress.advance();
    }

    group
}

fn candidates_are_similar(
    representative: &FunctionCandidate,
    candidate: &FunctionCandidate,
    threshold: f64,
) -> bool {
    length_ratio(representative.tokens.len(), candidate.tokens.len()) >= threshold
        && multiset_dice_upper_bound(representative, candidate) >= threshold
        && token_similarity_reaches(&representative.tokens, &candidate.tokens, threshold)
}

struct ComparisonProgress<'a> {
    progress: &'a mut dyn SimilarFunctionProgress,
    total: usize,
    completed: usize,
    last_reported_percent: Option<usize>,
}

impl<'a> ComparisonProgress<'a> {
    fn new(progress: &'a mut dyn SimilarFunctionProgress, total: usize) -> Self {
        Self {
            progress,
            total,
            completed: 0,
            last_reported_percent: None,
        }
    }

    fn advance(&mut self) {
        self.advance_by(1);
    }

    fn advance_by(&mut self, amount: usize) {
        if amount == 0 || self.total == 0 {
            return;
        }

        self.completed += amount;
        let percent = self.completed.saturating_mul(100) / self.total;
        if self.last_reported_percent != Some(percent) || self.completed == self.total {
            self.progress
                .report_compare_progress(self.completed, self.total);
            self.last_reported_percent = Some(percent);
        }
    }
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
    min_group_size: usize,
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
        severity: severity_for_threshold(group.len(), min_group_size),
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
#[path = "similar_functions_tests.rs"]
mod tests;
