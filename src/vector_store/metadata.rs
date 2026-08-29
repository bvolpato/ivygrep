//! Bounded-memory structural reads for routing and status, not deep integrity checks.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result, ensure};

// The pinned USearch serializer writes a u32 matrix shape, a dense header,
// an index_serialized_header_t, then i16 node levels. Header fields and node
// sizes are interpreted by the vendored native definitions, not duplicated here.
const DENSE_HEADER_BYTES: usize = 64;
const GRAPH_HEADER_BYTES: usize = 40;
const LEVEL_BYTES: usize = 2;

pub(super) fn read_count(path: &Path, dimensions: usize) -> Result<u64> {
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    let mut shape = [0u8; 8];
    file.read_exact(&mut shape)
        .context("vector store is truncated before matrix dimensions")?;
    let rows = u32::from_le_bytes(shape[..4].try_into().unwrap()) as u64;
    let vector_bytes = u32::from_le_bytes(shape[4..].try_into().unwrap()) as u64;
    let dense_offset = rows
        .checked_mul(vector_bytes)
        .and_then(|bytes| bytes.checked_add(shape.len() as u64))
        .context("vector store matrix size overflow")?;
    let levels_offset = dense_offset
        .checked_add((DENSE_HEADER_BYTES + GRAPH_HEADER_BYTES) as u64)
        .context("vector store header offset overflow")?;
    ensure!(
        levels_offset <= file_len,
        "vector store is truncated before graph header"
    );

    file.seek(SeekFrom::Start(dense_offset))?;
    let mut dense = [0u8; DENSE_HEADER_BYTES];
    let mut graph = [0u8; GRAPH_HEADER_BYTES];
    file.read_exact(&mut dense)?;
    file.read_exact(&mut graph)?;
    let metadata = usearch::inspect_serialized_header(&dense, &graph)?;
    ensure!(
        metadata.dimensions == dimensions as u64,
        "vector dimensions mismatch ({} != {dimensions})",
        metadata.dimensions
    );
    ensure!(
        metadata.dimensions.checked_mul(metadata.scalar_bytes) == Some(vector_bytes),
        "vector store matrix width does not match its scalar type and dimensions"
    );
    ensure!(
        metadata.count_present.checked_add(metadata.count_deleted) == Some(rows)
            && metadata.graph_size == rows,
        "vector store population does not match matrix and graph sizes"
    );
    if rows > 0 {
        ensure!(
            metadata.max_level <= i16::MAX as u64 && metadata.entry_slot < rows,
            "invalid vector store graph entry or maximum level"
        );
    }

    let base_bytes = metadata
        .node_base_bytes
        .checked_add(LEVEL_BYTES as u64)
        .and_then(|bytes| bytes.checked_mul(rows))
        .and_then(|bytes| bytes.checked_add(levels_offset))
        .context("vector store graph size overflow")?;
    ensure!(
        base_bytes <= file_len,
        "vector store is truncated before graph nodes"
    );

    // Graph length depends on the levels. Stream their compact table rather
    // than allocating a native node pointer array and a key-to-slot hash table.
    let mut remaining = rows;
    let mut total_levels = 0u64;
    let mut buffer = [0u8; 8_192];
    while remaining > 0 {
        let count = remaining.min((buffer.len() / LEVEL_BYTES) as u64) as usize;
        file.read_exact(&mut buffer[..count * LEVEL_BYTES])?;
        for encoded in buffer[..count * LEVEL_BYTES].as_chunks::<LEVEL_BYTES>().0 {
            let level = i16::from_le_bytes(*encoded);
            ensure!(
                level >= 0 && level as u64 <= metadata.max_level,
                "invalid vector store node level"
            );
            total_levels = total_levels
                .checked_add(level as u64)
                .context("vector store level count overflow")?;
        }
        remaining -= count as u64;
    }
    let required_len = total_levels
        .checked_mul(metadata.node_level_bytes)
        .and_then(|bytes| bytes.checked_add(base_bytes))
        .context("vector store graph length overflow")?;
    ensure!(
        required_len <= file_len,
        "vector store is truncated in graph nodes"
    );
    Ok(metadata.count_present)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector_store::{ScalarKind, VectorStore, VectorTier};
    use std::fs;

    fn saved_store(path: &Path, scalar: ScalarKind, tier: VectorTier) -> VectorStore {
        let mut store = VectorStore::open(path, 4, scalar, tier).unwrap();
        for (key, vector) in [
            (1, vec![1.0, 0.0, 0.0, 0.0]),
            (2, vec![0.0, 1.0, 0.0, 0.0]),
            (3, vec![0.0, 0.0, 1.0, 0.0]),
        ] {
            store.add_unchecked(key, vector).unwrap();
        }
        store.save().unwrap();
        store
    }

    fn dense_offset(bytes: &[u8]) -> usize {
        8 + u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize
            * u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize
    }

    #[test]
    fn persisted_count_matches_native_for_both_tiers_and_scalar_widths() {
        for scalar in [ScalarKind::F16, ScalarKind::F32] {
            for tier in [VectorTier::Hash, VectorTier::Neural] {
                let temp = tempfile::tempdir().unwrap();
                let path = temp.path().join("vectors.usearch");
                let mut store = saved_store(&path, scalar, tier);
                for expected in [3, 2, 1, 0] {
                    store.save().unwrap();
                    let native =
                        VectorStore::open_readonly(&path, 4, ScalarKind::F16, tier).unwrap();
                    assert_eq!(native.size(), expected);
                    assert_eq!(VectorStore::read_count(&path, 4).unwrap(), expected as u64);
                    store.remove(expected as u64);
                }
                store.upsert(8, vec![0.0, 1.0, 0.0, 0.0]).unwrap();
                store.save().unwrap();
                assert_eq!(VectorStore::read_count(&path, 4).unwrap(), 1);
            }
        }
    }

    #[test]
    fn missing_empty_and_unicode_paths_preserve_count_semantics() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("vecteurs-\u{00e9}\u{6d4b}.usearch");
        assert_eq!(VectorStore::read_count(&path, 4).unwrap(), 0);
        VectorStore::reset(&path, 4, ScalarKind::F16, VectorTier::Hash).unwrap();
        assert_eq!(VectorStore::read_count(&path, 4).unwrap(), 0);
        saved_store(&path, ScalarKind::F16, VectorTier::Hash);
        assert_eq!(VectorStore::read_count(&path, 4).unwrap(), 3);
        assert!(VectorStore::read_count(&path, 8).is_err());
    }

    #[test]
    fn persisted_count_streams_more_than_one_level_buffer() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("vectors.usearch");
        let mut store = VectorStore::open(&path, 4, ScalarKind::F16, VectorTier::Hash).unwrap();
        for key in 0..5_000 {
            store
                .add_unchecked(key, vec![key as f32, 1.0, 0.0, 0.0])
                .unwrap();
        }
        store.remove(17);
        store.save().unwrap();
        assert_eq!(VectorStore::read_count(&path, 4).unwrap(), 4_999);
    }

    #[test]
    fn rejects_truncated_matrix_headers_levels_and_graph_tail() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("vectors.usearch");
        saved_store(&path, ScalarKind::F16, VectorTier::Neural);
        let original = fs::read(&path).unwrap();
        let dense = dense_offset(&original);
        let levels = dense + DENSE_HEADER_BYTES + GRAPH_HEADER_BYTES;
        for length in [
            0,
            7,
            dense - 1,
            dense + 63,
            levels - 1,
            levels + 1,
            original.len() - 1,
        ] {
            fs::write(&path, &original[..length]).unwrap();
            assert!(
                VectorStore::read_count(&path, 4).is_err(),
                "accepted {length} bytes"
            );
        }
        // The fixed minimum length is insufficient: an upper-level node
        // needs more payload bytes even when all fixed headers fit.
        let mut missing_upper_level = original;
        let first_level =
            i16::from_le_bytes(missing_upper_level[levels..levels + 2].try_into().unwrap());
        let graph_max_offset = dense + DENSE_HEADER_BYTES + 24;
        let graph_max = u64::from_le_bytes(
            missing_upper_level[graph_max_offset..graph_max_offset + 8]
                .try_into()
                .unwrap(),
        );
        missing_upper_level[levels..levels + 2].copy_from_slice(&(first_level + 1).to_le_bytes());
        missing_upper_level[graph_max_offset..graph_max_offset + 8]
            .copy_from_slice(&graph_max.max(first_level as u64 + 1).to_le_bytes());
        fs::write(&path, missing_upper_level).unwrap();
        assert!(
            VectorStore::read_count(&path, 4)
                .unwrap_err()
                .to_string()
                .contains("truncated in graph nodes")
        );
    }

    #[test]
    fn rejects_invalid_header_types_population_and_node_levels() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("vectors.usearch");
        saved_store(&path, ScalarKind::F32, VectorTier::Neural);
        let original = fs::read(&path).unwrap();
        let dense = dense_offset(&original);
        let graph = dense + DENSE_HEADER_BYTES;
        let levels = graph + GRAPH_HEADER_BYTES;
        // Corrupt actual native output at each serialized field boundary.
        for (offset, payload) in [
            (dense, vec![b'x']),
            (dense + 7, 99u16.to_le_bytes().to_vec()),
            (dense + 13, vec![b'e']),
            (dense + 14, vec![23]),
            (dense + 15, vec![15]),
            (dense + 16, vec![14]),
            (dense + 17, u64::MAX.to_le_bytes().to_vec()),
            (dense + 41, vec![1]),
            (4, 8u32.to_le_bytes().to_vec()),
            (graph, 4u64.to_le_bytes().to_vec()),
            (graph + 8, 1u64.to_le_bytes().to_vec()),
            (graph + 16, u64::MAX.to_le_bytes().to_vec()),
            (graph + 32, 3u64.to_le_bytes().to_vec()),
            (levels, (-1i16).to_le_bytes().to_vec()),
        ] {
            let mut corrupted = original.clone();
            corrupted[offset..offset + payload.len()].copy_from_slice(&payload);
            fs::write(&path, corrupted).unwrap();
            assert!(
                VectorStore::read_count(&path, 4).is_err(),
                "accepted corruption at {offset}"
            );
        }
    }

    #[test]
    fn legacy_f32_scalar_codes_use_native_header_conversion() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("vectors.usearch");
        saved_store(&path, ScalarKind::F32, VectorTier::Neural);
        let mut bytes = fs::read(&path).unwrap();
        let dense = dense_offset(&bytes);
        bytes[dense + 9..dense + 11].copy_from_slice(&9u16.to_le_bytes());
        bytes[dense + 14] = 5; // pre-2.10 F32
        bytes[dense + 15] = 8; // pre-2.10 u64 key
        bytes[dense + 16] = 9; // pre-2.10 u32 slot
        fs::write(&path, bytes).unwrap();
        assert_eq!(VectorStore::read_count(&path, 4).unwrap(), 3);
        let native =
            VectorStore::open_readonly(&path, 4, ScalarKind::F16, VectorTier::Neural).unwrap();
        assert_eq!(native.size(), 3);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn persisted_count_uses_interrupted_save_backup() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("vectors.usearch");
        saved_store(&path, ScalarKind::F16, VectorTier::Neural);
        fs::rename(&path, path.with_extension("usearch.bak")).unwrap();
        assert_eq!(VectorStore::read_count(&path, 4).unwrap(), 3);
    }
}
