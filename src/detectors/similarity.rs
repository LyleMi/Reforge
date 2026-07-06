use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use tree_sitter::{Node, Parser, Tree};

use crate::language::{
    BODY_FIELD, FUNCTION_DECLARATION, FUNCTION_DEFINITION, FUNCTION_ITEM,
    GENERATOR_FUNCTION_DECLARATION, LanguageFamily, METHOD_DECLARATION, METHOD_DEFINITION,
    NAME_FIELD, adapter_for_path, is_identifier_like_kind,
};
use crate::scanner::{
    Finding, FindingInput, FindingKind, FindingMetric, RelatedLocation, scored_finding,
};

mod comparison;

use comparison::{LcsWorkspace, length_ratio, multiset_dice_upper_bound, token_similarity_reaches};

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
    pub source: Arc<str>,
}

#[derive(Debug, Clone)]
pub(crate) struct ParsedSourceFile {
    pub file: SourceFile,
    pub family: LanguageFamily,
    pub tree: Tree,
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
    let parsed_files = parse_source_files(files)?;
    scan_parsed_similar_functions_report_with_progress(&parsed_files, options, progress)
}

pub(crate) fn scan_parsed_similar_functions_report_with_progress(
    files: &[ParsedSourceFile],
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
        if options.include_test_similarity || !crate::scanner::is_test_source(&file.file.path) {
            candidates.extend(extract_candidates_from_parsed(
                file,
                options.min_tokens,
                options.include_test_similarity,
                &mut interner,
            ));
        }
        progress.report_extract_progress(index + 1, files.len(), &file.file.display_path);
    }

    let candidate_count = candidates.len();
    Ok(SimilarFunctionScan {
        findings: group_candidates(&candidates, options, progress),
        candidate_count,
    })
}

pub(crate) fn parse_source_files(files: &[SourceFile]) -> Result<Vec<ParsedSourceFile>> {
    files
        .iter()
        .filter_map(|file| parse_source_file(file.clone()).transpose())
        .collect()
}

pub(crate) fn parse_source_file(file: SourceFile) -> Result<Option<ParsedSourceFile>> {
    let Some(adapter) = adapter_for_path(&file.path) else {
        return Ok(None);
    };

    let mut parser = Parser::new();
    parser
        .set_language(&adapter.language())
        .with_context(|| format!("failed to load parser for {}", file.display_path))?;

    let Some(tree) = parser.parse(file.source.as_ref(), None) else {
        return Ok(None);
    };

    if tree.root_node().has_error() {
        return Ok(None);
    }

    Ok(Some(ParsedSourceFile {
        file,
        family: adapter.family,
        tree,
    }))
}

fn validate_options(options: &SimilarFunctionOptions) -> Result<()> {
    if !(0.0..=1.0).contains(&options.threshold) {
        bail!("--function-similarity must be between 0.0 and 1.0");
    }

    Ok(())
}

fn extract_candidates_from_parsed(
    file: &ParsedSourceFile,
    min_tokens: usize,
    include_test_similarity: bool,
    interner: &mut TokenInterner,
) -> Vec<FunctionCandidate> {
    let extraction = CandidateExtraction {
        source: &file.file.source,
        file: &file.file,
        family: file.family,
        min_tokens,
        include_test_similarity,
    };
    let mut candidates = Vec::new();
    collect_named_functions(file.tree.root_node(), extraction, interner, &mut candidates);
    candidates
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

    if is_identifier_like_kind(kind) {
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
    let mut lcs_workspace = LcsWorkspace::default();
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
            Representative {
                position: representative_position,
                index: representative_index,
            },
            progress,
            &mut lcs_workspace,
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

#[derive(Debug, Clone, Copy)]
struct Representative {
    position: usize,
    index: usize,
}

fn collect_similar_group(
    bucket: &CandidateBucket<'_>,
    used: &[bool],
    representative: Representative,
    progress: &mut ComparisonProgress<'_>,
    lcs_workspace: &mut LcsWorkspace,
) -> Vec<usize> {
    let representative_candidate = &bucket.candidates[representative.index];
    let mut group = vec![representative.index];

    for &candidate_index in bucket.indexes.iter().skip(representative.position + 1) {
        if !used[candidate_index]
            && candidates_are_similar(
                representative_candidate,
                &bucket.candidates[candidate_index],
                bucket.threshold,
                lcs_workspace,
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
    lcs_workspace: &mut LcsWorkspace,
) -> bool {
    if representative.tokens == candidate.tokens {
        return true;
    }

    length_ratio(representative.tokens.len(), candidate.tokens.len()) >= threshold
        && multiset_dice_upper_bound(representative, candidate) >= threshold
        && token_similarity_reaches(
            &representative.tokens,
            &candidate.tokens,
            threshold,
            lcs_workspace,
        )
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

    scored_finding(
        FindingInput::new(
            FindingKind::SimilarFunctions,
            representative.path.clone(),
            Some(representative.line),
            format!(
                "{} structurally similar functions/methods found at similarity >= {:.2}",
                group.len(),
                threshold
            ),
            vec![FindingMetric::threshold(
                "group_size",
                group.len(),
                min_group_size,
                "functions",
            )],
        )
        .with_confidence(threshold)
        .with_related_locations(related_locations),
    )
}

#[cfg(test)]
#[path = "../similar_functions_tests.rs"]
mod tests;
