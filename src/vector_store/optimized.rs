use std::fs;
#[cfg(target_os = "windows")]
use std::io::Write;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rayon::prelude::*;
use usearch::{Index, IndexOptions, MetricKind};

use super::{ScalarKind, VectorMatch, VectorTier, top_vector_matches};

const HASH_CONNECTIVITY: usize = 2;
const HASH_EXPANSION_ADD: usize = 8;
const HASH_EXPANSION_SEARCH: usize = 64;
const SERIALIZED_DIMENSIONS_BYTES: u64 = 8;
const SERIALIZED_HEADER_BYTES: u64 = 64;
const SERIALIZED_MAGIC: &[u8] = b"usearch";
const MIN_CAPACITY: usize = 1_024;
const MAX_CAPACITY_GROWTH: usize = 262_144;
const MAX_CAPACITY_GROWTH_BYTES: usize = 128 * 1024 * 1024;
const PARALLEL_SCORE_MIN_KEYS: usize = 5_000;
const BACKUP_EXTENSION: &str = "usearch.bak";

type DotAndNormSquared = fn(&[f32], &[f32]) -> (f32, f32);

fn scalar_dot_and_norm_squared(left: &[f32], right: &[f32]) -> (f32, f32) {
    debug_assert_eq!(left.len(), right.len());
    left.iter()
        .zip(right)
        .fold((0.0f32, 0.0f32), |(dot, norm), (left, right)| {
            (dot + left * right, norm + left * left)
        })
}

fn select_dot_and_norm_squared() -> DotAndNormSquared {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma") {
        return dot_and_norm_squared_avx2_dispatch;
    }
    scalar_dot_and_norm_squared
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn dot_and_norm_squared_avx2_dispatch(left: &[f32], right: &[f32]) -> (f32, f32) {
    // SAFETY: selector checks AVX2 and FMA before returning this function.
    unsafe { dot_and_norm_squared_avx2(left, right) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
unsafe fn dot_and_norm_squared_avx2(left: &[f32], right: &[f32]) -> (f32, f32) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut dot = _mm256_setzero_ps();
    let mut norm = _mm256_setzero_ps();
    let mut offset = 0;
    while offset + 8 <= left.len() {
        // SAFETY: loop bounds guarantee eight readable values from each slice.
        let (left_values, right_values) = unsafe {
            (
                _mm256_loadu_ps(left.as_ptr().add(offset)),
                _mm256_loadu_ps(right.as_ptr().add(offset)),
            )
        };
        dot = _mm256_fmadd_ps(left_values, right_values, dot);
        norm = _mm256_fmadd_ps(left_values, left_values, norm);
        offset += 8;
    }

    let mut dot_lanes = [0.0f32; 8];
    let mut norm_lanes = [0.0f32; 8];
    // SAFETY: both arrays contain eight writable f32 values.
    unsafe {
        _mm256_storeu_ps(dot_lanes.as_mut_ptr(), dot);
        _mm256_storeu_ps(norm_lanes.as_mut_ptr(), norm);
    }
    let mut totals = (
        dot_lanes.into_iter().sum::<f32>(),
        norm_lanes.into_iter().sum::<f32>(),
    );
    for offset in offset..left.len() {
        totals.0 += left[offset] * right[offset];
        totals.1 += left[offset] * left[offset];
    }
    totals
}

pub struct VectorStore {
    path: PathBuf,
    index: Index,
    quantization: ScalarKind,
    // USearch retains a pointer to the buffer passed to view_from_buffer().
    // Keep the index field first so it is dropped before its backing storage.
    #[cfg(target_os = "windows")]
    _readonly_buffer: Option<Box<[u8]>>,
}

fn create_index(dimensions: usize, quantization: ScalarKind, tier: VectorTier) -> Result<Index> {
    let mut options = IndexOptions {
        dimensions,
        metric: MetricKind::Cos,
        quantization: match quantization {
            ScalarKind::F16 => usearch::ScalarKind::F16,
            ScalarKind::F32 => usearch::ScalarKind::F32,
        },
        ..IndexOptions::default()
    };

    // Hash vectors provide first results before neural enhancement. A smaller
    // graph reduces background build cost; neural vectors retain quality
    // defaults. Select by tier only: the default neural profile shares the
    // hash store's 256-dimensional F16 shape, so shape cannot identify tier.
    if tier == VectorTier::Hash {
        options.connectivity = HASH_CONNECTIVITY;
        options.expansion_add = HASH_EXPANSION_ADD;
        options.expansion_search = HASH_EXPANSION_SEARCH;
    }

    Ok(Index::new(&options)?)
}

// USearch mmap/load trusts serialized matrix dimensions before validating its
// header, so reject malformed offsets here instead of risking a native crash.
fn validate_existing_index_file(path: &Path) -> Result<()> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    anyhow::ensure!(
        len >= SERIALIZED_DIMENSIONS_BYTES + SERIALIZED_HEADER_BYTES,
        "vector store is truncated ({len} bytes)"
    );

    let mut dimensions = [0u8; SERIALIZED_DIMENSIONS_BYTES as usize];
    file.read_exact(&mut dimensions)?;
    let rows = u32::from_le_bytes(dimensions[..4].try_into().unwrap()) as u64;
    let bytes_per_vector = u32::from_le_bytes(dimensions[4..].try_into().unwrap()) as u64;
    let vectors_bytes = rows
        .checked_mul(bytes_per_vector)
        .context("vector store dimensions overflow")?;
    let header_offset = SERIALIZED_DIMENSIONS_BYTES
        .checked_add(vectors_bytes)
        .context("vector store header offset overflow")?;
    let required_len = header_offset
        .checked_add(SERIALIZED_HEADER_BYTES)
        .context("vector store length overflow")?;
    anyhow::ensure!(
        required_len <= len,
        "vector store is truncated ({len} bytes, header requires {required_len})"
    );

    file.seek(SeekFrom::Start(header_offset))?;
    let mut magic = [0u8; SERIALIZED_MAGIC.len()];
    file.read_exact(&mut magic)?;
    anyhow::ensure!(
        magic == SERIALIZED_MAGIC,
        "invalid vector store header magic"
    );
    Ok(())
}

