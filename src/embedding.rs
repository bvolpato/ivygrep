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
use std::path::PathBuf;

use crate::config;
use crate::text::{singularize_token, split_identifier_segments};

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
            match OnnxEmbeddingModel::new() {
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
        let model = OnnxEmbeddingModel::new()?;
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
        let model = OnnxEmbeddingModel::new_background()?;
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

// ── ONNX neural embedding (behind `neural` feature) ───────────────────────

#[cfg(feature = "neural")]
pub struct OnnxEmbeddingModel {
    model: parking_lot::Mutex<fastembed::TextEmbedding>,
}

/// Maximum ONNX inter/intra-op thread count for background enhancement.
/// Uses 25% of logical CPUs (capped at 4) so the system stays responsive.
#[cfg(all(feature = "neural", target_os = "linux"))]
fn ort_thread_budget() -> usize {
    let cpus = num_cpus::get();
    (cpus / 4).clamp(2, 4)
}

/// Adaptive check for CoreML in background daemon phase.
#[cfg(all(feature = "neural", target_os = "macos"))]
fn device_context_allows_coreml(is_background: bool) -> bool {
    use std::process::Command;

    // In the background daemon phase, we adaptively disable CoreML (which uses ANE/GPU
    // and can freeze the UI on heavy workloads) if the system is contextually constrained.
    if is_background {
        // 1. Check if we're on battery power
        if let Ok(output) = Command::new("pmset").arg("-g").arg("batt").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("Battery Power") {
                tracing::info!("Battery power detected, disabling CoreML in background");
                return false;
            }
        }

        // 2. Check if the system is already under heavy load
        let mut loadavg = [0.0f64; 3];
        let has_load = unsafe { libc::getloadavg(loadavg.as_mut_ptr(), 3) };
        if has_load > 0 {
            let load1 = loadavg[0];
            let cpus = num_cpus::get() as f64;
            // If 1-minute load average > 70% of logical cores, system is quite busy
            if load1 > cpus * 0.7 {
                tracing::info!(
                    "System load is high ({:.2}), disabling CoreML in background",
                    load1
                );
                return false;
            }
        }
    }

    true
}

/// Register CoreML execution provider (Apple Neural Engine / GPU).
#[cfg(all(feature = "neural", target_os = "macos"))]
fn register_hardware_acceleration(is_background: bool) {
    if !device_context_allows_coreml(is_background) {
        tracing::info!("CoreML skipped due to adaptive device context rules");
        return;
    }

    let _ = ort::init()
        .with_execution_providers([
            ort::execution_providers::CoreMLExecutionProvider::default().build()
        ])
        .commit();
    tracing::info!("CoreML execution provider registered");
}

/// Probe whether cuDNN is actually loadable at runtime.
/// ONNX Runtime's CUDAExecutionProvider reports `is_available() == true`
/// when the CUDA toolkit exists, but silently falls back to CPU if cuDNN
/// is missing. We dlopen libcudnn to give an honest answer.
#[cfg(all(feature = "neural", target_os = "linux"))]
fn cudnn_is_loadable() -> bool {
    unsafe {
        let handle = libc::dlopen(c"libcudnn.so".as_ptr(), libc::RTLD_LAZY | libc::RTLD_NOLOAD);
        if !handle.is_null() {
            libc::dlclose(handle);
            return true;
        }
        // Try loading it for real (slower but definitive)
        let handle = libc::dlopen(c"libcudnn.so".as_ptr(), libc::RTLD_LAZY);
        if !handle.is_null() {
            libc::dlclose(handle);
            return true;
        }
        false
    }
}

#[cfg(all(feature = "neural", target_os = "linux"))]
fn register_hardware_acceleration(_is_background: bool) {
    if !cudnn_is_loadable() {
        tracing::info!("cuDNN not found — CUDA EP skipped, using CPU");
        return;
    }

    let _ = ort::init()
        .with_execution_providers([
            ort::execution_providers::CUDAExecutionProvider::default().build()
        ])
        .commit();
    tracing::info!("CUDA execution provider registered (if available)");
}

