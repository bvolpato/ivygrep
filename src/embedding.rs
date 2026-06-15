//! Embedding models for semantic vector search.
//!
//! Two implementations are available:
//!
//! | Model | Feature | Dimensions | Quality | Binary size |
//! |-------|---------|------------|---------|-------------|
//! | [`HashEmbeddingModel`] | *(always)* | 256 | Moderate — token overlap heuristic | Tiny |
//! | [`CandleEmbeddingModel`] | `neural` | 384 | High — true semantic similarity | Model download on first use |
//!
//! Use [`create_model`] to build the right model based on the `neural` flag.

use crate::text::{singularize_token, split_identifier_segments};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum NeuralProfile {
    General,
    Code,
}

impl NeuralProfile {
    pub fn configured() -> Self {
        match std::env::var("IVYGREP_MODEL_PROFILE")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "code" | "codesearchnet" | "code-minilm-l6-v1" => Self::Code,
            _ => Self::General,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Code => "code-minilm-l6-v1",
        }
    }

    pub fn dimensions(self) -> usize {
        384
    }

    #[cfg(feature = "neural")]
    fn model_id(self) -> &'static str {
        match self {
            Self::General => "sentence-transformers/all-MiniLM-L6-v2",
            Self::Code => "isuruwijesiri/all-MiniLM-L6-v2-code-search-512",
        }
    }

    #[cfg(feature = "neural")]
    fn model_revision(self) -> &'static str {
        match self {
            Self::General => "1110a243fdf4706b3f48f1d95db1a4f5529b4d41",
            Self::Code => "13b266a617039c16d924b49a56ae978dbd8727ff",
        }
    }
}

pub fn configured_neural_profile_name() -> &'static str {
    NeuralProfile::configured().name()
}

/// Shared interface implemented by all embedding backends.
pub trait EmbeddingModel: Send + Sync {
    fn dimensions(&self) -> usize;
    fn embed(&self, text: &str) -> Vec<f32>;

    /// Embed multiple texts in a single call. Backends that support efficient
    /// parallel inference override this for significant speedup.
    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// Human-readable backend used to create persisted neural vectors.
    fn backend_info(&self) -> Option<&'static str> {
        None
    }

    fn profile_info(&self) -> Option<&'static str> {
        None
    }

    /// Whether background embedding should pause under load, thermal, or
    /// battery pressure. Lightweight and test models stay deterministic.
    fn respects_system_constraints(&self) -> bool {
        false
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
/// By default (when `hash` is `false`), returns a [`CandleEmbeddingModel`]
/// backed by `all-MiniLM-L6-v2` for high-quality semantic search.
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

/// Create a hash-only embedding model (instant, no model download).
pub fn create_hash_model() -> Box<dyn EmbeddingModel> {
    Box::new(HashEmbeddingModel::new(256))
}

/// Create a neural Candle embedding model. Returns Err if the neural
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
        use std::sync::OnceLock;
        // Cache the bounded thread pool so it's built once, not per batch.
        static HASH_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
        let pool = HASH_POOL.get_or_init(|| {
            let n_threads = (num_cpus::get() / 2).max(1);
            rayon::ThreadPoolBuilder::new()
                .num_threads(n_threads)
                .build()
                .expect("failed to build hash embed thread pool")
        });
        pool.install(|| texts.par_iter().map(|t| self.embed(t)).collect())
    }
}

// ── Candle neural embedding (behind `neural` feature) ───────────────────────

#[cfg(feature = "neural")]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum NeuralBackend {
    Metal,
    Cuda,
    AccelerateCpu,
    Cpu,
}

#[cfg(feature = "neural")]
impl NeuralBackend {
    fn cpu() -> Self {
        if cfg!(feature = "accelerate") {
            Self::AccelerateCpu
        } else {
            Self::Cpu
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Metal => "AllMiniLML6V2 via Candle Metal",
            Self::Cuda => "AllMiniLML6V2 via Candle CUDA",
            Self::AccelerateCpu => "AllMiniLML6V2 via Candle CPU (Accelerate)",
            Self::Cpu => "AllMiniLML6V2 via Candle CPU",
        }
    }

    fn accelerator(self) -> bool {
        matches!(self, Self::Metal | Self::Cuda)
    }
}

#[cfg(feature = "neural")]
fn preferred_neural_backend() -> NeuralBackend {
    #[cfg(feature = "metal")]
    if candle_core::utils::metal_is_available() {
        return NeuralBackend::Metal;
    }

    #[cfg(feature = "cuda")]
    if candle_core::utils::cuda_is_available() {
        return NeuralBackend::Cuda;
    }

    NeuralBackend::cpu()
}

