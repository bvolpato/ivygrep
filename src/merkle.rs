use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::chunking::is_indexable_path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MerkleSnapshot {
    pub root_hash: String,
    pub files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MerkleDiff {
    pub added_or_modified: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
}

impl MerkleSnapshot {
    pub fn empty() -> Self {
        Self {
            root_hash: String::new(),
            files: BTreeMap::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::empty());
        }
        let data = fs::read(path)?;
        let snapshot = serde_json::from_slice(&data)?;
        Ok(snapshot)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let payload = serde_json::to_vec_pretty(self)?;
        fs::write(path, payload)?;
        Ok(())
    }

    pub fn build(root: &Path) -> Result<Self> {
        let mut files = BTreeMap::new();

        let mut walker = WalkBuilder::new(root);
        walker.hidden(false);
        walker.git_ignore(true);
        walker.git_exclude(true);
        walker.git_global(true);
        walker.ignore(true);
        walker.follow_links(false);

        for entry in walker.build() {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }

            let rel = path
                .strip_prefix(root)
                .with_context(|| {
                    format!(
                        "failed to strip prefix {} from {}",
                        root.display(),
                        path.display()
                    )
                })?
                .to_path_buf();

            if !is_indexable_path(&rel) {
                continue;
            }

            let metadata = fs::metadata(path)?;
            if metadata.len() > 2 * 1024 * 1024 {
                continue;
            }

            let content =
                fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
            let mut hasher = Sha256::new();
            hasher.update(rel.to_string_lossy().as_bytes());
            hasher.update(&content);
            let file_hash = hex::encode(hasher.finalize());

            files.insert(rel.to_string_lossy().to_string(), file_hash);
        }

        let root_hash = root_hash(&files);
        Ok(Self { root_hash, files })
    }

    pub fn diff(&self, newer: &Self) -> MerkleDiff {
        if self.root_hash == newer.root_hash {
            return MerkleDiff::default();
        }

        let old_paths: BTreeSet<_> = self.files.keys().cloned().collect();
        let new_paths: BTreeSet<_> = newer.files.keys().cloned().collect();

        let mut added_or_modified = Vec::new();
        let mut deleted = Vec::new();

        for path in new_paths.iter() {
            match self.files.get(path) {
                Some(old_hash) => {
                    let new_hash = newer
                        .files
                        .get(path)
                        .expect("path exists in new set and map");
                    if old_hash != new_hash {
                        added_or_modified.push(PathBuf::from(path));
                    }
                }
                None => added_or_modified.push(PathBuf::from(path)),
            }
        }

        for path in old_paths.difference(&new_paths) {
            deleted.push(PathBuf::from(path));
        }

        MerkleDiff {
            added_or_modified,
            deleted,
        }
    }
}

fn root_hash(files: &BTreeMap<String, String>) -> String {
    let mut hasher = Sha256::new();
    for (path, hash) in files {
        hasher.update(path.as_bytes());
        hasher.update(hash.as_bytes());
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn merkle_diff_detects_add_modify_delete() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("a.rs"), "fn a() {}\n").unwrap();
        fs::write(root.join("b.py"), "def b():\n    pass\n").unwrap();

        let first = MerkleSnapshot::build(root).unwrap();

        fs::remove_file(root.join("a.rs")).unwrap();
        fs::write(root.join("b.py"), "def b():\n    return 1\n").unwrap();
        fs::write(root.join("c.ts"), "export function c() {}\n").unwrap();

        let second = MerkleSnapshot::build(root).unwrap();
        let diff = first.diff(&second);

        assert!(diff.deleted.contains(&PathBuf::from("a.rs")));
        assert!(diff.added_or_modified.contains(&PathBuf::from("b.py")));
        assert!(diff.added_or_modified.contains(&PathBuf::from("c.ts")));
    }

    #[test]
    fn snapshot_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("snapshot.json");

        let mut map = BTreeMap::new();
        map.insert("a.rs".to_string(), "hash1".to_string());

        let snapshot = MerkleSnapshot {
            root_hash: "root".to_string(),
            files: map,
        };

        snapshot.save(&path).unwrap();
        let loaded = MerkleSnapshot::load(&path).unwrap();
        assert_eq!(snapshot, loaded);

        let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"\n").unwrap();
    }
}
