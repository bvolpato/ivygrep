use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::{ScalarKind, VectorMatch, top_vector_matches};

const MAGIC: &[u8; 8] = b"IVYVEC01";
const BACKUP_EXTENSION: &str = "usearch.bak";

pub struct VectorStore {
    path: PathBuf,
    dimensions: usize,
    quantization: ScalarKind,
    vectors: BTreeMap<u64, Vec<f32>>,
}

impl VectorStore {
    pub fn open(path: &Path, dimensions: usize, quantization: ScalarKind) -> Result<Self> {
        Self::load(path, dimensions, quantization)
    }

    pub fn open_readonly(path: &Path, dimensions: usize, quantization: ScalarKind) -> Result<Self> {
        Self::load(path, dimensions, quantization)
    }

    fn load(path: &Path, dimensions: usize, quantization: ScalarKind) -> Result<Self> {
        let mut store = Self {
            path: path.to_path_buf(),
            dimensions,
            quantization,
            vectors: BTreeMap::new(),
        };
        let load_path = if path.exists() {
            path.to_path_buf()
        } else {
            let backup = path.with_extension(BACKUP_EXTENSION);
            if !backup.exists() {
                return Ok(store);
            }
            backup
        };
        if !load_path.exists() {
            return Ok(store);
        }

        let mut file = fs::File::open(load_path)?;
        let mut magic = [0u8; 8];
        file.read_exact(&mut magic)?;
        anyhow::ensure!(magic == *MAGIC, "invalid portable vector store");

        let stored_dimensions = read_u32(&mut file)? as usize;
        anyhow::ensure!(
            stored_dimensions == dimensions,
            "vector dimensions mismatch ({stored_dimensions} != {dimensions})"
        );
        let mut scalar = [0u8; 1];
        file.read_exact(&mut scalar)?;
        let stored_quantization = match scalar[0] {
            1 => ScalarKind::F16,
            2 => ScalarKind::F32,
            other => anyhow::bail!("unsupported portable quantization {other}"),
        };
        anyhow::ensure!(
            stored_quantization == quantization,
            "vector quantization mismatch"
        );

        let count = read_u64(&mut file)?;
        let bytes_per_scalar = match quantization {
            ScalarKind::F16 => 2u64,
            ScalarKind::F32 => 4u64,
        };
        let bytes_per_vector = (dimensions as u64)
            .checked_mul(bytes_per_scalar)
            .and_then(|bytes| bytes.checked_add(8))
            .context("portable vector dimensions overflow")?;
        let expected_len = 8u64
            .checked_add(4)
            .and_then(|bytes| bytes.checked_add(1))
            .and_then(|bytes| bytes.checked_add(8))
            .and_then(|bytes| bytes.checked_add(count.checked_mul(bytes_per_vector)?))
            .context("portable vector store length overflow")?;
        anyhow::ensure!(
            file.metadata()?.len() == expected_len,
            "portable vector store length mismatch"
        );
        for _ in 0..count {
            let key = read_u64(&mut file)?;
            let mut vector = Vec::with_capacity(dimensions);
            match quantization {
                ScalarKind::F32 => {
                    for _ in 0..dimensions {
                        let mut value = [0u8; 4];
                        file.read_exact(&mut value)?;
                        vector.push(f32::from_le_bytes(value));
                    }
                }
                ScalarKind::F16 => {
                    for _ in 0..dimensions {
                        let mut value = [0u8; 2];
                        file.read_exact(&mut value)?;
                        vector.push(f16_to_f32(u16::from_le_bytes(value)));
                    }
                }
            }
            anyhow::ensure!(
                store.vectors.insert(key, vector).is_none(),
                "duplicate portable vector key {key}"
            );
        }
        Ok(store)
    }

    pub fn save(&self) -> Result<()> {
        let parent = self
            .path
            .parent()
            .context("vector store path has no parent")?;
        fs::create_dir_all(parent)?;
        let tmp_path = self.path.with_extension("usearch.tmp");
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(MAGIC)?;
        file.write_all(&(self.dimensions as u32).to_le_bytes())?;
        file.write_all(&[match self.quantization {
            ScalarKind::F16 => 1,
            ScalarKind::F32 => 2,
        }])?;
        file.write_all(&(self.vectors.len() as u64).to_le_bytes())?;
        for (key, vector) in &self.vectors {
            file.write_all(&key.to_le_bytes())?;
            match self.quantization {
                ScalarKind::F32 => {
                    for value in vector {
                        file.write_all(&value.to_le_bytes())?;
                    }
                }
                ScalarKind::F16 => {
                    for value in vector {
                        file.write_all(&f32_to_f16(*value).to_le_bytes())?;
                    }
                }
            }
        }
        file.sync_all()?;
        replace_file(&tmp_path, &self.path)?;
        Ok(())
    }

