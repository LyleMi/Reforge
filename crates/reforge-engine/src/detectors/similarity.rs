use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use tree_sitter::{Node, Parser, Tree};

use crate::evidence_analysis::DetectedEvidenceInput;
use crate::lang::{
    BODY_FIELD, FUNCTION_DECLARATION, FUNCTION_DEFINITION, FUNCTION_ITEM,
    GENERATOR_FUNCTION_DECLARATION, LanguageFamily, METHOD_DECLARATION, METHOD_DEFINITION,
    NAME_FIELD, adapter_for_source, child_by_kind, has_rust_cfg_test_attribute,
    is_identifier_like_kind,
};
use crate::model::{DetectedEvidence, DetectedMeasurement, RelatedLocation, Rule};

mod comparison;
mod index;

use comparison::{LcsWorkspace, length_ratio, multiset_dice_upper_bound, token_similarity_reaches};
use index::CandidateIndex;
pub use index::SimilarityComparisonStats;

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
    pub detections: Vec<DetectedEvidence>,
    pub candidate_count: usize,
    pub comparison_stats: SimilarityComparisonStats,
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
) -> Result<Vec<DetectedEvidence>> {
    Ok(scan_similar_functions_report(files, options)?.detections)
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
            detections: Vec::new(),
            candidate_count: 0,
            comparison_stats: SimilarityComparisonStats::default(),
        });
    }

    let mut candidates = Vec::new();
    let mut interner = TokenInterner::default();
    for (index, file) in files.iter().enumerate() {
        if options.include_test_similarity || !crate::scan::is_test_source(&file.file.path) {
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
    let (detections, comparison_stats) = group_candidates(&candidates, options, progress);
    Ok(SimilarFunctionScan {
        detections,
        candidate_count,
        comparison_stats,
    })
}

pub(crate) fn parse_source_files(files: &[SourceFile]) -> Result<Vec<ParsedSourceFile>> {
    files
        .iter()
        .filter_map(|file| parse_source_file(file.clone()).transpose())
        .collect()
}

pub(crate) fn parse_source_file(file: SourceFile) -> Result<Option<ParsedSourceFile>> {
    let Some(adapter) = adapter_for_source(&file.path, &file.source) else {
        return Ok(None);
    };
    let mut file = file;
    if let Some(source) = crate::lang::vue_script_source(&file.path, &file.source) {
        file.source = Arc::from(source);
    }

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

include!("similarity/extraction.rs");

fn group_candidates(
    candidates: &[FunctionCandidate],
    options: &SimilarFunctionOptions,
    progress: &mut dyn SimilarFunctionProgress,
) -> (Vec<DetectedEvidence>, SimilarityComparisonStats) {
    let buckets = candidate_buckets(candidates);
    let mut candidate_index = CandidateIndex::build(candidates, &buckets, options.threshold);
    let mut comparison_progress = ComparisonProgress::new(progress, total_comparisons(&buckets));
    let mut detections = Vec::new();

    for indexes in buckets.values() {
        detections.extend(group_candidate_bucket(
            candidates,
            indexes,
            options,
            &mut candidate_index,
            &mut comparison_progress,
        ));
    }

    (detections, candidate_index.stats)
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
    candidate_index: &mut CandidateIndex,
    progress: &mut ComparisonProgress<'_>,
) -> Vec<DetectedEvidence> {
    let mut detections = Vec::new();
    let mut used = vec![false; candidates.len()];
    let mut comparison = CandidateComparison {
        index: candidate_index,
        progress,
        workspace: LcsWorkspace::default(),
    };
    let bucket = CandidateBucket {
        candidates,
        indexes,
        threshold: options.threshold,
    };

    for (representative_position, &representative_index) in indexes.iter().enumerate() {
        if used[representative_index] {
            comparison
                .progress
                .advance_by(indexes.len().saturating_sub(representative_position + 1));
            continue;
        }

        let group = collect_similar_group(
            &bucket,
            &used,
            Representative {
                position: representative_position,
                index: representative_index,
            },
            &mut comparison,
        );

        if group.len() >= options.min_group_size {
            for &index in &group {
                used[index] = true;
            }
            detections.push(similar_function_detection(
                candidates,
                &group,
                options.min_group_size,
                options.threshold,
            ));
        }
    }

    detections
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
    comparison: &mut CandidateComparison<'_, '_>,
) -> Vec<usize> {
    let representative_candidate = &bucket.candidates[representative.index];
    let mut group = vec![representative.index];

    for &candidate in bucket.indexes.iter().skip(representative.position + 1) {
        if !used[candidate]
            && comparison.are_similar(
                representative.index,
                candidate,
                representative_candidate,
                &bucket.candidates[candidate],
                bucket.threshold,
            )
        {
            group.push(candidate);
        }
        comparison.progress.advance();
    }

    group
}

struct CandidateComparison<'index, 'progress> {
    index: &'index mut CandidateIndex,
    progress: &'index mut ComparisonProgress<'progress>,
    workspace: LcsWorkspace,
}

impl CandidateComparison<'_, '_> {
    fn are_similar(
        &mut self,
        representative_index: usize,
        candidate_index: usize,
        representative: &FunctionCandidate,
        candidate: &FunctionCandidate,
        threshold: f64,
    ) -> bool {
        if !self.index.contains(representative_index, candidate_index) {
            return false;
        }
        if representative.tokens == candidate.tokens {
            return true;
        }
        self.index.stats.lcs_comparisons += 1;
        token_similarity_reaches(
            &representative.tokens,
            &candidate.tokens,
            threshold,
            &mut self.workspace,
        )
    }
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

fn similar_function_detection(
    candidates: &[FunctionCandidate],
    group: &[usize],
    min_group_size: usize,
    threshold: f64,
) -> DetectedEvidence {
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

    DetectedEvidence::from(
        DetectedEvidenceInput::new(
            Rule::SimilarFunctions,
            representative.path.clone(),
            Some(representative.line),
            format!(
                "{} structurally similar functions/methods found at similarity >= {:.2}",
                group.len(),
                threshold
            ),
            vec![DetectedMeasurement::threshold(
                crate::model::MetricId::GroupSize,
                group.len(),
                min_group_size,
                "functions",
            )],
        )
        .with_related_locations(related_locations),
    )
}

#[cfg(test)]
#[path = "../similar_functions_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../script_similarity_tests.rs"]
mod script_tests;

#[cfg(test)]
mod index_tests;
