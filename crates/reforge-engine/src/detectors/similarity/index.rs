use super::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SimilarityComparisonStats {
    pub total_candidate_pairs: usize,
    /// Pairs passing the lossless filters when actually visited by grouping.
    pub indexed_candidate_pairs: usize,
    pub multiset_pruned_pairs: usize,
    pub lcs_comparisons: usize,
}

/// Apply lossless candidate filters on demand. Keeping every eligible pair in
/// memory makes repeated source scaffolds quadratic in memory before any
/// grouping can discard already-used candidates.
pub(super) struct CandidateIndex<'a> {
    candidates: &'a [FunctionCandidate],
    threshold: f64,
    pub stats: SimilarityComparisonStats,
}

impl<'a> CandidateIndex<'a> {
    pub(super) fn new(candidates: &'a [FunctionCandidate], threshold: f64) -> Self {
        Self {
            candidates,
            threshold,
            stats: SimilarityComparisonStats {
                total_candidate_pairs: candidates
                    .len()
                    .saturating_mul(candidates.len().saturating_sub(1))
                    / 2,
                ..SimilarityComparisonStats::default()
            },
        }
    }

    pub(super) fn contains(&mut self, left: usize, right: usize) -> bool {
        let left = &self.candidates[left];
        let right = &self.candidates[right];
        if left.family != right.family
            || left.category != right.category
            || length_ratio(left.tokens.len(), right.tokens.len()) < self.threshold
        {
            return false;
        }
        if multiset_dice_upper_bound(left, right) < self.threshold {
            self.stats.multiset_pruned_pairs += 1;
            return false;
        }
        self.stats.indexed_candidate_pairs += 1;
        true
    }
}
