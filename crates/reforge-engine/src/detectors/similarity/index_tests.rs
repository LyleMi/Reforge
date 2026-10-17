use super::*;

fn candidate(tokens: Vec<TokenId>) -> FunctionCandidate {
    FunctionCandidate {
        family: LanguageFamily::Rust,
        category: FunctionCategory::Function,
        name: "f".into(),
        path: "src/lib.rs".into(),
        line: 1,
        token_counts: token_counts(&tokens),
        tokens,
    }
}

fn sequences(length: usize, alphabet: TokenId) -> Vec<Vec<TokenId>> {
    let count = usize::pow(alphabet as usize, length as u32);
    (0..count)
        .map(|mut value| {
            let mut tokens = vec![0; length];
            for token in &mut tokens {
                *token = (value % alphabet as usize) as TokenId;
                value /= alphabet as usize;
            }
            tokens
        })
        .collect()
}

#[test]
fn lossless_index_matches_exhaustive_pair_oracle() {
    let candidates = (1..=4)
        .flat_map(|length| sequences(length, 3))
        .map(candidate)
        .collect::<Vec<_>>();

    for threshold in [0.0, 0.5, 0.75, 0.85, 1.0] {
        let mut index = CandidateIndex::new(&candidates, threshold);
        for left in 0..candidates.len() {
            for right in left + 1..candidates.len() {
                let mut workspace = LcsWorkspace::default();
                let exact = length_ratio(
                    candidates[left].tokens.len(),
                    candidates[right].tokens.len(),
                ) >= threshold
                    && multiset_dice_upper_bound(&candidates[left], &candidates[right])
                        >= threshold
                    && token_similarity_reaches(
                        &candidates[left].tokens,
                        &candidates[right].tokens,
                        threshold,
                        &mut workspace,
                    );
                assert!(
                    !exact || index.contains(left, right),
                    "index dropped an exact match for {left}/{right} at {threshold}"
                );
            }
        }
    }
}

#[test]
fn disjoint_large_fixture_avoids_lcs_work() {
    let candidates = (0..120)
        .map(|index| {
            candidate(
                (0..100)
                    .map(|offset| (index * 100 + offset) as TokenId)
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let options = SimilarFunctionOptions {
        min_group_size: 3,
        min_tokens: 1,
        threshold: 0.85,
        include_test_similarity: true,
    };
    let mut progress = NoopSimilarityProgress;
    let (detections, stats) = group_candidates(&candidates, &options, &mut progress);

    assert!(detections.is_empty());
    assert_eq!(stats.total_candidate_pairs, 7_140);
    assert_eq!(stats.indexed_candidate_pairs, 0);
    assert_eq!(stats.multiset_pruned_pairs, 7_140);
    assert_eq!(stats.lcs_comparisons, 0);
}

#[test]
fn repeated_scaffolds_do_not_materialize_quadratic_candidate_pairs() {
    let candidates = (0..10_000)
        .map(|_| candidate(vec![1, 2, 3, 4]))
        .collect::<Vec<_>>();
    let options = SimilarFunctionOptions {
        min_group_size: 3,
        min_tokens: 1,
        threshold: 0.85,
        include_test_similarity: true,
    };
    let (detections, stats) = group_candidates(&candidates, &options, &mut NoopSimilarityProgress);
    assert_eq!(detections.len(), 1);
    assert_eq!(stats.total_candidate_pairs, 49_995_000);
    assert_eq!(stats.indexed_candidate_pairs, 9_999);
    assert_eq!(stats.lcs_comparisons, 0);
}
