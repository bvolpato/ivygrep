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

#[cfg(not(target_os = "windows"))]
mod optimized;
#[cfg(any(target_os = "windows", test))]
#[cfg_attr(test, allow(dead_code))]
mod portable;

#[cfg(not(target_os = "windows"))]
pub use optimized::VectorStore;
#[cfg(target_os = "windows")]
pub use portable::VectorStore;
