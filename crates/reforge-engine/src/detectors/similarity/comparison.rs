use super::*;

pub(super) fn length_ratio(left_len: usize, right_len: usize) -> f64 {
    let shorter = left_len.min(right_len) as f64;
    let longer = left_len.max(right_len) as f64;

    if longer == 0.0 { 1.0 } else { shorter / longer }
}

pub(super) fn multiset_dice_upper_bound(
    left: &FunctionCandidate,
    right: &FunctionCandidate,
) -> f64 {
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

#[derive(Debug, Default)]
pub(super) struct LcsWorkspace {
    state: Vec<u64>,
    // Sparse word masks keep storage linear even when every token is unique.
    masks: HashMap<TokenId, Vec<(usize, u64)>>,
}

impl LcsWorkspace {
    fn prepare(&mut self, tokens: &[TokenId]) {
        self.state.resize(tokens.len().div_ceil(64), 0);
        self.state.fill(0);
        self.masks.clear();
        for (position, &token) in tokens.iter().enumerate() {
            let word = position / 64;
            let bit = 1u64 << (position % 64);
            let mask = self.masks.entry(token).or_default();
            if let Some((_, bits)) = mask.last_mut().filter(|(last_word, _)| *last_word == word) {
                *bits |= bit;
            } else {
                mask.push((word, bit));
            }
        }
    }

    fn advance(&mut self, token: TokenId) -> usize {
        let masks = self
            .masks
            .get(&token)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut matches = masks.iter().peekable();
        let mut carry = 1;
        let mut borrow = false;
        let mut length = 0;
        for (word, state) in self.state.iter_mut().enumerate() {
            let mask = if matches.peek().is_some_and(|(index, _)| *index == word) {
                matches.next().map(|(_, mask)| *mask).unwrap_or_default()
            } else {
                0
            };
            let x = *state | mask;
            let y = (*state << 1) | carry;
            carry = *state >> 63;
            let (difference, first_borrow) = x.overflowing_sub(y);
            let (difference, second_borrow) = difference.overflowing_sub(u64::from(borrow));
            borrow = first_borrow || second_borrow;
            *state = x & !difference;
            length += state.count_ones() as usize;
        }
        length
    }
}

pub(super) fn token_similarity_reaches(
    left: &[TokenId],
    right: &[TokenId],
    threshold: f64,
    workspace: &mut LcsWorkspace,
) -> bool {
    let required = ((threshold * (left.len() + right.len()) as f64) / 2.0).ceil() as usize;
    if required == 0 {
        return true;
    }
    if left.len().min(right.len()) < required {
        return false;
    }
    let (rows, columns) = if left.len() >= right.len() {
        (left, right)
    } else {
        (right, left)
    };
    workspace.prepare(columns);
    for (index, &token) in rows.iter().enumerate() {
        let length = workspace.advance(token);
        if length >= required {
            return true;
        }
        if length + rows.len() - index - 1 < required {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar_lcs(left: &[TokenId], right: &[TokenId]) -> usize {
        let mut previous = vec![0; right.len() + 1];
        for a in left {
            let mut current = vec![0; right.len() + 1];
            for (j, b) in right.iter().enumerate() {
                current[j + 1] = if a == b {
                    previous[j] + 1
                } else {
                    previous[j + 1].max(current[j])
                };
            }
            previous = current;
        }
        previous[right.len()]
    }

    #[test]
    fn bit_parallel_lcs_matches_scalar_oracle_across_word_boundaries() {
        let mut random = 17u64;
        let mut sequence = |length: usize, alphabet: u32| {
            (0..length)
                .map(|_| {
                    random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                    ((random >> 32) as u32) % alphabet
                })
                .collect::<Vec<_>>()
        };
        let mut workspace = LcsWorkspace::default();
        for length in [0, 1, 2, 31, 63, 64, 65, 127, 128, 129, 257] {
            for other in [0, 1, 63, 64, 65, 129] {
                for alphabet in [1, 3, 1000] {
                    let left = sequence(length, alphabet);
                    let right = sequence(other, alphabet);
                    let exact = scalar_lcs(&left, &right);
                    for threshold in [0.0, 0.5, 0.85, 1.0] {
                        let required =
                            ((threshold * (length + other) as f64) / 2.0).ceil() as usize;
                        assert_eq!(
                            token_similarity_reaches(&left, &right, threshold, &mut workspace),
                            exact >= required,
                            "lengths {length}/{other}, alphabet {alphabet}, threshold {threshold}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn unique_tokens_use_linear_sparse_mask_storage() {
        let mut workspace = LcsWorkspace::default();
        let tokens = (0..100_000).collect::<Vec<_>>();
        workspace.prepare(&tokens);
        assert_eq!(
            workspace.masks.values().map(Vec::len).sum::<usize>(),
            tokens.len()
        );
        assert_eq!(workspace.state.len(), tokens.len().div_ceil(64));
    }
}