#[cfg(feature = "neural")]
pub struct CandleEmbeddingModel {
    /// Pool of independent embedder instances. `candle_embed`'s `embed_batch`
    /// is a sequential `for text in texts` loop of single-text, single-threaded
    /// forward passes, so a lone instance behind a mutex uses ~1 core regardless
    /// of the thread budget. To actually use the allotted cores we run forwards
    /// in parallel — one embedder per worker thread, so there is no mutex
    /// contention. Foreground (query) embedding only ever needs one.
    pool: Vec<parking_lot::Mutex<candle_embed::BasedBertEmbedder>>,
    backend: NeuralBackend,
    profile: NeuralProfile,
}

/// Embed `texts` across up to `workers` OS threads, preserving input order.
///
/// Uses `std::thread`, **not** rayon, deliberately. candle's CPU kernels
/// parallelize via rayon's GLOBAL pool internally (see
/// `candle_core::cpu_backend`), so fanning out our batch on rayon too nests on
/// that same pool: every worker ends up parked in `collect()`/`Sleep::sleep`
/// waiting for nested jobs that can never be scheduled, and background neural
/// enhancement hangs at ~0% CPU after an initial burst (it never completes).
/// Plain threads aren't rayon workers, so candle's global-pool work always has
/// free workers and runs to completion.
///
/// `embed_slice(worker_idx, slice)` returns one vector per text in `slice`.
#[cfg(feature = "neural")]
fn parallel_embed<F>(texts: &[&str], workers: usize, embed_slice: F) -> Vec<Vec<f32>>
where
    F: Fn(usize, &[&str]) -> Vec<Vec<f32>> + Sync,
{
    if texts.is_empty() {
        return Vec::new();
    }
    if workers <= 1 || texts.len() <= 1 {
        return embed_slice(0, texts);
    }
    let chunk = texts.len().div_ceil(workers);
    let embed_slice = &embed_slice;
    std::thread::scope(|s| {
        let handles: Vec<_> = texts
            .chunks(chunk)
            .enumerate()
            .map(|(i, slice)| s.spawn(move || embed_slice(i, slice)))
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("embed worker thread panicked"))
            .collect()
    })
}

#[cfg(feature = "neural")]
fn validate_accelerator_embedder(
    embedder: &candle_embed::BasedBertEmbedder,
    backend: NeuralBackend,
) -> anyhow::Result<()> {
    let vector = embedder.embed_one("ivygrep neural backend validation probe")?;
    if vector.len() != embedder.model_dimensions {
        anyhow::bail!(
            "{} produced {} dimensions, expected {}",
            backend.label(),
            vector.len(),
            embedder.model_dimensions
        );
    }
    if vector.iter().any(|value| !value.is_finite()) {
        anyhow::bail!(
            "{} produced a non-finite validation vector",
            backend.label()
        );
    }
    if vector.iter().all(|value| value.abs() <= f32::EPSILON) {
        anyhow::bail!("{} produced a zero validation vector", backend.label());
    }
    Ok(())
}

#[cfg(feature = "neural")]
impl CandleEmbeddingModel {
    pub fn new() -> anyhow::Result<Self> {
        Self::new_internal(false)
    }

    pub fn new_background() -> anyhow::Result<Self> {
        Self::new_internal(true)
    }

