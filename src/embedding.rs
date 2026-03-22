//! Embedding models for semantic vector search.
//!
//! Two implementations are available:
//!
//! | Model | Feature | Dimensions | Quality | Binary size |
//! |-------|---------|------------|---------|-------------|
//! | [`HashEmbeddingModel`] | *(always)* | 256 | Moderate — token overlap heuristic | Tiny |
//! | [`OnnxEmbeddingModel`] | `neural` | 384 | High — true semantic similarity | ~23 MB model download |
//!
//! Use [`create_model`] to build the right model based on the `neural` flag.

use std::collections::HashMap;

use pluralizer::pluralize;
use sha2::{Digest, Sha256};

/// Shared interface implemented by all embedding backends.
pub trait EmbeddingModel: Send + Sync {
    fn dimensions(&self) -> usize;
    fn embed(&self, text: &str) -> Vec<f32>;
}

/// Returns the embedding dimension for the selected mode.
pub fn model_dimensions(neural: bool) -> usize {
    if neural {
        #[cfg(feature = "neural")]
        {
            384 // all-MiniLM-L6-v2
        }
        #[cfg(not(feature = "neural"))]
        {
            let _ = neural;
            256
        }
    } else {
        256
    }
}

/// Create the appropriate embedding model.
///
/// When `neural` is `true` **and** the `neural` feature is compiled in,
/// returns an [`OnnxEmbeddingModel`] backed by `all-MiniLM-L6-v2`.
/// Otherwise falls back to the zero-dependency [`HashEmbeddingModel`].
pub fn create_model(neural: bool) -> Box<dyn EmbeddingModel> {
    #[cfg(feature = "neural")]
    if neural {
        match OnnxEmbeddingModel::new() {
            Ok(model) => return Box::new(model),
            Err(e) => {
                tracing::warn!("Failed to load neural model, falling back to hash: {e}");
            }
        }
    }

    let _ = neural;
    Box::new(HashEmbeddingModel::new(256))
}

// ── Hash-based embedding (always available) ────────────────────────────────

#[derive(Debug, Clone)]
pub struct HashEmbeddingModel {
    dimensions: usize,
    normalization_aliases: HashMap<&'static str, &'static str>,
}

impl HashEmbeddingModel {
    pub fn new(dimensions: usize) -> Self {
        let normalization_aliases = HashMap::from([
            ("calc", "calculate"),
            ("taxes", "tax"),
            ("compute", "calculate"),
            ("sum", "total"),
            ("klass", "class"),
            ("func", "function"),
        ]);

        Self {
            dimensions,
            normalization_aliases,
        }
    }

    fn normalize_token<'a>(&'a self, token: &'a str) -> &'a str {
        self.normalization_aliases
            .get(token)
            .copied()
            .unwrap_or(token)
    }
}

impl EmbeddingModel for HashEmbeddingModel {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0f32; self.dimensions];
        if text.is_empty() {
            return vector;
        }

        let mut token_count = 0usize;

        for raw_token in text
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|t| !t.is_empty())
        {
            for token in semantic_token_variants(raw_token) {
                let singular = singularize_token(&token);
                let normalized = self.normalize_token(&singular);
                token_count += 1;

                let mut hasher = Sha256::new();
                hasher.update(normalized.as_bytes());
                let hash = hasher.finalize();

                let bucket = u64::from_le_bytes(hash[..8].try_into().expect("slice length"))
                    as usize
                    % self.dimensions;
                let sign = if hash[8] & 1 == 0 { 1.0 } else { -1.0 };
                vector[bucket] += sign;
            }
        }

        if token_count == 0 {
            return vector;
        }

        let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for value in &mut vector {
                *value /= norm;
            }
        }

        vector
    }
}

// ── ONNX neural embedding (behind `neural` feature) ───────────────────────

#[cfg(feature = "neural")]
pub struct OnnxEmbeddingModel {
    model: parking_lot::Mutex<fastembed::TextEmbedding>,
}