    pub fn contains(&self, key: u64) -> bool {
        self.vectors.contains_key(&key)
    }

    pub fn remove(&mut self, key: u64) {
        self.vectors.remove(&key);
    }

    pub fn reserve_additional(&mut self, _additional: usize) -> Result<()> {
        Ok(())
    }

    pub fn add_unchecked(&mut self, key: u64, vector: Vec<f32>) -> Result<()> {
        self.validate_vector(&vector)?;
        anyhow::ensure!(!self.vectors.contains_key(&key), "duplicate vector key");
        self.vectors.insert(key, vector);
        Ok(())
    }

    pub fn upsert(&mut self, key: u64, vector: Vec<f32>) -> Result<()> {
        self.validate_vector(&vector)?;
        self.vectors.insert(key, vector);
        Ok(())
    }

    pub fn size(&self) -> usize {
        self.vectors.len()
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    pub fn search(&self, query: &[f32], count: usize) -> Vec<VectorMatch> {
        if query.len() != self.dimensions {
            return Vec::new();
        }
        let mut matches = self
            .vectors
            .iter()
            .map(|(key, vector)| VectorMatch {
                key: *key,
                score: cosine_similarity(vector, query),
            })
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.key.cmp(&right.key))
        });
        matches.truncate(count);
        matches
    }

    pub fn score(&self, key: u64, query: &[f32]) -> Option<f32> {
        let vector = self.vectors.get(&key)?;
        (query.len() == self.dimensions).then(|| cosine_similarity(vector, query))
    }

    pub fn score_many_top_k(&self, keys: &[u64], query: &[f32], count: usize) -> Vec<VectorMatch> {
        if count == 0 || query.len() != self.dimensions {
            return Vec::new();
        }

        let query_norm = query.iter().map(|value| value * value).sum::<f32>().sqrt();
        let matches = keys.iter().filter_map(|key| {
            let vector = self.vectors.get(key)?;
            let dot = vector
                .iter()
                .zip(query)
                .map(|(left, right)| left * right)
                .sum::<f32>();
            let vector_norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
            let score = if vector_norm > 0.0 && query_norm > 0.0 {
                dot / (vector_norm * query_norm)
            } else {
                0.0
            };
            Some(VectorMatch { key: *key, score })
        });
        top_vector_matches(matches, count)
    }

    fn validate_vector(&self, vector: &[f32]) -> Result<()> {
        anyhow::ensure!(
            vector.len() == self.dimensions,
            "vector dimensions mismatch ({} != {})",
            vector.len(),
            self.dimensions
        );
        Ok(())
    }
}

