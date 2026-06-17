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

mod optimized;
#[cfg(test)]
#[cfg_attr(test, allow(dead_code))]
mod portable;

pub use optimized::VectorStore;
