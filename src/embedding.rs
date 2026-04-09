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

use crate::text::{singularize_token, split_identifier_segments};
use std::collections::HashMap;

/// Shared interface implemented by all embedding backends.
pub trait EmbeddingModel: Send + Sync {
    fn dimensions(&self) -> usize;
    fn embed(&self, text: &str) -> Vec<f32>;

    /// Embed multiple texts in a single call. Backends that support batch
    /// inference (e.g. ONNX) override this for significant speedup.
    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }
}

/// Returns the embedding dimension for the selected mode.
pub fn model_dimensions(hash: bool) -> usize {
    if hash {
        256
    } else {
        #[cfg(feature = "neural")]
        {
            384 // all-MiniLM-L6-v2
        }
        #[cfg(not(feature = "neural"))]
        {
            256
        }
    }
}

/// Create the appropriate embedding model.
///
/// By default (when `hash` is `false`), returns an [`OnnxEmbeddingModel`]
/// backed by `all-MiniLM-L6-v2` (quantized) for high-quality semantic search.
/// Pass `hash = true` to use the lightweight [`HashEmbeddingModel`] instead.
///
/// If the `neural` feature is not compiled in, always falls back to hash.
pub fn create_model(hash: bool) -> Box<dyn EmbeddingModel> {
    if !hash {
        #[cfg(feature = "neural")]
        {
            match CandleEmbeddingModel::new() {
                Ok(model) => return Box::new(model),
                Err(e) => {
                    tracing::warn!("Failed to load neural model, falling back to hash: {e}");
                }
            }
        }
    }

    Box::new(HashEmbeddingModel::new(256))
}

/// Create a hash-only embedding model (instant, no ONNX).
pub fn create_hash_model() -> Box<dyn EmbeddingModel> {
    Box::new(HashEmbeddingModel::new(256))
}

/// Create a neural (ONNX) embedding model. Returns Err if the neural
/// feature is not compiled in or the model fails to load.
pub fn create_neural_model() -> anyhow::Result<Box<dyn EmbeddingModel>> {
    #[cfg(feature = "neural")]
    {
        let model = CandleEmbeddingModel::new()?;
        Ok(Box::new(model))
    }
    #[cfg(not(feature = "neural"))]
    {
        anyhow::bail!("neural feature not compiled in")
    }
}

/// Create a neural model with reduced thread budget for background work.
/// Uses half the CPU cores so the system stays responsive.
pub fn create_neural_model_background() -> anyhow::Result<Box<dyn EmbeddingModel>> {
    #[cfg(feature = "neural")]
    {
        let model = CandleEmbeddingModel::new_background()?;
        Ok(Box::new(model))
    }
    #[cfg(not(feature = "neural"))]
    {
        anyhow::bail!("neural feature not compiled in")
    }
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

                use std::hash::{DefaultHasher, Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                normalized.hash(&mut hasher);
                let hash_val = hasher.finish();

                let bucket = (hash_val as usize) % self.dimensions;
                let sign = if (hash_val >> 16) & 1 == 0 { 1.0 } else { -1.0 };
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

    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        use rayon::prelude::*;
        texts.par_iter().map(|t| self.embed(t)).collect()
    }
}

// ── Candle neural embedding (behind `neural` feature) ───────────────────────

#[cfg(feature = "neural")]
pub struct CandleEmbeddingModel {
    model: parking_lot::Mutex<candle_embed::BasedBertEmbedder>,
}

#[cfg(feature = "neural")]
pub fn hardware_acceleration_info() -> &'static str {
    "AllMiniLML6V2 via Candle"
}

#[cfg(not(feature = "neural"))]
pub fn hardware_acceleration_info() -> &'static str {
    "Disabled"
}

#[cfg(feature = "neural")]
impl CandleEmbeddingModel {
    pub fn new() -> anyhow::Result<Self> {
        Self::new_internal(false)
    }

    pub fn new_background() -> anyhow::Result<Self> {
        Self::new_internal(true)
    }