    fn new_internal(is_background: bool) -> anyhow::Result<Self> {
        // When running as a background enhancement process, limit rayon's
        // global thread pool to 25% of cores (min 1) so the system stays
        // responsive. This affects both the Candle BLAS work-stealing and
        // any par_iter calls in the enhancement pipeline.
        let cpu_pool_size = if is_background {
            // Cap the background budget to at most 8. The cap applies to BOTH
            // the rayon pool and the embedder pool so worker count == embedder
            // count: in embed_batch each worker maps to its own embedder, so if
            // there were more workers than embedders the extras would collide on
            // a shared mutex and serialize (wasting budget on high-core hosts).
            // It also bounds model-copy memory (~tens of MB per instance).
            let bg_threads = (num_cpus::get() / 4).clamp(1, 8);
            let _ = rayon::ThreadPoolBuilder::new()
                .num_threads(bg_threads)
                .build_global();
            tracing::info!("background mode: rayon global pool limited to {bg_threads} thread(s)");
            bg_threads
        } else {
            // Foreground/query model embeds one text at a time — a pool of one.
            1
        };

        use candle_embed::{CandleEmbedBuilder, WithModel};
        let profile = NeuralProfile::configured();

        let build_one = |requested: NeuralBackend| -> anyhow::Result<(
            candle_embed::BasedBertEmbedder,
            NeuralBackend,
        )> {
            let builder = match profile {
                NeuralProfile::General => {
                    CandleEmbedBuilder::new().set_model_from_presets(WithModel::AllMinilmL6V2)
                }
                NeuralProfile::Code => {
                    CandleEmbedBuilder::new().custom_embedding_model(profile.model_id())
                }
            }
            .custom_model_revision(profile.model_revision());
            let builder = match requested {
                NeuralBackend::Metal => builder.with_device_metal(),
                NeuralBackend::Cuda => builder.with_device_any_cuda(),
                NeuralBackend::AccelerateCpu | NeuralBackend::Cpu => builder.with_device_cpu(),
            };
            let embedder = builder.build()?;
            embedder.load_tokenizer()?;
            embedder.load_model()?;
            let actual = match embedder.active_device_name() {
                Some("metal") => NeuralBackend::Metal,
                Some("cuda") => NeuralBackend::Cuda,
                _ => NeuralBackend::cpu(),
            };
            if actual.accelerator() {
                validate_accelerator_embedder(&embedder, actual)?;
            }
            Ok((embedder, actual))
        };

        let preferred = preferred_neural_backend();
        let (first, backend) = match build_one(preferred) {
            Ok(loaded) => loaded,
            Err(accelerator_error) if preferred.accelerator() => {
                tracing::warn!(
                    "failed to initialize {}; falling back to local CPU inference: {accelerator_error:#}",
                    preferred.label()
                );
                build_one(NeuralBackend::cpu())?
            }
            Err(error) => return Err(error),
        };
        // CPU throughput benefits from independent model instances. For GPU
        // inference, replicate only after a measured win: each copy uploads
        // the full model and can needlessly consume unified memory or VRAM.
        let pool_size = if backend.accelerator() {
            1
        } else {
            cpu_pool_size
        };
        let mut pool = Vec::with_capacity(pool_size);
        pool.push(parking_lot::Mutex::new(first));

        for i in 1..pool_size {
            match build_one(backend) {
                Ok((embedder, actual)) if actual == backend => {
                    pool.push(parking_lot::Mutex::new(embedder));
                }
                Ok((_embedder, actual)) => {
                    tracing::warn!(
                        "neural embedder pool: worker {} selected {} instead of {}; continuing with {} instance(s)",
                        i + 1,
                        actual.label(),
                        backend.label(),
                        pool.len()
                    );
                    break;
                }
                // Already have at least one working embedder: degrade to fewer
                // workers rather than disabling neural enhancement entirely if a
                // later copy can't be allocated (e.g. OOM / limited VRAM loading
                // the Nth instance).
                Err(e) => {
                    tracing::warn!(
                        "neural embedder pool: loaded {} of {} instances; continuing with fewer ({e:#})",
                        pool.len(),
                        pool_size
                    );
                    break;
                }
            }
        }

        Ok(Self {
            pool,
            backend,
            profile,
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
        self.pool[0]
            .lock()
            .embed_one(text)
            .unwrap_or_else(|_| vec![0.0; 384])
    }

    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        // Fan out across OS threads, one contiguous slice per embedder, so each
        // embedder is touched by exactly one thread (no mutex contention) and
        // input order is preserved. Deliberately NOT rayon: candle's matmul
        // uses rayon's global pool internally, so a rayon fan-out nests on the
        // same pool and deadlocks — see `parallel_embed`.
        let n = self.pool.len();
        parallel_embed(texts, n, |i, slice| {
            let embedder = &self.pool[i % n];
            slice
                .iter()
                .map(|t| {
                    embedder
                        .lock()
                        .embed_one(t)
                        .unwrap_or_else(|_| vec![0.0; 384])
                })
                .collect()
        })
    }

    fn backend_info(&self) -> Option<&'static str> {
        Some(self.backend.label())
    }