impl VectorStore {
    pub fn open(
        path: &Path,
        dimensions: usize,
        quantization: ScalarKind,
        tier: VectorTier,
    ) -> Result<Self> {
        let index = create_index(dimensions, quantization, tier)?;
        if let Some(load_path) = existing_index_path(path) {
            validate_existing_index_file(&load_path)?;
            #[cfg(target_os = "windows")]
            let load_result = {
                let bytes = fs::read(&load_path)?;
                index.load_from_buffer(&bytes)
            };
            #[cfg(not(target_os = "windows"))]
            let load_result = {
                let path_str = load_path
                    .to_str()
                    .context("vector path contains invalid UTF-8")?;
                index.load(path_str)
            };

            match load_result {
                Ok(()) => {}
                Err(err) => {
                    if matches!(quantization, ScalarKind::F32) {
                        // Already F32 — no fallback possible. Propagate the
                        // error instead of silently returning an empty index
                        // (which would wipe the file on the next save()).
                        return Err(err.into());
                    }
                    // Old index may use different quantization; retry with F32.
                    let fallback = create_index(dimensions, ScalarKind::F32, tier)?;
                    #[cfg(target_os = "windows")]
                    {
                        let bytes = fs::read(&load_path)?;
                        fallback.load_from_buffer(&bytes)?;
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        let path_str = load_path
                            .to_str()
                            .context("vector path contains invalid UTF-8")?;
                        fallback.load(path_str)?;
                    }
                    return Ok(Self::new(path, fallback, ScalarKind::F32));
                }
            }
        }

        Ok(Self::new(path, index, quantization))
    }

    fn new(path: &Path, index: Index, quantization: ScalarKind) -> Self {
        Self {
            path: path.to_path_buf(),
            index,
            quantization,
            #[cfg(target_os = "windows")]
            _readonly_buffer: None,
        }
    }

    /// Atomically replace an existing store with a freshly allocated empty index.
    pub(crate) fn reset(
        path: &Path,
        dimensions: usize,
        quantization: ScalarKind,
        tier: VectorTier,
    ) -> Result<()> {
        Self::new(
            path,
            create_index(dimensions, quantization, tier)?,
            quantization,
        )
        .save()
    }

