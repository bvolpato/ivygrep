use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

#[derive(Debug, Clone)]
pub struct VectorMatch {
    pub key: u64,
    pub score: f32,
}

pub struct VectorStore {
    path: PathBuf,
    index: Index,
}

impl VectorStore {
    pub fn open(path: &Path, dimensions: usize) -> Result<Self> {
        let options = IndexOptions {
            dimensions,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            ..IndexOptions::default()
        };

        let index = Index::new(&options)?;
        if path.exists() {
            let path_str = path
                .to_str()
                .context("vector path contains invalid UTF-8")?;
            index.load(path_str)?;
        }

        Ok(Self {
            path: path.to_path_buf(),
            index,
        })
    }

    pub fn save(&self) -> Result<()> {
        let parent = self
            .path
            .parent()
            .context("vector store path has no parent")?;
        fs::create_dir_all(parent)?;

        let tmp_path = self.path.with_extension("usearch.tmp");
        let path_str = tmp_path
            .to_str()
            .context("vector path contains invalid UTF-8")?;

        // Write to temporary file first
        if let Err(e) = self.index.save(path_str) {
            let _ = fs::remove_file(&tmp_path);
            return Err(e.into());
        }

        // Atomically rename to avoid corrupted reads by concurrent search processes
        fs::rename(&tmp_path, &self.path)?;

        Ok(())
    }

    pub fn contains(&self, key: u64) -> bool {
        self.index.contains(key)
    }

    pub fn remove(&mut self, key: u64) {
        let _ = self.index.remove(key);
    }

    pub fn upsert(&mut self, key: u64, vector: Vec<f32>) {
        self.ensure_capacity_for_insert();

        if self.index.contains(key) {
            let _ = self.index.remove(key);
        }
        let _ = self.index.add(key, &vector);
    }

    pub fn size(&self) -> usize {
        self.index.size()
    }

    pub fn search(&self, query: &[f32], count: usize) -> Vec<VectorMatch> {
        match self.index.search(query, count) {
            Ok(matches) => matches
                .keys
                .iter()
                .zip(matches.distances.iter())
                .map(|(key, distance)| VectorMatch {
                    key: *key,
                    score: -distance,
                })
                .collect(),
            Err(_) => vec![],
        }
    }

    /// Score a single vector by key against a query vector.
    /// Returns None if the key doesn't exist in the index.
    pub fn score(&self, key: u64, query: &[f32]) -> Option<f32> {
        if !self.index.contains(key) {
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

    fn ensure_capacity_for_insert(&mut self) {
        let size = self.index.size();
        let capacity = self.index.capacity();

        if size < capacity {
            return;
        }

        let next_capacity = match capacity {
            0 => 1024,
            n => n.saturating_mul(2),
        };

        let _ = self.index.reserve(next_capacity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_store_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vectors.bin");

        let mut store = VectorStore::open(&path, 4).unwrap();
        store.upsert(1, vec![1.0, 0.0, 0.0, 0.0]);
        store.upsert(2, vec![0.0, 1.0, 0.0, 0.0]);
        store.save().unwrap();

        let store = VectorStore::open(&path, 4).unwrap();
        let hits = store.search(&[1.0, 0.0, 0.0, 0.0], 2);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].key, 1);
    }
}
