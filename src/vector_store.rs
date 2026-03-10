use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[cfg(not(feature = "usearch-native"))]
use serde::{Deserialize, Serialize};
#[cfg(not(feature = "usearch-native"))]
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct VectorMatch {
    pub key: u64,
    pub score: f32,
}

#[cfg(not(feature = "usearch-native"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiskVectorStore {
    dimensions: usize,
    vectors: BTreeMap<u64, Vec<f32>>,
}

#[cfg(not(feature = "usearch-native"))]
#[derive(Debug, Clone)]
pub struct VectorStore {
    path: PathBuf,
    dimensions: usize,
    vectors: BTreeMap<u64, Vec<f32>>,
}

#[cfg(not(feature = "usearch-native"))]
impl VectorStore {
    pub fn open(path: &Path, dimensions: usize) -> Result<Self> {
        if path.exists() {
            let payload = fs::read(path)?;
            let disk: DiskVectorStore = serde_json::from_slice(&payload)?;
            if disk.dimensions != dimensions {
                return Ok(Self {
                    path: path.to_path_buf(),
                    dimensions,
                    vectors: BTreeMap::new(),
                });
            }

            Ok(Self {
                path: path.to_path_buf(),
                dimensions,
                vectors: disk.vectors,
            })
        } else {
            Ok(Self {
                path: path.to_path_buf(),
                dimensions,
                vectors: BTreeMap::new(),
            })
        }
    }

    pub fn save(&self) -> Result<()> {
        let parent = self
            .path
            .parent()
            .context("vector store path has no parent")?;
        fs::create_dir_all(parent)?;

        let payload = serde_json::to_vec(&DiskVectorStore {
            dimensions: self.dimensions,
            vectors: self.vectors.clone(),
        })?;
        fs::write(&self.path, payload)?;
        Ok(())
    }

    pub fn contains(&self, key: u64) -> bool {
        self.vectors.contains_key(&key)
    }

    pub fn remove(&mut self, key: u64) {
        self.vectors.remove(&key);
    }

    pub fn upsert(&mut self, key: u64, vector: Vec<f32>) {
        if vector.len() == self.dimensions {
            self.vectors.insert(key, vector);
        }
    }

    pub fn size(&self) -> usize {
        self.vectors.len()
    }

    pub fn search(&self, query: &[f32], count: usize) -> Vec<VectorMatch> {
        if query.len() != self.dimensions {
            return vec![];
        }

        let mut scores = self
            .vectors
            .iter()
            .map(|(key, vector)| VectorMatch {
                key: *key,
                score: cosine_similarity(query, vector),
            })
            .collect::<Vec<_>>();

        scores.sort_by(|a, b| b.score.total_cmp(&a.score));
        scores.truncate(count);
        scores
    }
}

#[cfg(not(feature = "usearch-native"))]
fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut left_norm = 0.0f32;
    let mut right_norm = 0.0f32;

    for (l, r) in left.iter().zip(right.iter()) {
        dot += l * r;
        left_norm += l * l;
        right_norm += r * r;
    }

    if left_norm == 0.0 || right_norm == 0.0 {
        return 0.0;
    }

    dot / (left_norm.sqrt() * right_norm.sqrt())
}

#[cfg(feature = "usearch-native")]
pub struct VectorStore {
    path: PathBuf,
    index: usearch::Index,
}

#[cfg(feature = "usearch-native")]
impl VectorStore {
    pub fn open(path: &Path, dimensions: usize) -> Result<Self> {
        use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

        let mut options = IndexOptions::default();
        options.dimensions = dimensions;
        options.metric = MetricKind::Cos;
        options.quantization = ScalarKind::F32;

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

        let path_str = self
            .path
            .to_str()
            .context("vector path contains invalid UTF-8")?;
        self.index.save(path_str)?;
        Ok(())
    }

    pub fn contains(&self, key: u64) -> bool {
        self.index.contains(key)
    }

    pub fn remove(&mut self, key: u64) {
        let _ = self.index.remove(key);
    }

    pub fn upsert(&mut self, key: u64, vector: Vec<f32>) {
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
