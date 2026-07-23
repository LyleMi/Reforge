use std::collections::BTreeSet;

use super::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SimilarityComparisonStats {
    pub total_candidate_pairs: usize,
    pub indexed_candidate_pairs: usize,
    pub multiset_pruned_pairs: usize,
    pub lcs_comparisons: usize,
}

pub(super) struct CandidateIndex {
    eligible_pairs: BTreeSet<(usize, usize)>,
    pub stats: SimilarityComparisonStats,
}

impl CandidateIndex {
    pub(super) fn build(
        candidates: &[FunctionCandidate],
        buckets: &BTreeMap<(LanguageFamily, FunctionCategory), Vec<usize>>,
        threshold: f64,
    ) -> Self {
        let mut index = Self {
            eligible_pairs: BTreeSet::new(),
            stats: SimilarityComparisonStats {
                total_candidate_pairs: candidates
                    .len()
                    .saturating_mul(candidates.len().saturating_sub(1))
                    / 2,
                ..SimilarityComparisonStats::default()
            },
        };
        for indexes in buckets.values() {
            index.index_bucket(candidates, indexes, threshold);
        }
        index.stats.indexed_candidate_pairs = index.eligible_pairs.len();
        index
    }

    pub(super) fn contains(&self, left: usize, right: usize) -> bool {
        self.eligible_pairs.contains(&ordered_pair(left, right))
    }

    fn index_bucket(
        &mut self,
        candidates: &[FunctionCandidate],
        indexes: &[usize],
        threshold: f64,
    ) {
        let mut by_length = indexes.to_vec();
        by_length.sort_by_key(|candidate| candidates[*candidate].tokens.len());
        for left_position in 0..by_length.len() {
            self.index_candidates_for_left(candidates, &by_length, left_position, threshold);
        }
    }

    fn index_candidates_for_left(
        &mut self,
        candidates: &[FunctionCandidate],
        by_length: &[usize],
        left_position: usize,
        threshold: f64,
    ) {
        let left_index = by_length[left_position];
        let left = &candidates[left_index];
        for &right_index in &by_length[left_position + 1..] {
            let right = &candidates[right_index];
            if threshold > 0.0 && right.tokens.len() as f64 > left.tokens.len() as f64 / threshold {
                break;
            }
            if length_ratio(left.tokens.len(), right.tokens.len()) < threshold {
                continue;
            }
            if multiset_dice_upper_bound(left, right) < threshold {
                self.stats.multiset_pruned_pairs += 1;
                continue;
            }
            self.eligible_pairs
                .insert(ordered_pair(left_index, right_index));
        }
    }
}

fn ordered_pair(left: usize, right: usize) -> (usize, usize) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}
