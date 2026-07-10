use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ScalarKind {
    F16,
    F32,
}

pub const HASH_VECTOR_QUANTIZATION: ScalarKind = ScalarKind::F16;
pub const NEURAL_VECTOR_QUANTIZATION: ScalarKind = ScalarKind::F16;

#[derive(Debug, Clone)]
pub struct VectorMatch {
    pub key: u64,
    pub score: f32,
}

#[derive(Debug, Clone, Copy)]
struct RankedVectorMatch {
    key: u64,
    score: f32,
}

impl PartialEq for RankedVectorMatch {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.score.to_bits() == other.score.to_bits()
    }
}

impl Eq for RankedVectorMatch {}

impl PartialOrd for RankedVectorMatch {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RankedVectorMatch {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .total_cmp(&other.score)
            .then_with(|| other.key.cmp(&self.key))
    }
}

fn top_vector_matches(
    matches: impl IntoIterator<Item = VectorMatch>,
    count: usize,
) -> Vec<VectorMatch> {
    if count == 0 {
        return Vec::new();
    }

    let mut top = BinaryHeap::with_capacity(count);
    for vector_match in matches {
        let ranked = RankedVectorMatch {
            key: vector_match.key,
            score: vector_match.score,
        };
        if top.len() < count {
            top.push(Reverse(ranked));
        } else if top.peek().is_some_and(|worst| ranked > worst.0) {
            top.pop();
            top.push(Reverse(ranked));
        }
    }

    let mut matches = top
        .into_iter()
        .map(|Reverse(ranked)| VectorMatch {
            key: ranked.key,
            score: ranked.score,
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.key.cmp(&right.key))
    });
    matches
}

mod optimized;
#[cfg(test)]
#[cfg_attr(test, allow(dead_code))]
mod portable;

pub use optimized::VectorStore;