    /// Open for read-only search using memory-mapping on Unix and a retained
    /// serialized buffer on Windows. The Windows path avoids USearch's narrow
    /// native path APIs and keeps the index file replaceable by another process.
    ///
    /// The returned store must NOT be used for writes (upsert/remove/save).
    pub fn open_readonly(
        path: &Path,
        dimensions: usize,
        quantization: ScalarKind,
        tier: VectorTier,
    ) -> Result<Self> {
        let index = create_index(dimensions, quantization, tier)?;
        let Some(load_path) = existing_index_path(path) else {
            return Ok(Self::new(path, index, quantization));
        };
        validate_existing_index_file(&load_path)?;

        #[cfg(target_os = "windows")]
        {
            let buffer = fs::read(&load_path)?.into_boxed_slice();
            // SAFETY: the buffer is stored in VectorStore and the index field is
            // dropped before readonly_buffer, so it outlives every index access.
            let view_result = unsafe { index.view_from_buffer(&buffer) };
            match view_result {
                Ok(()) => Ok(Self {
                    path: path.to_path_buf(),
                    index,
                    quantization,
                    _readonly_buffer: Some(buffer),
                }),
                Err(err) => {
                    if matches!(quantization, ScalarKind::F32) {
                        return Err(err.into());
                    }
                    let fallback = create_index(dimensions, ScalarKind::F32, tier)?;
                    // SAFETY: same retained-buffer lifetime guarantee as above.
                    unsafe { fallback.view_from_buffer(&buffer) }?;
                    Ok(Self {
                        path: path.to_path_buf(),
                        index: fallback,
                        quantization: ScalarKind::F32,
                        _readonly_buffer: Some(buffer),
                    })
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let path_str = path
                .to_str()
                .context("vector path contains invalid UTF-8")?;
            match index.view(path_str) {
                Ok(()) => {}
                Err(err) => {
                    if matches!(quantization, ScalarKind::F32) {
                        return Err(err.into());
                    }
                    let fallback = create_index(dimensions, ScalarKind::F32, tier)?;
                    fallback.view(path_str)?;
                    return Ok(Self::new(path, fallback, ScalarKind::F32));
                }
            }
            Ok(Self::new(path, index, quantization))
        }
    }

    pub fn save(&self) -> Result<()> {
        let parent = self
            .path
            .parent()
            .context("vector store path has no parent")?;
        fs::create_dir_all(parent)?;

        let tmp_path = self.path.with_extension("usearch.tmp");
        #[cfg(target_os = "windows")]
        let save_result: Result<()> = (|| {
            let mut bytes = vec![0; self.index.serialized_length()];
            self.index.save_to_buffer(&mut bytes)?;
            let mut file = fs::File::create(&tmp_path)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            Ok(())
        })();
        #[cfg(not(target_os = "windows"))]
        let save_result: Result<()> = (|| {
            let path_str = tmp_path
                .to_str()
                .context("vector path contains invalid UTF-8")?;
            self.index.save(path_str)?;
            Ok(())
        })();

        if let Err(error) = save_result {
            let _ = fs::remove_file(&tmp_path);
            return Err(error);
        }

        replace_file(&tmp_path, &self.path)?;
        Ok(())
    }

    pub fn contains(&self, key: u64) -> bool {
        self.index.contains(key)
    }

    pub fn remove(&mut self, key: u64) {
        let _ = self.index.remove(key);
    }

    /// Reserve a bounded portion of `additional` entries upfront. Bulk
    /// enhancement can request millions of entries, but USearch allocates
    /// native graph metadata during `reserve`; reserving the whole corpus at
    /// once can trigger the OOM killer before background pressure checks run.
    pub fn reserve_additional(&mut self, additional: usize) -> Result<()> {
        if let Some(target) = self.next_capacity(additional) {
            self.index.reserve(target)?;
        }
        Ok(())
    }

    /// Add a vector without checking for duplicates. Use only when the caller
    /// guarantees the key does not already exist (e.g., fresh enhancement).
    pub fn add_unchecked(&mut self, key: u64, vector: Vec<f32>) -> Result<()> {
        self.validate_vector(&vector)?;
        self.ensure_capacity_for_insert()?;
        self.index.add(key, &vector)?;
        Ok(())
    }

    pub fn upsert(&mut self, key: u64, vector: Vec<f32>) -> Result<()> {
        self.validate_vector(&vector)?;
        self.ensure_capacity_for_insert()?;

        if self.index.contains(key) {
            let _ = self.index.remove(key);
        }
        self.index.add(key, &vector)?;
        Ok(())
    }

    pub fn size(&self) -> usize {
        self.index.size()
    }

    /// Dimensionality of the vectors stored in this index. Useful for asserting
    /// that an index was built with the expected embedding model (e.g. 256-dim
    /// hash vs 384-dim neural).
    pub fn dimensions(&self) -> usize {
        self.index.dimensions()
    }

    pub fn search(&self, query: &[f32], count: usize) -> Vec<VectorMatch> {
        if query.len() != self.index.dimensions() {
            return Vec::new();
        }
        match self.index.search(query, count) {
            Ok(matches) => matches
                .keys
                .iter()
                .zip(matches.distances.iter())
                .map(|(key, distance)| VectorMatch {
                    key: *key,
                    // usearch uses MetricKind::Cos where distance = 1 - cosine,
                    // so recover the true cosine similarity in [-1, 1]. This is
                    // monotonic with -distance (ranking is unchanged) but keeps
                    // the magnitude usable by downstream score normalization,
                    // which clamps to [0, 1].
                    score: 1.0 - distance,
                })
                .collect(),
            Err(_) => vec![],
        }
    }

    /// Score a single vector by key against a query vector.
    /// Returns None if the key doesn't exist in the index.
    pub fn score(&self, key: u64, query: &[f32]) -> Option<f32> {
        if query.len() != self.index.dimensions() || !self.index.contains(key) {
            return None;
        }
        // Use search with the query and check if this key appears
        // For efficiency, retrieve the vector and compute cosine similarity directly
        let dims = query.len();
        let mut stored = vec![0.0f32; dims];
        match self.index.get(key, &mut stored) {
            Ok(_count) => {
                // Cosine similarity: dot(a,b) / (|a| * |b|)
                let dot: f32 = stored.iter().zip(query.iter()).map(|(a, b)| a * b).sum();
                let norm_a: f32 = stored.iter().map(|x| x * x).sum::<f32>().sqrt();
                let norm_b: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm_a > 0.0 && norm_b > 0.0 {
                    Some(dot / (norm_a * norm_b))
                } else {
                    Some(0.0)
                }
            }
            Err(_) => None,
        }
    }

    /// Exactly score selected vectors and retain only the best matches.
    ///
    /// Unlike repeated [`Self::score`] calls, this allocates one retrieval
    /// buffer and computes the query norm once for the whole batch.
    pub fn score_many_top_k(&self, keys: &[u64], query: &[f32], count: usize) -> Vec<VectorMatch> {
        if count == 0 || query.len() != self.index.dimensions() {
            return Vec::new();
        }

        let query_norm = query.iter().map(|value| value * value).sum::<f32>().sqrt();
        let dot_and_norm_squared = select_dot_and_norm_squared();
        let score_chunk = |keys: &[u64]| {
            let mut stored = vec![0.0f32; query.len()];
            let matches = keys.iter().filter_map(|key| {
                if !matches!(self.index.get(*key, &mut stored), Ok(count) if count > 0) {
                    return None;
                }

                let (dot, stored_norm_squared) = dot_and_norm_squared(&stored, query);
                let stored_norm = stored_norm_squared.sqrt();
                let score = if stored_norm > 0.0 && query_norm > 0.0 {
                    dot / (stored_norm * query_norm)
                } else {
                    0.0
                };
                Some(VectorMatch { key: *key, score })
            });
            top_vector_matches(matches, count)
        };

        let threads = rayon::current_num_threads();
        if keys.len() >= PARALLEL_SCORE_MIN_KEYS && threads > 1 {
            let chunk_size = keys.len().div_ceil(threads * 4).max(256);
            let local_matches = keys
                .par_chunks(chunk_size)
                .flat_map_iter(score_chunk)
                .collect::<Vec<_>>();
            top_vector_matches(local_matches, count)
        } else {
            score_chunk(keys)
        }
    }

    fn validate_vector(&self, vector: &[f32]) -> Result<()> {
        anyhow::ensure!(
            vector.len() == self.index.dimensions(),
            "vector dimensions mismatch ({} != {})",
            vector.len(),
            self.index.dimensions()
        );
        Ok(())
    }

    fn ensure_capacity_for_insert(&mut self) -> Result<()> {
        if let Some(target) = self.next_capacity(1) {
            self.index.reserve(target)?;
        }
        Ok(())
    }

    fn next_capacity(&self, additional: usize) -> Option<usize> {
        let size = self.index.size();
        let capacity = self.index.capacity();
        let needed = size.saturating_add(additional);
        if needed <= capacity {
            return None;
        }

        let growth_limit = capacity_growth_limit(
            self.index.dimensions(),
            self.quantization,
            self.index.connectivity(),
        );
        Some(bounded_capacity_target(needed, capacity, growth_limit))
    }
}

fn capacity_growth_limit(
    dimensions: usize,
    quantization: ScalarKind,
    connectivity: usize,
) -> usize {
    let scalar_bytes = match quantization {
        ScalarKind::F16 => 2,
        ScalarKind::F32 => 4,
    };
    let vector_bytes = dimensions.saturating_mul(scalar_bytes);
    // Include a conservative graph/lookup allowance. USearch's exact native
    // layout is implementation-specific, but this keeps the bound responsive
    // to both vector width and HNSW connectivity.
    let graph_bytes = connectivity
        .saturating_mul(2)
        .saturating_mul(std::mem::size_of::<u32>());
    let lookup_and_node_bytes = std::mem::size_of::<u64>() + 2 * std::mem::size_of::<usize>() + 64;
    let estimated_entry_bytes = vector_bytes
        .saturating_add(graph_bytes)
        .saturating_add(lookup_and_node_bytes)
        .max(1);

    (MAX_CAPACITY_GROWTH_BYTES / estimated_entry_bytes).clamp(MIN_CAPACITY, MAX_CAPACITY_GROWTH)
}

fn bounded_capacity_target(needed: usize, capacity: usize, growth_limit: usize) -> usize {
    let geometric_target = match capacity {
        0 => MIN_CAPACITY,
        current => current.saturating_add(current.min(growth_limit)),
    };
    let requested_target = needed.min(capacity.saturating_add(growth_limit));
    geometric_target
        .max(requested_target)
        .max(needed.min(MIN_CAPACITY))
}

/// Delete a store file together with every sibling artifact that `open` or
/// `open_readonly` would otherwise recover from (the Windows backup and an
/// interrupted temporary save). Callers use this when the store must be rebuilt
/// from scratch, e.g. after a vector identity change; removing only the
/// primary file would let the backup resurrect the old vectors.
pub fn remove_store_files(path: &Path) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(path.with_extension("usearch.tmp"));
    let _ = fs::remove_file(path.with_extension(BACKUP_EXTENSION));
}

fn existing_index_path(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return Some(path.to_path_buf());
    }
    #[cfg(target_os = "windows")]
    {
        let backup = path.with_extension(BACKUP_EXTENSION);
        if backup.exists() {
            return Some(backup);
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
fn replace_file(source: &Path, destination: &Path) -> Result<()> {
    fs::rename(source, destination)?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn replace_file(source: &Path, destination: &Path) -> Result<()> {
    let backup = destination.with_extension(BACKUP_EXTENSION);
    if destination.exists() {
        if backup.exists() {
            fs::remove_file(&backup)?;
        }
        fs::rename(destination, &backup)?;
    }
    if let Err(error) = fs::rename(source, destination) {
        if backup.exists() {
            let _ = fs::rename(&backup, destination);
        }
        return Err(error.into());
    }
    if backup.exists() {
        fs::remove_file(backup)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_vector_math_matches_scalar_with_tail_dimensions() {
        let left = (0..259)
            .map(|index| (index as f32 * 0.03125).sin())
            .collect::<Vec<_>>();
        let right = (0..259)
            .map(|index| (index as f32 * 0.0625).cos())
            .collect::<Vec<_>>();
        let expected = scalar_dot_and_norm_squared(&left, &right);
        let actual = select_dot_and_norm_squared()(&left, &right);
        assert!((actual.0 - expected.0).abs() < 1e-4);
        assert!((actual.1 - expected.1).abs() < 1e-4);
    }

    #[test]
    fn search_score_is_cosine_similarity_not_clamped_to_zero() {
        // Regression: search() used to return -distance (range [-2, 0]) which
        // downstream normalization clamped to 0, discarding semantic magnitude.
        // It must now return the true cosine similarity in [0, 1].
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");
        let mut store = VectorStore::open(&path, 4, ScalarKind::F32, VectorTier::Neural).unwrap();
        store.upsert(1, vec![1.0, 0.0, 0.0, 0.0]).unwrap(); // identical to query
        store.upsert(2, vec![0.0, 1.0, 0.0, 0.0]).unwrap(); // orthogonal to query

        let hits = store.search(&[1.0, 0.0, 0.0, 0.0], 2);
        let by_key: std::collections::HashMap<u64, f32> =
            hits.iter().map(|h| (h.key, h.score)).collect();

        assert!(
            by_key[&1] > 0.9,
            "identical vector should score ~1.0 (cosine), got {}",
            by_key[&1]
        );
        assert!(
            by_key[&2].abs() < 0.1,
            "orthogonal vector should score ~0.0 (cosine), got {}",
            by_key[&2]
        );
        assert!(by_key[&1] > by_key[&2]);
    }

    #[test]
    fn vector_store_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");

        let mut store = VectorStore::open(&path, 4, ScalarKind::F32, VectorTier::Neural).unwrap();
        store.upsert(1, vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        store.upsert(2, vec![0.0, 1.0, 0.0, 0.0]).unwrap();
        store.save().unwrap();

        let store = VectorStore::open(&path, 4, ScalarKind::F32, VectorTier::Neural).unwrap();
        let hits = store.search(&[1.0, 0.0, 0.0, 0.0], 2);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].key, 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn unicode_path_roundtrip_uses_buffer_persistence() {
        let tmp = tempfile::tempdir().unwrap();
        let directory = tmp.path().join("ivygrep-数据-é");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("vectors.usearch");

        let mut store = VectorStore::open(&path, 4, ScalarKind::F16, VectorTier::Neural).unwrap();
        store.upsert(7, vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        store.save().unwrap();

        let reopened =
            VectorStore::open_readonly(&path, 4, ScalarKind::F16, VectorTier::Neural).unwrap();
        assert!(reopened.contains(7));
        assert_eq!(reopened.search(&[1.0, 0.0, 0.0, 0.0], 1)[0].key, 7);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn save_replaces_an_existing_index_while_readonly_view_is_open() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.usearch");
        let mut store = VectorStore::open(&path, 2, ScalarKind::F16, VectorTier::Neural).unwrap();
        store.upsert(1, vec![1.0, 0.0]).unwrap();
        store.save().unwrap();

        let readonly =
            VectorStore::open_readonly(&path, 2, ScalarKind::F16, VectorTier::Neural).unwrap();
        store.upsert(2, vec![0.0, 1.0]).unwrap();
        store.save().unwrap();

        assert!(readonly.contains(1));
        assert!(!readonly.contains(2));
        let reopened =
            VectorStore::open_readonly(&path, 2, ScalarKind::F16, VectorTier::Neural).unwrap();
        assert!(reopened.contains(1));
        assert!(reopened.contains(2));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn recovers_interrupted_replacement_from_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.usearch");
        let backup = path.with_extension(BACKUP_EXTENSION);
        let mut store = VectorStore::open(&path, 2, ScalarKind::F16, VectorTier::Neural).unwrap();
        store.upsert(1, vec![1.0, 0.0]).unwrap();
        store.save().unwrap();
        fs::rename(&path, &backup).unwrap();

        let mut recovered =
            VectorStore::open(&path, 2, ScalarKind::F16, VectorTier::Neural).unwrap();
        assert!(recovered.contains(1));
        recovered.upsert(2, vec![0.0, 1.0]).unwrap();
        recovered.save().unwrap();

        let reopened =
            VectorStore::open_readonly(&path, 2, ScalarKind::F16, VectorTier::Neural).unwrap();
        assert!(reopened.contains(1));
        assert!(reopened.contains(2));
        assert!(!backup.exists());
    }

    #[test]
    fn contains_and_remove() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");

        let mut store = VectorStore::open(&path, 4, ScalarKind::F32, VectorTier::Neural).unwrap();
        store.upsert(42, vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        assert!(store.contains(42));
        assert!(!store.contains(99));

        store.remove(42);
        assert!(!store.contains(42));
    }

    #[test]
    fn size_tracks_insertions() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");

        let mut store = VectorStore::open(&path, 4, ScalarKind::F32, VectorTier::Neural).unwrap();
        assert_eq!(store.size(), 0);
        store.upsert(1, vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        assert_eq!(store.size(), 1);
        store.upsert(2, vec![0.0, 1.0, 0.0, 0.0]).unwrap();
        assert_eq!(store.size(), 2);
    }

    #[test]
    fn upsert_replaces_existing_key() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");

        let mut store = VectorStore::open(&path, 4, ScalarKind::F32, VectorTier::Neural).unwrap();
        store.upsert(1, vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        store.upsert(1, vec![0.0, 1.0, 0.0, 0.0]).unwrap();

        assert_eq!(store.size(), 1);
        let hits = store.search(&[0.0, 1.0, 0.0, 0.0], 1);
        assert_eq!(hits[0].key, 1);
    }

    #[test]
    fn invalid_upsert_preserves_existing_vector() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");

        let mut store = VectorStore::open(&path, 4, ScalarKind::F32, VectorTier::Neural).unwrap();
        store.upsert(1, vec![1.0, 0.0, 0.0, 0.0]).unwrap();

        assert!(store.upsert(1, vec![0.0, 1.0]).is_err());
        assert!(store.contains(1));
        assert!(store.score(1, &[1.0, 0.0]).is_none());
        assert!(store.search(&[1.0, 0.0], 1).is_empty());
        assert!(store.score(1, &[1.0, 0.0, 0.0, 0.0]).unwrap() > 0.9);
    }

    #[test]
    fn score_returns_similarity() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");

        let mut store = VectorStore::open(&path, 4, ScalarKind::F32, VectorTier::Neural).unwrap();
        store.upsert(1, vec![1.0, 0.0, 0.0, 0.0]).unwrap();

        let score = store.score(1, &[1.0, 0.0, 0.0, 0.0]);
        assert!(score.is_some());
        assert!(score.unwrap() > 0.9);

        assert!(store.score(999, &[1.0, 0.0, 0.0, 0.0]).is_none());
    }

    #[test]
    fn batch_exact_top_k_matches_scalar_scoring() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");
        let mut store = VectorStore::open(&path, 16, ScalarKind::F16, VectorTier::Neural).unwrap();
        let keys = (0..128u64).rev().collect::<Vec<_>>();
        for key in &keys {
            let vector = (0..16)
                .map(|dimension| (((*key + 3) * (dimension + 5) as u64) % 29) as f32 / 29.0 - 0.5)
                .collect::<Vec<_>>();
            store.add_unchecked(*key, vector).unwrap();
        }
        let query = (0..16)
            .map(|dimension| ((dimension * 7 + 3) % 19) as f32 / 19.0 - 0.5)
            .collect::<Vec<_>>();

        let mut expected = keys
            .iter()
            .filter_map(|key| {
                store
                    .score(*key, &query)
                    .map(|score| VectorMatch { key: *key, score })
            })
            .collect::<Vec<_>>();
        expected.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.key.cmp(&right.key))
        });
        expected.truncate(17);

        let actual = store.score_many_top_k(&keys, &query, 17);
        assert_eq!(
            actual.iter().map(|item| item.key).collect::<Vec<_>>(),
            expected.iter().map(|item| item.key).collect::<Vec<_>>()
        );
        for (actual, expected) in actual.iter().zip(expected) {
            let delta = (actual.score - expected.score).abs();
            assert!(
                delta < 1e-4,
                "native/scalar score delta {delta} exceeded tolerance: {} vs {}",
                actual.score,
                expected.score
            );
        }
    }