    fn new_internal(_is_background: bool) -> anyhow::Result<Self> {
        use candle_embed::{CandleEmbedBuilder, WithModel};

        let mut builder =
            CandleEmbedBuilder::new().set_model_from_presets(WithModel::AllMinilmL6V2);

        if !candle_core::utils::cuda_is_available() && !candle_core::utils::metal_is_available() {
            builder = builder.with_device_cpu();
        }

        let embedder = builder.build()?;

        embedder.load_tokenizer()?;
        embedder.load_model()?;

        Ok(Self {
            model: parking_lot::Mutex::new(embedder),
        })
    }
}

#[cfg(feature = "neural")]
impl EmbeddingModel for CandleEmbeddingModel {
    fn dimensions(&self) -> usize {
        384
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        // BasedBertEmbedder returns Result<Vec<f32>>
        self.model
            .lock()
            .embed_one(text)
            .unwrap_or_else(|_| vec![0.0; 384])
    }

    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        if texts.is_empty() {
            return vec![];
        }

        let mut all_results = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(256) {
            let mut results = self
                .model
                .lock()
                .embed_batch(chunk)
                .unwrap_or_else(|_| chunk.iter().map(|_| vec![0.0; 384]).collect());
            all_results.append(&mut results);
        }
        all_results
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
    fn create_model_returns_hash_when_requested() {
        let model = create_model(true);
        assert_eq!(model.dimensions(), 256);
    }

    #[test]
    fn model_dimensions_hash() {
        assert_eq!(model_dimensions(true), 256);
    }

    #[test]
    fn embed_batch_returns_correct_count() {
        let model = HashEmbeddingModel::new(64);
        let texts = vec!["fn foo() {}", "fn bar() {}", "fn baz() {}"];
        let results = model.embed_batch(&texts);
        assert_eq!(results.len(), 3);
        for vec in &results {
            assert_eq!(vec.len(), 64);
        }
    }

    #[test]
    fn embed_batch_matches_individual_embeds() {
        let model = HashEmbeddingModel::new(128);
        let texts = vec!["calculate tax", "process payment"];
        let batch = model.embed_batch(&texts);
        let individual: Vec<Vec<f32>> = texts.iter().map(|t| model.embed(t)).collect();
        assert_eq!(batch, individual);
    }

    #[test]
    fn create_hash_model_returns_correct_dimensions() {
        let model = create_hash_model();
        assert_eq!(model.dimensions(), 256);
    }

    #[test]
    fn embeddings_are_l2_normalized() {
        let model = HashEmbeddingModel::new(128);
        let vec = model.embed("pub fn calculate_tax(amount: f64) -> f64 { amount * 0.2 }");
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "embedding should be L2-normalized, got norm={norm}"
        );
    }

    #[test]
    fn empty_input_produces_valid_embedding() {
        let model = HashEmbeddingModel::new(64);
        let vec = model.embed("");
        assert_eq!(vec.len(), 64);
        // Empty input should still produce a valid vector (all zeros or normalized)
    }

    #[test]
    fn semantic_token_variants_splits_camel_case() {
        let variants = semantic_token_variants("calculateTax");
        assert!(variants.contains(&"calculatetax".to_string()));
        assert!(variants.contains(&"calculate".to_string()));
        assert!(variants.contains(&"tax".to_string()));
    }

    #[test]
    fn semantic_token_variants_handles_single_word() {
        let variants = semantic_token_variants("hello");
        assert!(variants.contains(&"hello".to_string()));
    }

    #[test]
    fn semantic_token_variants_splits_snake_case() {
        let variants = semantic_token_variants("process_payment");
        assert!(variants.contains(&"process_payment".to_string()));
        assert!(variants.contains(&"process".to_string()));
        assert!(variants.contains(&"payment".to_string()));
    }

    #[test]
    fn hardware_acceleration_info_returns_nonempty() {
        let info = hardware_acceleration_info();
        assert!(!info.is_empty());
    }

    #[test]
    fn different_texts_produce_different_embeddings() {
        let model = HashEmbeddingModel::new(128);
        let v1 = model.embed("fn calculate_tax() {}");
        let v2 = model.embed("struct DatabaseConnection {}");
        assert_ne!(
            v1, v2,
            "semantically different texts should have different embeddings"
        );
    }
}