#[cfg(feature = "neural")]
impl OnnxEmbeddingModel {
    /// Initialize the neural model.  On first run this downloads
    /// `all-MiniLM-L6-v2` (~23 MB) to a local cache.
    pub fn new() -> anyhow::Result<Self> {
        use fastembed::{EmbeddingModel as FastModel, InitOptions};

        let model = fastembed::TextEmbedding::try_new(
            InitOptions::new(FastModel::AllMiniLML6V2).with_show_download_progress(true),
        )?;

        Ok(Self {
            model: parking_lot::Mutex::new(model),
        })
    }
}

#[cfg(feature = "neural")]
impl EmbeddingModel for OnnxEmbeddingModel {
    fn dimensions(&self) -> usize {
        384
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        self.model
            .lock()
            .embed(vec![text], None)
            .ok()
            .and_then(|mut vecs| vecs.pop())
            .unwrap_or_else(|| vec![0.0; 384])
    }
}

// ── Token helpers ──────────────────────────────────────────────────────────

fn semantic_token_variants(raw_token: &str) -> Vec<String> {
    let compact = raw_token.to_ascii_lowercase();
    let segments = split_identifier_segments(raw_token);

    let mut out = Vec::with_capacity(segments.len().saturating_add(2));
    out.push(compact.clone());

    for segment in &segments {
        if segment != &compact {
            out.push(segment.clone());
        }
    }

    if segments.len() > 1 {
        let joined = segments.join("");
        if joined != compact {
            out.push(joined);
        }
    }

    out.sort();
    out.dedup();
    out
}

fn split_identifier_segments(token: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut prev_is_lower = false;
    let mut prev_is_alpha = false;

    for ch in token.chars() {
        if !ch.is_ascii_alphanumeric() {
            if !current.is_empty() {
                segments.push(current.to_ascii_lowercase());
                current.clear();
            }
            prev_is_lower = false;
            prev_is_alpha = false;
            continue;
        }

        let is_upper = ch.is_ascii_uppercase();
        let is_alpha = ch.is_ascii_alphabetic();

        if !current.is_empty() && is_upper && prev_is_lower {
            segments.push(current.to_ascii_lowercase());
            current.clear();
        }

        if !current.is_empty() && is_alpha != prev_is_alpha {
            segments.push(current.to_ascii_lowercase());
            current.clear();
        }

        current.push(ch);
        prev_is_lower = ch.is_ascii_lowercase();
        prev_is_alpha = is_alpha;
    }

    if !current.is_empty() {
        segments.push(current.to_ascii_lowercase());
    }

    segments
}

fn singularize_token(token: &str) -> String {
    if token.len() <= 3 || !token.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return token.to_string();
    }

    let singular = pluralize(token, 1isize, false).to_ascii_lowercase();
    if singular.is_empty() {
        token.to_string()
    } else {
        singular
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_embeddings_are_stable() {
        let model = HashEmbeddingModel::new(64);
        let left = model.embed("calculate tax total");
        let right = model.embed("calculate tax total");
        assert_eq!(left, right);
    }

    #[test]
    fn alias_mapping_changes_similarity() {
        let model = HashEmbeddingModel::new(64);
        let v1 = model.embed("calc tax");
        let v2 = model.embed("calculate taxes");
        let cosine = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum::<f32>();
        assert!(cosine > 0.2);
    }

    #[test]
    fn identifier_and_plural_forms_align() {
        let model = HashEmbeddingModel::new(128);
        let query = model.embed("apply limits");
        let code = model.embed("void applyLimit() {}");
        let cosine = query
            .iter()
            .zip(code.iter())
            .map(|(a, b)| a * b)
            .sum::<f32>();
        assert!(cosine > 0.15);
    }

    #[test]
    fn create_model_returns_hash_without_neural() {
        let model = create_model(false);
        assert_eq!(model.dimensions(), 256);
    }

    #[test]
    fn model_dimensions_hash() {
        assert_eq!(model_dimensions(false), 256);
    }
}
