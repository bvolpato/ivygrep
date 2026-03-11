use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub root: PathBuf,
    pub index_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMetadata {
    pub id: String,
    pub root: PathBuf,
    pub created_at_unix: u64,
    pub last_indexed_at_unix: Option<u64>,
    pub watch_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStatus {
    pub id: String,
    pub root: PathBuf,
    pub last_indexed_at_unix: Option<u64>,
    pub watch_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct WorkspaceScope {
    pub rel_path: PathBuf,
    pub is_file: bool,
}

impl WorkspaceScope {
    pub fn matches(&self, rel_path: &Path) -> bool {
        if self.is_file {
            rel_path == self.rel_path
        } else {
            rel_path.starts_with(&self.rel_path)
        }
    }
}

impl Workspace {
    pub fn resolve(path: &Path) -> Result<Self> {
        let root = detect_workspace_root(path)?;
        let id = workspace_id(&root);
        let index_dir = config::indexes_root()?.join(&id);

        Ok(Self {
            id,
            root,
            index_dir,
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.index_dir)?;
        Ok(())
    }

    pub fn metadata_path(&self) -> PathBuf {
        self.index_dir.join("workspace.json")
    }

    pub fn sqlite_path(&self) -> PathBuf {
        self.index_dir.join("metadata.sqlite3")
    }

    pub fn tantivy_dir(&self) -> PathBuf {
        self.index_dir.join("tantivy")
    }

    pub fn vector_path(&self) -> PathBuf {
        self.index_dir.join("vectors.usearch")
    }

    pub fn merkle_snapshot_path(&self) -> PathBuf {
        self.index_dir.join("merkle_snapshot.json")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.index_dir.join("index.lock")
    }

    pub fn write_metadata(&self, metadata: &WorkspaceMetadata) -> Result<()> {
        let data = serde_json::to_vec_pretty(metadata)?;
        fs::write(self.metadata_path(), data)?;
        Ok(())
    }

    pub fn read_metadata(&self) -> Result<Option<WorkspaceMetadata>> {
        let path = self.metadata_path();
        if !path.exists() {
            return Ok(None);
        }
        let data = fs::read(path)?;
        let parsed = serde_json::from_slice(&data)?;
        Ok(Some(parsed))
    }

    pub fn exists(&self) -> bool {
        self.index_dir.exists()
    }
}

pub fn detect_workspace_root(path: &Path) -> Result<PathBuf> {
    let mut current = config::canonicalize_lossy(path)?;

    if current.is_file() {
        current = current
            .parent()
            .map(Path::to_path_buf)
            .context("file has no parent directory")?;
    }

    let mut cursor = current.clone();
    loop {
        if cursor.join(".git").exists() {
            return Ok(cursor);
        }

        if !cursor.pop() {
            break;
        }
    }

    Ok(current)
}

pub fn resolve_workspace_and_scope(path: &Path) -> Result<(Workspace, Option<WorkspaceScope>)> {
    let canonical = config::canonicalize_lossy(path)?;
    let metadata = fs::metadata(&canonical)
        .with_context(|| format!("failed to inspect path {}", canonical.display()))?;
    let scope_is_file = metadata.is_file();

    let workspace = Workspace::resolve(&canonical)?;
    let rel_path = canonical
        .strip_prefix(&workspace.root)
        .with_context(|| {
            format!(
                "path {} is not inside workspace root {}",
                canonical.display(),
                workspace.root.display()
            )
        })?
        .to_path_buf();

    let scope = if rel_path.as_os_str().is_empty() {
        None
    } else {
        Some(WorkspaceScope {
            rel_path,
            is_file: scope_is_file,
        })
    };

    Ok((workspace, scope))
}

pub fn workspace_id(root: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(root.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..16])
}

pub fn list_workspaces() -> Result<Vec<WorkspaceStatus>> {
    let root = config::indexes_root()?;
    if !root.exists() {
        return Ok(vec![]);
    }

    let mut by_id = BTreeMap::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let metadata_path = entry.path().join("workspace.json");
        if !metadata_path.exists() {
            continue;
        }

        let raw = fs::read(&metadata_path).with_context(|| {
            format!(
                "failed reading workspace metadata at {}",
                metadata_path.display()
            )
        })?;
        let metadata: WorkspaceMetadata = serde_json::from_slice(&raw)?;

        by_id.insert(
            metadata.id.clone(),
            WorkspaceStatus {
                id: metadata.id,
                root: metadata.root,
                last_indexed_at_unix: metadata.last_indexed_at_unix,
                watch_enabled: metadata.watch_enabled,
            },
        );
    }

    Ok(by_id.into_values().collect())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn resolve_workspace_and_scope_tracks_subpaths() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/lib.rs"), "pub fn sample() {}\n").unwrap();
        let canonical_root = config::canonicalize_lossy(tmp.path()).unwrap();

        let (workspace_dir, scope_dir) =
            resolve_workspace_and_scope(&tmp.path().join("src")).unwrap();
        assert_eq!(workspace_dir.root, canonical_root);
        assert_eq!(
            scope_dir,
            Some(WorkspaceScope {
                rel_path: PathBuf::from("src"),
                is_file: false,
            })
        );

        let (workspace_file, scope_file) =
            resolve_workspace_and_scope(&tmp.path().join("src/lib.rs")).unwrap();
        assert_eq!(workspace_file.root, canonical_root);
        assert_eq!(
            scope_file,
            Some(WorkspaceScope {
                rel_path: PathBuf::from("src/lib.rs"),
                is_file: true,
            })
        );
    }
}