    #[test]
    fn batch_exact_top_k_uses_key_order_for_tied_scores() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");
        let mut store = VectorStore::open(&path, 4, ScalarKind::F32, VectorTier::Neural).unwrap();
        for key in [9, 3, 7, 1] {
            store.add_unchecked(key, vec![0.0, 1.0, 0.0, 0.0]).unwrap();
        }

        let matches = store.score_many_top_k(&[9, 3, 7, 1], &[1.0, 0.0, 0.0, 0.0], 3);
        assert_eq!(
            matches.iter().map(|item| item.key).collect::<Vec<_>>(),
            [1, 3, 7]
        );
    }

    #[test]
    fn parallel_batch_exact_top_k_preserves_tie_order() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");
        let mut store = VectorStore::open(&path, 4, ScalarKind::F32, VectorTier::Neural).unwrap();
        let keys = (0..PARALLEL_SCORE_MIN_KEYS as u64)
            .rev()
            .collect::<Vec<_>>();
        for key in &keys {
            store.add_unchecked(*key, vec![0.0, 1.0, 0.0, 0.0]).unwrap();
        }

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap();
        let matches = pool.install(|| store.score_many_top_k(&keys, &[1.0, 0.0, 0.0, 0.0], 50));
        assert_eq!(
            matches.iter().map(|item| item.key).collect::<Vec<_>>(),
            (0..50).collect::<Vec<_>>()
        );
    }

    #[test]
    fn batch_exact_top_k_skips_missing_keys_without_reusing_the_buffer() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");
        let mut store = VectorStore::open(&path, 4, ScalarKind::F32, VectorTier::Neural).unwrap();
        store.add_unchecked(1, vec![1.0, 0.0, 0.0, 0.0]).unwrap();

        let matches = store.score_many_top_k(&[1, 999], &[1.0, 0.0, 0.0, 0.0], 2);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].key, 1);
    }

    #[test]
    fn open_readonly_sees_saved_data() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");

        let mut store = VectorStore::open(&path, 4, ScalarKind::F32, VectorTier::Neural).unwrap();
        store.upsert(1, vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        store.upsert(2, vec![0.0, 1.0, 0.0, 0.0]).unwrap();
        store.save().unwrap();

        let ro = VectorStore::open_readonly(&path, 4, ScalarKind::F32, VectorTier::Neural).unwrap();
        assert_eq!(ro.size(), 2);
        assert!(ro.contains(1));
        assert!(ro.contains(2));
    }

    #[test]
    fn open_rejects_truncated_file_before_native_load() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");
        fs::write(&path, b"truncated").unwrap();

        let err = VectorStore::open_readonly(&path, 4, ScalarKind::F32, VectorTier::Neural)
            .err()
            .expect("truncated vector store should fail");
        assert!(err.to_string().contains("vector store is truncated"));
    }

    #[test]
    fn open_rejects_invalid_header_before_native_load() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");
        fs::write(&path, vec![0; 80]).unwrap();

        let err = VectorStore::open_readonly(&path, 4, ScalarKind::F32, VectorTier::Neural)
            .err()
            .expect("invalid vector store should fail");
        assert!(
            err.to_string()
                .contains("invalid vector store header magic")
        );
    }

    #[test]
    fn capacity_grows_beyond_initial() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");

        let mut store = VectorStore::open(&path, 4, ScalarKind::F32, VectorTier::Neural).unwrap();
        for i in 0..1100 {
            store.upsert(i, vec![i as f32, 0.0, 0.0, 0.0]).unwrap();
        }
        assert_eq!(store.size(), 1100);
    }

    #[test]
    fn bulk_capacity_target_does_not_reserve_the_remaining_corpus() {
        let growth_limit = capacity_growth_limit(384, ScalarKind::F16, 16);

        assert!(growth_limit < 1_000_000);
        assert_eq!(
            bounded_capacity_target(1_000_000, 0, growth_limit),
            growth_limit
        );
        assert_eq!(
            bounded_capacity_target(1_000_000, growth_limit, growth_limit),
            growth_limit * 2
        );
    }

    #[test]
    fn capacity_budget_accounts_for_vector_storage_cost() {
        let hash_f16 = capacity_growth_limit(256, ScalarKind::F16, 8);
        let neural_f16 = capacity_growth_limit(384, ScalarKind::F16, 16);
        let neural_f32 = capacity_growth_limit(384, ScalarKind::F32, 16);

        assert!(hash_f16 > neural_f16);
        assert!(neural_f16 > neural_f32);
        assert!(hash_f16 <= MAX_CAPACITY_GROWTH);
        assert!(neural_f32 >= MIN_CAPACITY);
    }

    #[test]
    fn native_reservation_does_not_amplify_the_bounded_target() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");
        let mut store = VectorStore::open(&path, 4, ScalarKind::F32, VectorTier::Neural).unwrap();
        let requested = 5_000;

        store.reserve_additional(requested).unwrap();

        assert_eq!(store.index.capacity(), requested);
    }

    #[test]
    fn remove_store_files_deletes_recoverable_siblings() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors_neural.usearch");
        let mut store = VectorStore::open(&path, 4, ScalarKind::F16, VectorTier::Neural).unwrap();
        store.upsert(1, vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        store.save().unwrap();
        let backup = path.with_extension(BACKUP_EXTENSION);
        fs::copy(&path, &backup).unwrap();
        fs::write(path.with_extension("usearch.tmp"), b"partial").unwrap();

        remove_store_files(&path);

        assert!(!path.exists());
        assert!(
            !backup.exists(),
            "a stale backup must not resurrect old vectors"
        );
        assert!(!path.with_extension("usearch.tmp").exists());
        let reopened = VectorStore::open(&path, 4, ScalarKind::F16, VectorTier::Neural).unwrap();
        assert_eq!(reopened.size(), 0);
    }

    #[test]
    fn hash_tier_uses_sparse_graph_and_neural_tier_keeps_defaults_for_same_shape() {
        let tmp = tempfile::tempdir().unwrap();
        // The default neural profile shares the hash store's 256-d F16 shape.
        let hash = VectorStore::open(
            &tmp.path().join("hash.bin"),
            256,
            ScalarKind::F16,
            VectorTier::Hash,
        )
        .unwrap();
        let neural = VectorStore::open(
            &tmp.path().join("neural.bin"),
            256,
            ScalarKind::F16,
            VectorTier::Neural,
        )
        .unwrap();
        let defaults = Index::new(&IndexOptions::default()).unwrap();

        assert_eq!(hash.index.connectivity(), HASH_CONNECTIVITY);
        assert_eq!(hash.index.expansion_add(), HASH_EXPANSION_ADD);
        assert_eq!(hash.index.expansion_search(), HASH_EXPANSION_SEARCH);
        assert_eq!(neural.index.connectivity(), defaults.connectivity());
        assert_eq!(neural.index.expansion_add(), defaults.expansion_add());
        assert_eq!(neural.index.expansion_search(), defaults.expansion_search());
    }

    /// Deterministic clustered unit vectors: `clusters` centroids with small
    /// per-vector perturbations, mimicking embedded code chunks.
    fn clustered_unit_vectors(count: usize, dimensions: usize, clusters: usize) -> Vec<Vec<f32>> {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 40) as f32 / (1u64 << 24) as f32 - 0.5
        };
        let centroids = (0..clusters)
            .map(|_| (0..dimensions).map(|_| next()).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        (0..count)
            .map(|index| {
                let centroid = &centroids[index % clusters];
                let mut vector = centroid
                    .iter()
                    .map(|value| value + next() * 0.15)
                    .collect::<Vec<_>>();
                let norm = vector
                    .iter()
                    .map(|v| v * v)
                    .sum::<f32>()
                    .sqrt()
                    .max(f32::EPSILON);
                vector.iter_mut().for_each(|v| *v /= norm);
                vector
            })
            .collect()
    }

    fn exact_top_keys(vectors: &[Vec<f32>], query: &[f32], count: usize) -> Vec<u64> {
        let mut scored = vectors
            .iter()
            .enumerate()
            .map(|(key, vector)| {
                let (dot, _) = scalar_dot_and_norm_squared(vector, query);
                (key as u64, dot)
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| right.1.total_cmp(&left.1));
        scored.into_iter().take(count).map(|(key, _)| key).collect()
    }

    fn ann_recall_at_10(tier: VectorTier) -> f64 {
        const COUNT: usize = 20_000;
        const DIMENSIONS: usize = 256;
        const QUERIES: usize = 50;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("store.usearch");
        let vectors = clustered_unit_vectors(COUNT + QUERIES, DIMENSIONS, 64);
        let (corpus, queries) = vectors.split_at(COUNT);
        {
            let mut store = VectorStore::open(&path, DIMENSIONS, ScalarKind::F16, tier).unwrap();
            store.reserve_additional(COUNT).unwrap();
            for (key, vector) in corpus.iter().enumerate() {
                store.add_unchecked(key as u64, vector.clone()).unwrap();
            }
            store.save().unwrap();
        }
        let store = VectorStore::open_readonly(&path, DIMENSIONS, ScalarKind::F16, tier).unwrap();
        let mut hits = 0usize;
        for query in queries {
            let expected = exact_top_keys(corpus, query, 10);
            let found = store.search(query, 10);
            hits += found
                .iter()
                .filter(|found| expected.contains(&found.key))
                .count();
        }
        hits as f64 / (QUERIES * 10) as f64
    }

    #[test]
    fn neural_tier_f16_256d_store_keeps_ann_recall() {
        // Regression: the neural store was silently built with the hash
        // tier's sparse graph whenever it matched the 256-d F16 shape,
        // dropping ANN recall@10 to roughly 0.15 on clustered vectors.
        let neural_recall = ann_recall_at_10(VectorTier::Neural);
        assert!(
            neural_recall >= 0.9,
            "neural tier recall@10 {neural_recall:.3} below 0.9"
        );
    }

    #[test]
    fn f16_neural_store_preserves_recall_and_reduces_size() {
        let tmp = tempfile::tempdir().unwrap();
        let f16_path = tmp.path().join("neural-f16.bin");
        let f32_path = tmp.path().join("neural-f32.bin");
        let vectors = (0..1_000u64)
            .map(|key| {
                let vector = (0..384)
                    .map(|dimension| {
                        let value = key.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(
                            (dimension as u64 + 1).wrapping_mul(1_442_695_040_888_963_407),
                        );
                        ((value >> 40) as f32 / (1u64 << 24) as f32) - 0.5
                    })
                    .collect::<Vec<_>>();
                (key, vector)
            })
            .collect::<Vec<_>>();

        for (path, quantization) in [(&f16_path, ScalarKind::F16), (&f32_path, ScalarKind::F32)] {
            let mut store = VectorStore::open(path, 384, quantization, VectorTier::Neural).unwrap();
            store.reserve_additional(vectors.len()).unwrap();
            for (key, vector) in &vectors {
                store.add_unchecked(*key, vector.clone()).unwrap();
            }
            store.save().unwrap();
        }

        let query = &vectors[427].1;
        let f16 = VectorStore::open_readonly(&f16_path, 384, ScalarKind::F16, VectorTier::Neural)
            .unwrap();
        let f32 = VectorStore::open_readonly(&f32_path, 384, ScalarKind::F32, VectorTier::Neural)
            .unwrap();
        let f16_keys = f16
            .search(query, 20)
            .into_iter()
            .map(|hit| hit.key)
            .collect::<std::collections::HashSet<_>>();
        let f32_keys = f32
            .search(query, 20)
            .into_iter()
            .map(|hit| hit.key)
            .collect::<std::collections::HashSet<_>>();
        let overlap = f16_keys.intersection(&f32_keys).count();

        assert!(f16_keys.contains(&427));
        assert!(f32_keys.contains(&427));
        assert!(overlap >= 18, "F16/F32 top-20 overlap was {overlap}/20");
        assert!(
            fs::metadata(&f16_path).unwrap().len() * 10
                < fs::metadata(&f32_path).unwrap().len() * 7,
            "F16 store should be materially smaller than F32"
        );
    }
}
