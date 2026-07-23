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
    previous: Vec<usize>,
    current: Vec<usize>,
}

impl LcsWorkspace {
    fn prepare(&mut self, width: usize) {
        if self.previous.len() < width {
            self.previous.resize(width, 0);
        } else {
            self.previous[..width].fill(0);
        }

        if self.current.len() < width {
            self.current.resize(width, 0);
        } else {
            self.current[..width].fill(0);
        }
    }
}

pub(super) fn token_similarity_reaches(
    left: &[TokenId],
    right: &[TokenId],
    threshold: f64,
    lcs_workspace: &mut LcsWorkspace,
) -> bool {
    if left.is_empty() && right.is_empty() {
        return true;
    }

    let required_lcs = required_lcs_len(left.len(), right.len(), threshold);
    longest_common_subsequence_reaches(left, right, required_lcs, lcs_workspace)
}

fn required_lcs_len(left_len: usize, right_len: usize, threshold: f64) -> usize {
    ((threshold * (left_len + right_len) as f64) / 2.0).ceil() as usize
}

fn longest_common_subsequence_reaches(
    left: &[TokenId],
    right: &[TokenId],
    required_lcs: usize,
    lcs_workspace: &mut LcsWorkspace,
) -> bool {
    if required_lcs == 0 {
        return true;
    }

    if left.len().min(right.len()) < required_lcs {
        return false;
    }

    let width = right.len() + 1;
    lcs_workspace.prepare(width);
    let mut previous = &mut lcs_workspace.previous;
    let mut current = &mut lcs_workspace.current;

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
