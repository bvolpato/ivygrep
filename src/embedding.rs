//! Embedding models for semantic vector search.
//!
//! Three implementations are available:
//!
//! | Model | Feature | Dimensions | Quality | Binary size |
//! |-------|---------|------------|---------|-------------|
//! | [`HashEmbeddingModel`] | *(always)* | 256 | Moderate — token overlap heuristic | Tiny |
//! | Static retrieval embedding | `neural` | 256 | High — portable learned retrieval | Model download on first use |
//! | [`CandleEmbeddingModel`] | `neural` | 384 | Optional transformer profiles | Model download on first use |
//!
//! Use [`create_model`] to build the right model based on the `neural` flag.

#[cfg(feature = "neural")]
use crate::system_resources::available_memory_bytes;
use crate::text::{singularize_token, split_identifier_segments};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum NeuralProfile {
    Static,
    PotionCode,
    General,
    Code,
    CodeHighQuality,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NeuralModelIdentity {
    pub schema_version: u32,
    pub profile: String,
    pub model_id: String,
    pub revision: String,
    pub architecture: String,
    pub dimensions: usize,
    pub max_input_tokens: usize,
    pub document_character_limit: usize,
    pub pooling: String,
    pub normalize_embeddings: bool,
    pub model_weight_dtype: String,
    pub vector_quantization: String,
    pub license: String,
    pub parameter_count: u64,
    pub model_asset_bytes: u64,
    pub model_weights_sha256: String,
}

impl NeuralProfile {
    pub fn configured() -> Self {
        match std::env::var("IVYGREP_MODEL_PROFILE")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "static" | "portable" | "static-retrieval" | "static-retrieval-v1" => Self::Static,
            "potion" | "potion-code" | "potion-code-16m" | "model2vec-code" => Self::PotionCode,
            "general" | "minilm" | "all-minilm-l6-v2" => Self::General,
            "code" | "codesearchnet" | "code-minilm-l6-v1" => Self::Code,
            "code-hq" | "code-high-quality" | "code-minilm-l12-v1" => Self::CodeHighQuality,
            _ => Self::Static,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Static => "static-retrieval-v1",
            Self::PotionCode => "potion-code-16m-v1",
            Self::General => "general",
            Self::Code => "code-minilm-l6-v1",
            Self::CodeHighQuality => "code-minilm-l12-v1",
        }
    }

    pub fn dimensions(self) -> usize {
        match self {
            Self::Static | Self::PotionCode => 256,
            Self::General | Self::Code | Self::CodeHighQuality => 384,
        }
    }

    #[cfg(feature = "neural")]
    fn model_id(self) -> &'static str {
        match self {
            Self::Static => "sentence-transformers/static-retrieval-mrl-en-v1",
            Self::PotionCode => "minishlab/potion-code-16M",
            Self::General => "sentence-transformers/all-MiniLM-L6-v2",
            Self::Code => "isuruwijesiri/all-MiniLM-L6-v2-code-search-512",
            Self::CodeHighQuality => "isuruwijesiri/all-MiniLM-L12-v2-code-search-512",
        }
    }

    pub fn model_revision(self) -> &'static str {
        match self {
            Self::Static => "f60985c706f192d45d218078e49e5a8b6f15283a",
            Self::PotionCode => "86848193a842865570d9c8d3e7d268b66ab52752",
            Self::General => "1110a243fdf4706b3f48f1d95db1a4f5529b4d41",
            Self::Code => "13b266a617039c16d924b49a56ae978dbd8727ff",
            Self::CodeHighQuality => "0574cd81b67ad333192c62bb5da302bec71818fe",
        }
    }

    pub fn model_asset_bytes(self) -> u64 {
        match self {
            Self::Static => 125_729_604,
            Self::PotionCode => 65_360_349,
            Self::General => 91_335_235,
            Self::Code => 91_576_452,
            Self::CodeHighQuality => 134_174_389,
        }
    }

    pub fn model_weights_sha256(self) -> &'static str {
        match self {
            Self::Static => "164fc63ee9f9267be7378fcbd7df99d09788a2f45244c92aa99ae5a574925716",
            Self::PotionCode => "ca6159081a6e96cebe4ad878e5e8437bfccc761e8db16223370149cd2faa6c0b",
            Self::General => "53aa51172d142c89d9012cce15ae4d6cc0ca6895895114379cacb4fab128d9db",
            Self::Code => "c71b7305a842dc64189c1e2c7b8e58aa0d430d8181afdb1a95db6d0a3617c90b",
            Self::CodeHighQuality => {
                "038856ae48e815d7510a2277e23650aab3293cbc2e21ffb7c22a86c1854ba109"
            }
        }
    }

    pub fn identity(self) -> NeuralModelIdentity {
        let (
            model_id,
            architecture,
            dimensions,
            max_input_tokens,
            pooling,
            parameter_count,
            license,
        ) = match self {
            Self::Static => (
                "sentence-transformers/static-retrieval-mrl-en-v1",
                "static-embedding",
                256,
                0,
                "token-mean",
                31_254_528,
                "Apache-2.0",
            ),
            Self::PotionCode => (
                "minishlab/potion-code-16M",
                "model2vec-static",
                256,
                512,
                "weighted-token-mean",
                15_827_456,
                "MIT",
            ),
            Self::General => (
                "sentence-transformers/all-MiniLM-L6-v2",
                "bert",
                384,
                256,
                "attention-mask-mean",
                22_713_728,
                "Apache-2.0",
            ),
            Self::Code => (
                "isuruwijesiri/all-MiniLM-L6-v2-code-search-512",
                "bert",
                384,
                512,
                "attention-mask-mean",
                22_713_216,
                "Apache-2.0",
            ),
            Self::CodeHighQuality => (
                "isuruwijesiri/all-MiniLM-L12-v2-code-search-512",
                "bert",
                384,
                512,
                "attention-mask-mean",
                33_360_000,
                "Apache-2.0",
            ),
        };
        NeuralModelIdentity {
            schema_version: 1,
            profile: self.name().to_string(),
            model_id: model_id.to_string(),
            revision: self.model_revision().to_string(),
            architecture: architecture.to_string(),
            dimensions,
            max_input_tokens,
            document_character_limit: 1024,
            pooling: pooling.to_string(),
            normalize_embeddings: true,
            model_weight_dtype: "f32".to_string(),
            vector_quantization: "f16".to_string(),
            license: license.to_string(),
            parameter_count,
            model_asset_bytes: self.model_asset_bytes(),
            model_weights_sha256: self.model_weights_sha256().to_string(),
        }
    }
}

