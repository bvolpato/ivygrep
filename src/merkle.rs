use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::IsTerminal;

const MAX_INDEXABLE_FILE_BYTES: u64 = 16 * 1024 * 1024;

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
        let walker = crate::walker::source_walker(root);

        let show_progress = std::io::stderr().is_terminal();
        let mut scanned = 0;

        for entry in walker.build() {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }

            scanned += 1;
            if show_progress && scanned % 500 == 0 {
                use std::io::Write;
                eprint!("\r\x1b[K  scanning files... {}", scanned);
                let _ = std::io::stderr().flush();
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

            let metadata = fs::metadata(path)?;
            if metadata.len() > MAX_INDEXABLE_FILE_BYTES {
                continue;
            }

            // Pseudo-hash based on filesystem metadata (size + mtime) instead of reading
            // the entire file contents. This makes the diffing traversal extremely fast,
            // bypassing massive multi-gigabyte I/O overheads on monoliths like dd-source.
            let mut hasher = Sha256::new();
            hasher.update(rel.to_string_lossy().as_bytes());
            hasher.update(metadata.len().to_le_bytes());

            if let Ok(mtime) = metadata.modified() {
                if let Ok(duration) = mtime.duration_since(std::time::UNIX_EPOCH) {
                    hasher.update(duration.as_nanos().to_le_bytes());
                }
            }

            let file_hash = hex::encode(hasher.finalize());

            files.insert(rel.to_string_lossy().to_string(), file_hash);
        }

        if show_progress {
            eprint!("\r\x1b[K"); // clean up progress line
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

    #[test]
    fn medium_txt_files_are_included_in_snapshot() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let content = "lorem ipsum dolor sit amet\n".repeat(160_000);

        fs::write(root.join("shakespeare.txt"), content).unwrap();

        let snapshot = MerkleSnapshot::build(root).unwrap();
        assert!(snapshot.files.contains_key("shakespeare.txt"));
    }

    #[test]
    fn unknown_text_files_are_indexed_but_binary_are_skipped() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::write(
            root.join("notes.custom"),
            "plain text in custom extension\n",
        )
        .unwrap();
        fs::write(root.join("blob.custom"), b"\x89PNG\r\n\x1a\n\0\0\0IHDR").unwrap();

        let snapshot = MerkleSnapshot::build(root).unwrap();
        assert!(snapshot.files.contains_key("notes.custom"));
        assert!(!snapshot.files.contains_key("blob.custom"));
    }

    #[test]
    fn dot_git_directory_is_excluded_but_other_hidden_files_are_included() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Simulate a .git directory with objects
        fs::create_dir_all(root.join(".git/objects")).unwrap();
        fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(
            root.join(".git/objects/pack.idx"),
            "fake pack index content\n",
        )
        .unwrap();

        // Regular hidden files that SHOULD be indexed
        fs::write(root.join(".env"), "DATABASE_URL=postgres://localhost\n").unwrap();
        fs::write(root.join(".eslintrc.json"), "{}\n").unwrap();

        // Normal source file
        fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();

        let snapshot = MerkleSnapshot::build(root).unwrap();

        // .git contents must be excluded
        assert!(!snapshot.files.contains_key(".git/HEAD"));
        assert!(!snapshot.files.contains_key(".git/objects/pack.idx"));
        assert!(
            snapshot.files.keys().all(|k| !k.starts_with(".git/")),
            "no file under .git/ should be indexed"
        );

        // Other hidden files and normal files must be included
        assert!(snapshot.files.contains_key(".env"));
        assert!(snapshot.files.contains_key(".eslintrc.json"));
        assert!(snapshot.files.contains_key("main.rs"));
    }
}