    fn profile_info(&self) -> Option<&'static str> {
        Some(self.profile.name())
    }

    fn respects_system_constraints(&self) -> bool {
        true
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
    use serial_test::serial;

    #[test]
    #[serial]
    fn neural_profile_selection_is_explicit_and_stable() {
        unsafe { std::env::remove_var("IVYGREP_MODEL_PROFILE") };
        assert_eq!(NeuralProfile::configured(), NeuralProfile::General);
        assert_eq!(NeuralProfile::General.name(), "general");
        assert_eq!(NeuralProfile::General.dimensions(), 384);

        unsafe { std::env::set_var("IVYGREP_MODEL_PROFILE", "code") };
        assert_eq!(NeuralProfile::configured(), NeuralProfile::Code);
        assert_eq!(NeuralProfile::Code.name(), "code-minilm-l6-v1");
        assert_eq!(NeuralProfile::Code.dimensions(), 384);
        unsafe { std::env::remove_var("IVYGREP_MODEL_PROFILE") };
    }

    #[cfg(feature = "neural")]
    #[test]
    fn neural_profiles_pin_model_revisions() {
        assert_eq!(
            NeuralProfile::General.model_revision(),
            "1110a243fdf4706b3f48f1d95db1a4f5529b4d41"
        );
        assert_eq!(
            NeuralProfile::Code.model_id(),
            "isuruwijesiri/all-MiniLM-L6-v2-code-search-512"
        );
        assert_eq!(
            NeuralProfile::Code.model_revision(),
            "13b266a617039c16d924b49a56ae978dbd8727ff"
        );
    }

    /// `parallel_embed` must return vectors in input order no matter how the
    /// texts are split across worker threads (regression guard for the
    /// std::thread fan-out that replaced the deadlocking rayon version).
    #[cfg(feature = "neural")]
    #[test]
    fn parallel_embed_preserves_order_across_workers() {
        let texts: Vec<String> = (0..50).map(|i| format!("t{i}")).collect();
        let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        for workers in [1usize, 2, 3, 8, 64] {
            let out = parallel_embed(&refs, workers, |_i, slice| {
                slice
                    .iter()
                    .map(|t| vec![t.trim_start_matches('t').parse::<f32>().unwrap()])
                    .collect()
            });
            assert_eq!(out.len(), 50, "count wrong for workers={workers}");
            for (i, v) in out.iter().enumerate() {
                assert_eq!(v[0], i as f32, "order broken at {i} (workers={workers})");
            }
        }
        // Edge cases: empty input, and a single text with many workers.
        let empty = parallel_embed(&[], 4, |_, s| s.iter().map(|_| vec![0.0]).collect());
        assert!(empty.is_empty());
        let one = parallel_embed(&["a"], 8, |_, s| s.iter().map(|_| vec![1.0]).collect());
        assert_eq!(one, vec![vec![1.0]]);
    }

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

    #[cfg(feature = "neural")]
    #[test]
    fn cpu_backend_label_reports_accelerate_feature_truthfully() {
        let expected = if cfg!(feature = "accelerate") {
            "AllMiniLML6V2 via Candle CPU (Accelerate)"
        } else {
            "AllMiniLML6V2 via Candle CPU"
        };
        assert_eq!(NeuralBackend::cpu().label(), expected);
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

    #[test]
    fn embed_batch_large_batch_matches_sequential() {
        // Verify that the bounded thread pool produces identical results to
        // sequential embedding even with a large batch (exercises pool reuse).
        let model = HashEmbeddingModel::new(128);
        let texts: Vec<String> = (0..200)
            .map(|i| format!("fn function_{i}(x: i32) -> i32 {{ x + {i} }}"))
            .collect();
        let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();

        let batch_results = model.embed_batch(&refs);
        let sequential_results: Vec<Vec<f32>> = refs.iter().map(|t| model.embed(t)).collect();

        assert_eq!(batch_results.len(), sequential_results.len());
        for (i, (batch, seq)) in batch_results
            .iter()
            .zip(sequential_results.iter())
            .enumerate()
        {
            assert_eq!(batch, seq, "batch[{i}] differs from sequential[{i}]");
        }
    }

    #[test]
    fn embed_batch_repeated_calls_consistent() {
        // Verify thread pool reuse (OnceLock) produces consistent results.
        let model = HashEmbeddingModel::new(64);
        let texts = vec!["fn alpha() {}", "fn beta() {}", "fn gamma() {}"];
        let r1 = model.embed_batch(&texts);
        let r2 = model.embed_batch(&texts);
        assert_eq!(
            r1, r2,
            "repeated embed_batch calls should produce identical results"
        );
    }

    #[test]
    fn embed_batch_empty_input() {
        let model = HashEmbeddingModel::new(64);
        let empty: Vec<&str> = vec![];
        let results = model.embed_batch(&empty);
        assert!(results.is_empty());
    }
}