fn read_u32(reader: &mut impl Read) -> Result<u32> {
    let mut bytes = [0u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read) -> Result<u64> {
    let mut bytes = [0u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let dot = left
        .iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm > 0.0 && right_norm > 0.0 {
        dot / (left_norm * right_norm)
    } else {
        0.0
    }
}

fn f32_to_f16(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let mantissa = bits & 0x7fffff;

    if exponent == 0xff {
        return sign | 0x7c00 | ((mantissa != 0) as u16);
    }

    let half_exponent = exponent - 127 + 15;
    if half_exponent >= 31 {
        return sign | 0x7c00;
    }
    if half_exponent <= 0 {
        if half_exponent < -10 {
            return sign;
        }
        let mantissa = mantissa | 0x800000;
        let shift = (14 - half_exponent) as u32;
        let rounded = (mantissa + (1 << (shift - 1))) >> shift;
        return sign | rounded as u16;
    }

    let rounded_mantissa = mantissa + 0x1000;
    if rounded_mantissa & 0x800000 != 0 {
        let exponent = half_exponent + 1;
        if exponent >= 31 {
            return sign | 0x7c00;
        }
        return sign | ((exponent as u16) << 10);
    }
    sign | ((half_exponent as u16) << 10) | ((rounded_mantissa >> 13) as u16)
}

fn f16_to_f32(value: u16) -> f32 {
    let sign = ((value & 0x8000) as u32) << 16;
    let exponent = ((value >> 10) & 0x1f) as u32;
    let mantissa = (value & 0x03ff) as u32;
    let bits = match exponent {
        0 if mantissa == 0 => sign,
        0 => {
            let leading = mantissa.leading_zeros() - 22;
            let normalized = (mantissa << (leading + 1)) & 0x03ff;
            sign | ((112 - leading) << 23) | (normalized << 13)
        }
        31 => sign | 0x7f800000 | (mantissa << 13),
        _ => sign | ((exponent + 112) << 23) | (mantissa << 13),
    };
    f32::from_bits(bits)
}

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
    fn persists_and_searches_portable_vectors() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("vectors.usearch");
        let mut store = VectorStore::open(&path, 4, ScalarKind::F16).unwrap();
        store.upsert(7, vec![1.0, 0.0, 0.0, 0.0]).unwrap();
        store.upsert(9, vec![0.0, 1.0, 0.0, 0.0]).unwrap();
        store.save().unwrap();

        let reopened = VectorStore::open_readonly(&path, 4, ScalarKind::F16).unwrap();
        assert_eq!(reopened.size(), 2);
        assert_eq!(reopened.search(&[0.9, 0.1, 0.0, 0.0], 1)[0].key, 7);
        assert!(reopened.score(7, &[1.0, 0.0, 0.0, 0.0]).unwrap() > 0.99);
        assert_eq!(
            reopened.score_many_top_k(&[9, 7], &[1.0, 0.0, 0.0, 0.0], 1)[0].key,
            7
        );
    }

    #[test]
    fn f16_storage_is_smaller_than_f32() {
        let temp = tempfile::tempdir().unwrap();
        let f16_path = temp.path().join("f16.usearch");
        let f32_path = temp.path().join("f32.usearch");
        for (path, quantization) in [(&f16_path, ScalarKind::F16), (&f32_path, ScalarKind::F32)] {
            let mut store = VectorStore::open(path, 384, quantization).unwrap();
            for key in 0..32 {
                store
                    .add_unchecked(
                        key,
                        (0..384)
                            .map(|dimension| (key + dimension) as f32 / 1000.0)
                            .collect(),
                    )
                    .unwrap();
            }
            store.save().unwrap();
        }

        let f16_size = fs::metadata(f16_path).unwrap().len();
        let f32_size = fs::metadata(f32_path).unwrap().len();
        assert!(f16_size < f32_size * 60 / 100);
    }

    #[test]
    fn recovers_interrupted_replacement_from_backup() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("vectors.usearch");
        let backup = path.with_extension(BACKUP_EXTENSION);
        let mut store = VectorStore::open(&path, 2, ScalarKind::F16).unwrap();
        store.upsert(1, vec![1.0, 0.0]).unwrap();
        store.save().unwrap();
        fs::rename(&path, &backup).unwrap();

        let reopened = VectorStore::open_readonly(&path, 2, ScalarKind::F16).unwrap();
        assert!(reopened.contains(1));
    }

    #[test]
    fn recovered_backup_survives_until_the_next_save_completes() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("vectors.usearch");
        let backup = path.with_extension(BACKUP_EXTENSION);
        let mut store = VectorStore::open(&path, 2, ScalarKind::F16).unwrap();
        store.upsert(1, vec![1.0, 0.0]).unwrap();
        store.save().unwrap();
        fs::rename(&path, &backup).unwrap();

        let mut recovered = VectorStore::open(&path, 2, ScalarKind::F16).unwrap();
        recovered.upsert(2, vec![0.0, 1.0]).unwrap();
        recovered.save().unwrap();

        let reopened = VectorStore::open_readonly(&path, 2, ScalarKind::F16).unwrap();
        assert!(reopened.contains(1));
        assert!(reopened.contains(2));
        assert!(!backup.exists());
    }

    #[test]
    fn half_conversion_handles_subnormal_and_special_values() {
        for value in [
            0.0,
            -0.0,
            1.0,
            -2.0,
            0.000_061_035_156,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ] {
            let round_trip = f16_to_f32(f32_to_f16(value));
            if value.is_infinite() {
                assert_eq!(round_trip, value);
            } else {
                assert!((round_trip - value).abs() <= value.abs().max(1.0) * 0.001);
            }
        }
        assert!(f16_to_f32(f32_to_f16(f32::NAN)).is_nan());
    }
}