#[cfg(all(feature = "neural", not(target_os = "macos"), not(target_os = "linux")))]
fn register_hardware_acceleration(_is_background: bool) {}

pub fn hardware_acceleration_info() -> &'static str {
    #[cfg(feature = "neural")]
    {
        #[cfg(target_os = "macos")]
        {
            use ort::ep::ExecutionProvider;
            if ort::execution_providers::CoreMLExecutionProvider::default()
                .is_available()
                .unwrap_or(false)
            {
                return "AllMiniLML6V2Q via CoreML";
            }
        }

        #[cfg(target_os = "linux")]
        {
            use ort::ep::ExecutionProvider;
            if ort::execution_providers::CUDAExecutionProvider::default()
                .is_available()
                .unwrap_or(false)
                && cudnn_is_loadable()
            {
                return "AllMiniLML6V2Q via CUDA";
            }
        }

        "AllMiniLML6V2Q via CPU"
    }
    #[cfg(not(feature = "neural"))]
    {
        "Disabled"
    }
}

#[cfg(feature = "neural")]
impl OnnxEmbeddingModel {
    /// Initialize the neural model.  On first run this downloads
    /// `all-MiniLM-L6-v2` (~23 MB) to `~/.local/share/ivygrep/models/`.
    pub fn new() -> anyhow::Result<Self> {
        Self::new_internal(false)
    }

    /// Initialize with limited thread count for background processing.
    pub fn new_background() -> anyhow::Result<Self> {
        // Limit CPU affinity so that `std::thread::available_parallelism()`
        // returns our budget instead of all cores. fastembed reads this value
        // to set ONNX intra-op threads, and there's no other override path.
        #[cfg(target_os = "linux")]
        {
            use std::mem;
            let budget = ort_thread_budget();
            let mut set: libc::cpu_set_t = unsafe { mem::zeroed() };
            for i in 0..budget {
                unsafe { libc::CPU_SET(i, &mut set) };
            }
            unsafe {
                libc::sched_setaffinity(0, mem::size_of::<libc::cpu_set_t>(), &set);
            }
        }
        Self::new_internal(true)
    }

    fn new_internal(is_background: bool) -> anyhow::Result<Self> {
        use fastembed::{EmbeddingModel as FastModel, InitOptions};

        register_hardware_acceleration(is_background);

        let cache_dir = model_cache_dir();
        std::fs::create_dir_all(&cache_dir)?;

        let needs_download = cache_dir
            .read_dir()
            .ok()
            .and_then(|mut entries| entries.next())
            .is_none();
        if needs_download {
            eprintln!("⟐ Downloading embedding model (~23 MB, one-time)...");
        }

        let model = fastembed::TextEmbedding::try_new(
            InitOptions::new(FastModel::AllMiniLML6V2Q)
                .with_cache_dir(cache_dir)
                .with_show_download_progress(true),
        )?;

        if needs_download {
            eprintln!("✓ Model ready.");
        }

        Ok(Self {
            model: parking_lot::Mutex::new(model),
        })
    }
}

/// Returns the centralized model cache directory inside the ivygrep app home.
/// Falls back to a temp directory if the app home cannot be resolved.
#[cfg(feature = "neural")]
fn model_cache_dir() -> PathBuf {
    config::app_home()
        .map(|home| home.join("models"))
        .unwrap_or_else(|_| std::env::temp_dir().join("ivygrep-models"))
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

    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        if texts.is_empty() {
            return vec![];
        }

        // Fastembed handles batching internally, but passing 50K+ strings at once
        // can cause massive ONNX memory allocations. We explicitly chunk the input
        // string array to cap GPU memory usage (keeps it well under 8GB).
        let mut all_results = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(256) {
            let mut results = self
                .model
                .lock()
                .embed(chunk, None) // fastembed takes &[&str]
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