pub fn configured_neural_profile_name() -> &'static str {
    NeuralProfile::configured().name()
}

pub fn configured_neural_model_identity() -> NeuralModelIdentity {
    NeuralProfile::configured().identity()
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

    /// Run any backend-specific warmup needed before serving latency-sensitive
    /// search requests.
    fn warm_for_search(&self) {}

    /// Human-readable backend used to create persisted neural vectors.
    fn backend_info(&self) -> Option<&'static str> {
        None
    }

    fn profile_info(&self) -> Option<&'static str> {
        None
    }

    fn model_identity(&self) -> Option<&NeuralModelIdentity> {
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
            NeuralProfile::configured().dimensions()
        }
        #[cfg(not(feature = "neural"))]
        {
            256
        }
    }
}

/// Create the appropriate embedding model.
///
/// By default (when `hash` is `false`), returns the portable static retrieval
/// model selected by the public embedding bake-off.
/// Pass `hash = true` to use the lightweight [`HashEmbeddingModel`] instead.
///
/// If the `neural` feature is not compiled in, always falls back to hash.
pub fn create_model(hash: bool) -> Box<dyn EmbeddingModel> {
    if !hash {
        #[cfg(feature = "neural")]
        {
            match create_configured_neural_model(false) {
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
        let model = create_configured_neural_model(false)?;
        Ok(Box::new(model))
    }
    #[cfg(not(feature = "neural"))]
    {
        anyhow::bail!("neural feature not compiled in")
    }
}

/// Create a neural model with reduced thread budget for background work.
/// Uses up to a quarter of the CPU cores so the system stays responsive.
pub fn create_neural_model_background() -> anyhow::Result<Box<dyn EmbeddingModel>> {
    #[cfg(feature = "neural")]
    {
        let model = create_configured_neural_model(true)?;
        Ok(Box::new(model))
    }
    #[cfg(not(feature = "neural"))]
    {
        anyhow::bail!("neural feature not compiled in")
    }
}

#[cfg(feature = "neural")]
fn create_configured_neural_model(is_background: bool) -> anyhow::Result<ConfiguredNeuralModel> {
    let profile = NeuralProfile::configured();
    match profile {
        NeuralProfile::Static | NeuralProfile::PotionCode => {
            StaticEmbeddingModel::new(profile, is_background)
                .map(Box::new)
                .map(ConfiguredNeuralModel::Static)
        }
        NeuralProfile::General | NeuralProfile::Code | NeuralProfile::CodeHighQuality => {
            CandleEmbeddingModel::new_internal(is_background)
                .map(Box::new)
                .map(ConfiguredNeuralModel::Candle)
        }
    }
}

#[cfg(feature = "neural")]
fn neural_thread_budget(is_background: bool) -> usize {
    let requested = std::env::var("IVYGREP_NEURAL_THREADS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0);
    neural_thread_budget_for(is_background, num_cpus::get(), requested)
}

#[cfg(feature = "neural")]
fn neural_thread_budget_for(
    is_background: bool,
    logical_cpus: usize,
    requested: Option<usize>,
) -> usize {
    let default_threads = if is_background {
        (logical_cpus / 4).clamp(1, 8)
    } else {
        logical_cpus.clamp(1, 8)
    };
    requested.unwrap_or(default_threads).clamp(1, 32)
}

#[cfg(feature = "neural")]
const MIB: u64 = 1024 * 1024;
#[cfg(feature = "neural")]
const TRANSFORMER_RUNTIME_BASE_BYTES: u64 = 128 * MIB;
#[cfg(feature = "neural")]
const TRANSFORMER_WORKER_BYTES: u64 = 16 * MIB;

#[cfg(feature = "neural")]
fn configured_neural_memory_budget_bytes() -> Option<u64> {
    std::env::var("IVYGREP_NEURAL_MEMORY_MB")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .and_then(|value| value.checked_mul(MIB))
}

#[cfg(feature = "neural")]
fn transformer_pool_size(
    profile: NeuralProfile,
    requested_workers: usize,
    available_bytes: Option<u64>,
    configured_budget_bytes: Option<u64>,
) -> usize {
    let requested_workers = requested_workers.max(1);
    let budget = configured_budget_bytes.or_else(|| available_bytes.map(|bytes| bytes / 4));
    let Some(budget) = budget else {
        return requested_workers;
    };

    let shared_model_bytes = profile
        .model_asset_bytes()
        .saturating_add(TRANSFORMER_RUNTIME_BASE_BYTES);
    if budget <= shared_model_bytes {
        return 1;
    }

    let additional_workers = ((budget - shared_model_bytes) / TRANSFORMER_WORKER_BYTES)
        .min(requested_workers.saturating_sub(1) as u64) as usize;
    1 + additional_workers
}

#[cfg(feature = "neural")]
fn configured_neural_accelerator_handles() -> Option<usize> {
    std::env::var("IVYGREP_NEURAL_ACCELERATOR_HANDLES")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .map(|value| value.min(8))
}

#[cfg(feature = "neural")]
fn accelerator_pool_size_for(
    is_background: bool,
    requested_workers: usize,
    configured: Option<usize>,
) -> usize {
    if !is_background {
        return 1;
    }
    let requested_workers = requested_workers.max(1);
    let default_handles = 2.min(requested_workers);
    configured
        .unwrap_or(default_handles)
        .clamp(1, 8)
        .min(requested_workers)
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
enum ConfiguredNeuralModel {
    Static(Box<StaticEmbeddingModel>),
    Candle(Box<CandleEmbeddingModel>),
}

#[cfg(feature = "neural")]
impl EmbeddingModel for ConfiguredNeuralModel {
    fn dimensions(&self) -> usize {
        match self {
            Self::Static(model) => model.dimensions(),
            Self::Candle(model) => model.dimensions(),
        }
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        match self {
            Self::Static(model) => model.embed(text),
            Self::Candle(model) => model.embed(text),
        }
    }

    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        match self {
            Self::Static(model) => model.embed_batch(texts),
            Self::Candle(model) => model.embed_batch(texts),
        }
    }

    fn warm_for_search(&self) {
        let _ = self.embed("semantic search warmup");
    }

    fn backend_info(&self) -> Option<&'static str> {
        match self {
            Self::Static(model) => model.backend_info(),
            Self::Candle(model) => model.backend_info(),
        }
    }

    fn profile_info(&self) -> Option<&'static str> {
        match self {
            Self::Static(model) => model.profile_info(),
            Self::Candle(model) => model.profile_info(),
        }
    }

    fn model_identity(&self) -> Option<&NeuralModelIdentity> {
        match self {
            Self::Static(model) => model.model_identity(),
            Self::Candle(model) => model.model_identity(),
        }
    }

    fn respects_system_constraints(&self) -> bool {
        match self {
            Self::Static(model) => model.respects_system_constraints(),
            Self::Candle(model) => model.respects_system_constraints(),
        }
    }
}

#[cfg(feature = "neural")]
struct StaticEmbeddingModel {
    tokenizer: tokenizers::Tokenizer,
    embeddings: Vec<f32>,
    token_mapping: Option<Vec<usize>>,
    token_weights: Option<Vec<f32>>,
    unknown_token_id: Option<u32>,
    dimensions: usize,
    profile: NeuralProfile,
    identity: NeuralModelIdentity,
    thread_pool: rayon::ThreadPool,
    is_background: bool,
}

#[cfg(feature = "neural")]
impl StaticEmbeddingModel {
    fn new(profile: NeuralProfile, is_background: bool) -> anyhow::Result<Self> {
        use candle_core::{Device, safetensors::MmapedSafetensors};
        use hf_hub::{Repo, RepoType, api::sync::Api};

        let repo = Repo::with_revision(
            profile.model_id().to_string(),
            RepoType::Model,
            profile.model_revision().to_string(),
        );
        let repo = Api::new()?.repo(repo);
        let (tokenizer_asset, weights_asset, embedding_tensor) = match profile {
            NeuralProfile::Static => (
                "0_StaticEmbedding/tokenizer.json",
                "0_StaticEmbedding/model.safetensors",
                "embedding.weight",
            ),
            NeuralProfile::PotionCode => ("tokenizer.json", "model.safetensors", "embeddings"),
            _ => anyhow::bail!("transformer profile must use the Candle embedding backend"),
        };
        let tokenizer_path = repo.get(tokenizer_asset)?;
        let weights_path = repo.get(weights_asset)?;
        let tokenizer =
            tokenizers::Tokenizer::from_file(tokenizer_path).map_err(anyhow::Error::msg)?;
        // SAFETY: the immutable model file remains mapped only while the
        // tensor is copied into owned memory below.
        let tensors = unsafe { MmapedSafetensors::new(weights_path)? };
        let dimensions = profile.dimensions();
        let embeddings = tensors
            .load(embedding_tensor, &Device::Cpu)?
            .narrow(1, 0, dimensions)?
            .contiguous()?
            .flatten_all()?
            .to_vec1::<f32>()?;
        let (token_mapping, token_weights) = if profile == NeuralProfile::PotionCode {
            let mapping = tensors
                .load("mapping", &Device::Cpu)?
                .flatten_all()?
                .to_vec1::<i64>()?
                .into_iter()
                .map(|value| usize::try_from(value).map_err(anyhow::Error::msg))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let token_weights = tensors
                .load("weights", &Device::Cpu)?
                .flatten_all()?
                .to_vec1::<f64>()?
                .into_iter()
                .map(|value| value as f32)
                .collect();
            (Some(mapping), Some(token_weights))
        } else {
            (None, None)
        };
        let unknown_token_id = tokenizer.token_to_id("[UNK]");
        let thread_count = neural_thread_budget(is_background);
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .build()?;
        tracing::info!("static neural pool limited to {thread_count} thread(s)");
        Ok(Self {
            tokenizer,
            embeddings,
            token_mapping,
            token_weights,
            unknown_token_id,
            dimensions,
            profile,
            identity: profile.identity(),
            thread_pool,
            is_background,
        })
    }

    fn embed_inner(&self, text: &str) -> Vec<f32> {
        let Ok(encoding) = self.tokenizer.encode(text, false) else {
            return vec![0.0; self.dimensions];
        };
        let ids = encoding.get_ids();
        if ids.is_empty() {
            return vec![0.0; self.dimensions];
        }
        let mut embedding = vec![0.0f32; self.dimensions];
        let mut token_count = 0usize;
        for &id in ids {
            if self.unknown_token_id == Some(id) {
                continue;
            }
            let token_id = id as usize;
            let embedding_id = self
                .token_mapping
                .as_ref()
                .and_then(|mapping| mapping.get(token_id))
                .copied()
                .unwrap_or(token_id);
            let start = embedding_id * self.dimensions;
            let Some(row) = self.embeddings.get(start..start + self.dimensions) else {
                continue;
            };
            let token_weight = self
                .token_weights
                .as_ref()
                .and_then(|weights| weights.get(token_id))
                .copied()
                .unwrap_or(1.0);
            for (value, token_value) in embedding.iter_mut().zip(row) {
                *value += token_value * token_weight;
            }
            token_count += 1;
        }
        if token_count == 0 {
            return embedding;
        }
        let scale = 1.0 / token_count as f32;
        for value in &mut embedding {
            *value *= scale;
        }
        let norm = embedding
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        if norm > 0.0 {
            for value in &mut embedding {
                *value /= norm;
            }
        }
        embedding
    }
}

#[cfg(feature = "neural")]
impl EmbeddingModel for StaticEmbeddingModel {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        self.embed_inner(text)
    }

    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        use rayon::prelude::*;
        self.thread_pool.install(|| {
            texts
                .par_iter()
                .map(|text| self.embed_inner(text))
                .collect()
        })
    }

    fn backend_info(&self) -> Option<&'static str> {
        Some(match self.profile {
            NeuralProfile::PotionCode => "Model2Vec weighted token mean via Rust",
            _ => "StaticEmbedding token mean via Rust",
        })
    }

    fn profile_info(&self) -> Option<&'static str> {
        Some(self.profile.name())
    }

    fn model_identity(&self) -> Option<&NeuralModelIdentity> {
        Some(&self.identity)
    }

    fn respects_system_constraints(&self) -> bool {
        self.is_background
    }
}

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
            Self::Metal => "BERT embedding via Candle Metal",
            Self::Cuda => "BERT embedding via Candle CUDA",
            Self::AccelerateCpu => "BERT embedding via Candle CPU (Accelerate)",
            Self::Cpu => "BERT embedding via Candle CPU",
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
fn configured_neural_foreground_accelerator() -> bool {
    std::env::var("IVYGREP_NEURAL_FOREGROUND_ACCELERATOR")
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

#[cfg(feature = "neural")]
fn preferred_neural_backend_for(is_background: bool) -> NeuralBackend {
    if is_background || configured_neural_foreground_accelerator() {
        preferred_neural_backend()
    } else {
        NeuralBackend::cpu()
    }
}

#[cfg(feature = "neural")]
pub struct CandleEmbeddingModel {
    /// Pool of per-worker embedder handles. The handles share immutable model
    /// tensors but retain independent tokenizer state. CPU background inference
    /// uses one batched handle per worker so tokenization and forward passes can
    /// use multiple cores without mutex contention. Foreground (query)
    /// embedding only ever needs one.
    pool: Vec<parking_lot::Mutex<candle_embed::BasedBertEmbedder>>,
    backend: NeuralBackend,
    profile: NeuralProfile,
    identity: NeuralModelIdentity,
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
        // Candle's small matrix operations regress sharply when rayon fans out
        // across every logical CPU on large hosts. Keep a laptop-sized default
        // and allow controlled benchmark overrides.
        let neural_threads = neural_thread_budget(is_background);
        let profile = NeuralProfile::configured();
        let requested_pool_size = if is_background { neural_threads } else { 1 };
        let available_bytes = available_memory_bytes();
        let configured_budget_bytes = configured_neural_memory_budget_bytes();
        let cpu_pool_size = transformer_pool_size(
            profile,
            requested_pool_size,
            available_bytes,
            configured_budget_bytes,
        );
        if cpu_pool_size < requested_pool_size {
            tracing::info!(
                "transformer worker pool capped from {requested_pool_size} to {cpu_pool_size} by memory budget"
            );
        }
        let rayon_threads = if is_background {
            cpu_pool_size
        } else {
            neural_threads
        };
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(rayon_threads)
            .build_global();
        tracing::info!("neural rayon pool limited to {rayon_threads} thread(s)");

        use candle_embed::{CandleEmbedBuilder, WithModel};

        let build_one = |requested: NeuralBackend| -> anyhow::Result<(
            candle_embed::BasedBertEmbedder,
            NeuralBackend,
        )> {
            let builder = match profile {
                NeuralProfile::Static | NeuralProfile::PotionCode => {
                    anyhow::bail!("static profile must use the static embedding backend")
                }
                NeuralProfile::General => {
                    CandleEmbedBuilder::new().set_model_from_presets(WithModel::AllMinilmL6V2)
                }
                NeuralProfile::Code | NeuralProfile::CodeHighQuality => {
                    CandleEmbedBuilder::new().custom_embedding_model(profile.model_id())
                }
            }
            .custom_model_revision(profile.model_revision())
            .normalize_embeddings(true);
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

        let preferred = preferred_neural_backend_for(is_background);
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
        let pool_size = if backend.accelerator() {
            accelerator_pool_size_for(
                is_background,
                neural_threads,
                configured_neural_accelerator_handles(),
            )
        } else {
            cpu_pool_size
        };
        tracing::info!("neural embedder pool uses {pool_size} handle(s)");
        let mut embedders = Vec::with_capacity(pool_size);
        embedders.push(first);

        for _ in 1..pool_size {
            match embedders[0].fork_shared() {
                Ok(embedder) => embedders.push(embedder),
                Err(e) => {
                    tracing::warn!(
                        "neural embedder pool: created {} of {} shared workers; continuing with fewer ({e:#})",
                        embedders.len(),
                        pool_size
                    );
                    break;
                }
            }
        }
        let pool = embedders.into_iter().map(parking_lot::Mutex::new).collect();

        Ok(Self {
            pool,
            backend,
            profile,
            identity: profile.identity(),
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
            embedder
                .lock()
                .embed_batch(slice)
                .unwrap_or_else(|_| vec![vec![0.0; 384]; slice.len()])
        })
    }

    fn backend_info(&self) -> Option<&'static str> {
        Some(self.backend.label())
    }

    fn profile_info(&self) -> Option<&'static str> {
        Some(self.profile.name())
    }

    fn model_identity(&self) -> Option<&NeuralModelIdentity> {
        Some(&self.identity)
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
        assert_eq!(NeuralProfile::configured(), NeuralProfile::Static);
        assert_eq!(NeuralProfile::Static.name(), "static-retrieval-v1");
        assert_eq!(NeuralProfile::Static.dimensions(), 256);
        assert_eq!(NeuralProfile::General.name(), "general");
        assert_eq!(NeuralProfile::General.dimensions(), 384);

        unsafe { std::env::set_var("IVYGREP_MODEL_PROFILE", "potion-code") };
        assert_eq!(NeuralProfile::configured(), NeuralProfile::PotionCode);
        assert_eq!(NeuralProfile::PotionCode.name(), "potion-code-16m-v1");
        assert_eq!(NeuralProfile::PotionCode.dimensions(), 256);

        unsafe { std::env::set_var("IVYGREP_MODEL_PROFILE", "code") };
        assert_eq!(NeuralProfile::configured(), NeuralProfile::Code);
        assert_eq!(NeuralProfile::Code.name(), "code-minilm-l6-v1");
        assert_eq!(NeuralProfile::Code.dimensions(), 384);

        unsafe { std::env::set_var("IVYGREP_MODEL_PROFILE", "code-hq") };
        assert_eq!(NeuralProfile::configured(), NeuralProfile::CodeHighQuality);
        assert_eq!(NeuralProfile::CodeHighQuality.name(), "code-minilm-l12-v1");
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
            NeuralProfile::PotionCode.model_id(),
            "minishlab/potion-code-16M"
        );
        assert_eq!(
            NeuralProfile::PotionCode.model_revision(),
            "86848193a842865570d9c8d3e7d268b66ab52752"
        );
        assert_eq!(
            NeuralProfile::Code.model_id(),
            "isuruwijesiri/all-MiniLM-L6-v2-code-search-512"
        );
        assert_eq!(
            NeuralProfile::Code.model_revision(),
            "13b266a617039c16d924b49a56ae978dbd8727ff"
        );
        assert_eq!(
            NeuralProfile::CodeHighQuality.model_revision(),
            "0574cd81b67ad333192c62bb5da302bec71818fe"
        );
    }

    #[cfg(feature = "neural")]
    #[test]
    fn background_neural_thread_budget_is_bounded() {
        assert_eq!(neural_thread_budget_for(true, 2, None), 1);
        assert_eq!(neural_thread_budget_for(true, 32, None), 8);
        assert_eq!(neural_thread_budget_for(false, 32, None), 8);
        assert_eq!(neural_thread_budget_for(true, 32, Some(20)), 20);
        assert_eq!(neural_thread_budget_for(true, 32, Some(64)), 32);
    }

    #[cfg(feature = "neural")]
    #[test]
    fn transformer_pool_is_capped_by_available_memory() {
        let gib = 1024 * MIB;
        assert_eq!(
            transformer_pool_size(NeuralProfile::General, 8, Some(gib), None),
            3
        );
        assert_eq!(
            transformer_pool_size(NeuralProfile::General, 8, Some(2 * gib), None),
            8
        );
        assert_eq!(
            transformer_pool_size(NeuralProfile::General, 8, Some(64 * gib), Some(256 * MIB)),
            3
        );
        assert_eq!(
            transformer_pool_size(NeuralProfile::General, 8, None, None),
            8
        );
    }

    #[cfg(feature = "neural")]
    #[test]
    fn accelerator_pool_is_background_only_and_bounded() {
        assert_eq!(accelerator_pool_size_for(false, 8, None), 1);
        assert_eq!(accelerator_pool_size_for(true, 1, None), 1);
        assert_eq!(accelerator_pool_size_for(true, 8, None), 2);
        assert_eq!(accelerator_pool_size_for(true, 8, Some(2)), 2);
        assert_eq!(accelerator_pool_size_for(true, 4, Some(8)), 4);
    }

    #[test]
    fn neural_identity_covers_vector_compatibility_inputs() {
        let identity = NeuralProfile::CodeHighQuality.identity();
        assert_eq!(identity.dimensions, 384);
        assert_eq!(identity.max_input_tokens, 512);
        assert_eq!(identity.document_character_limit, 1024);
        assert_eq!(identity.pooling, "attention-mask-mean");
        assert!(identity.normalize_embeddings);
        assert_eq!(identity.vector_quantization, "f16");

        let potion = NeuralProfile::PotionCode.identity();
        assert_eq!(potion.dimensions, 256);
        assert_eq!(potion.pooling, "weighted-token-mean");
        assert_eq!(potion.license, "MIT");
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
            "BERT embedding via Candle CPU (Accelerate)"
        } else {
            "BERT embedding via Candle CPU"
        };
        assert_eq!(NeuralBackend::cpu().label(), expected);
    }

    #[cfg(feature = "neural")]
    #[test]
    #[serial]
    fn foreground_backend_defaults_to_cpu_without_override() {
        unsafe { std::env::remove_var("IVYGREP_NEURAL_FOREGROUND_ACCELERATOR") };
        assert_eq!(preferred_neural_backend_for(false), NeuralBackend::cpu());
        assert_eq!(
            preferred_neural_backend_for(true),
            preferred_neural_backend()
        );

        unsafe { std::env::set_var("IVYGREP_NEURAL_FOREGROUND_ACCELERATOR", "1") };
        assert_eq!(
            preferred_neural_backend_for(false),
            preferred_neural_backend()
        );
        unsafe { std::env::remove_var("IVYGREP_NEURAL_FOREGROUND_ACCELERATOR") };
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
