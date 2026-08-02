use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::workspace::Workspace;

pub(super) struct FreshIndexStaging {
    pub(super) dir: PathBuf,
    pub(super) sqlite_path: PathBuf,
    pub(super) tantivy_dir: PathBuf,
    pub(super) vector_path: PathBuf,
    active: bool,
}

impl FreshIndexStaging {
    pub(super) fn create(workspace: &Workspace) -> Result<Self> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = workspace.index_dir.join(format!(
            ".fresh-index-staging-{}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir)?;
        Ok(Self {
            sqlite_path: dir.join("metadata.sqlite3"),
            tantivy_dir: dir.join("tantivy"),
            vector_path: dir.join("vectors.usearch"),
            dir,
            active: true,
        })
    }

    pub(super) fn promote(mut self, workspace: &Workspace) -> Result<()> {
        anyhow::ensure!(self.sqlite_path.is_file(), "staged SQLite index is missing");
        anyhow::ensure!(self.tantivy_dir.is_dir(), "staged Tantivy index is missing");
        anyhow::ensure!(self.vector_path.is_file(), "staged vector index is missing");

        let backup_dir = workspace
            .index_dir
            .join(format!(".fresh-index-backup-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&backup_dir)?;
        let mut backups = Vec::new();
        for (index, live_path) in main_store_artifacts(workspace).into_iter().enumerate() {
            if !live_path.exists() {
                continue;
            }
            let backup_path = backup_dir.join(index.to_string());
            if let Err(error) = fs::rename(&live_path, &backup_path) {
                if let Err(rollback_error) = restore_main_store_backups(&backups) {
                    anyhow::bail!(
                        "failed to preserve live index {}: {error}; rollback failed: \
                         {rollback_error:#}; backups retained at {}",
                        live_path.display(),
                        backup_dir.display()
                    );
                }
                let _ = fs::remove_dir_all(&backup_dir);
                return Err(error).with_context(|| {
                    format!("failed to preserve live index {}", live_path.display())
                });
            }
            backups.push((live_path, backup_path));
        }

        let promotions = [
            (self.sqlite_path.clone(), workspace.sqlite_path()),
            (self.tantivy_dir.clone(), workspace.tantivy_dir()),
            (self.vector_path.clone(), workspace.vector_path()),
        ];
        let mut promoted = Vec::<PathBuf>::new();
        for (staged_path, live_path) in promotions {
            if let Err(error) = fs::rename(&staged_path, &live_path) {
                if let Err(rollback_error) = rollback_main_store(&promoted, &backups) {
                    anyhow::bail!(
                        "failed to promote staged index {} -> {}: {error}; rollback failed: \
                         {rollback_error:#}; backups retained at {}",
                        staged_path.display(),
                        live_path.display(),
                        backup_dir.display()
                    );
                }
                let _ = fs::remove_dir_all(&backup_dir);
                return Err(error).with_context(|| {
                    format!(
                        "failed to promote staged index {} -> {}",
                        staged_path.display(),
                        live_path.display()
                    )
                });
            }
            promoted.push(live_path);
        }

        self.active = false;
        let _ = fs::remove_dir_all(backup_dir);
        let _ = fs::remove_dir_all(&self.dir);
        Ok(())
    }
}

impl Drop for FreshIndexStaging {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
}

fn sqlite_sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut sidecar = path.as_os_str().to_owned();
    sidecar.push(suffix);
    PathBuf::from(sidecar)
}

fn remove_path_if_exists(path: &Path) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn main_store_artifacts(workspace: &Workspace) -> [PathBuf; 6] {
    let sqlite_path = workspace.sqlite_path();
    [
        sqlite_path.clone(),
        sqlite_sidecar_path(&sqlite_path, "-wal"),
        sqlite_sidecar_path(&sqlite_path, "-shm"),
        workspace.tantivy_dir(),
        workspace.vector_path(),
        workspace.vector_path().with_extension("usearch.bak"),
    ]
}

fn restore_main_store_backups(backups: &[(PathBuf, PathBuf)]) -> Result<()> {
    for (live_path, backup_path) in backups.iter().rev() {
        fs::rename(backup_path, live_path).with_context(|| {
            format!(
                "failed to restore live index {} from {}",
                live_path.display(),
                backup_path.display()
            )
        })?;
    }
    Ok(())
}

fn rollback_main_store(promoted: &[PathBuf], backups: &[(PathBuf, PathBuf)]) -> Result<()> {
    for path in promoted.iter().rev() {
        remove_path_if_exists(path)
            .with_context(|| format!("failed to remove partial index {}", path.display()))?;
    }
    restore_main_store_backups(backups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_workspace(id: &str, root: &Path, index_root: &Path) -> Workspace {
        Workspace {
            id: id.to_string(),
            root: root.to_path_buf(),
            index_dir: index_root.join("index"),
            repo_id: None,
            base_index_dir: None,
        }
    }

    #[test]
    fn promotion_replaces_main_artifacts_and_preserves_lock() {
        let root = tempdir().unwrap();
        let index_root = tempdir().unwrap();
        let workspace = test_workspace("staging-test", root.path(), index_root.path());
        fs::create_dir_all(&workspace.index_dir).unwrap();
        fs::write(workspace.lock_path(), "locked").unwrap();
        fs::write(workspace.sqlite_path(), "old sqlite").unwrap();
        fs::write(
            sqlite_sidecar_path(&workspace.sqlite_path(), "-wal"),
            "old wal",
        )
        .unwrap();
        fs::create_dir_all(workspace.tantivy_dir()).unwrap();
        fs::write(workspace.tantivy_dir().join("old"), "old tantivy").unwrap();
        fs::write(workspace.vector_path(), "old vector").unwrap();
        fs::write(
            workspace.vector_path().with_extension("usearch.bak"),
            "old vector backup",
        )
        .unwrap();

        let staging = FreshIndexStaging::create(&workspace).unwrap();
        let staging_dir = staging.dir.clone();
        fs::write(&staging.sqlite_path, "new sqlite").unwrap();
        fs::create_dir_all(&staging.tantivy_dir).unwrap();
        fs::write(staging.tantivy_dir.join("new"), "new tantivy").unwrap();
        fs::write(&staging.vector_path, "new vector").unwrap();

        staging.promote(&workspace).unwrap();

        assert_eq!(fs::read_to_string(workspace.lock_path()).unwrap(), "locked");
        assert_eq!(
            fs::read_to_string(workspace.sqlite_path()).unwrap(),
            "new sqlite"
        );
        assert!(!sqlite_sidecar_path(&workspace.sqlite_path(), "-wal").exists());
        assert!(workspace.tantivy_dir().join("new").exists());
        assert!(!workspace.tantivy_dir().join("old").exists());
        assert_eq!(
            fs::read_to_string(workspace.vector_path()).unwrap(),
            "new vector"
        );
        assert!(
            !workspace
                .vector_path()
                .with_extension("usearch.bak")
                .exists()
        );
        assert!(!staging_dir.exists());
    }

    #[test]
    fn incomplete_staging_preserves_main_artifacts() {
        let root = tempdir().unwrap();
        let index_root = tempdir().unwrap();
        let workspace = test_workspace("rollback-test", root.path(), index_root.path());
        fs::create_dir_all(&workspace.index_dir).unwrap();
        fs::write(workspace.lock_path(), "locked").unwrap();
        fs::write(workspace.sqlite_path(), "old sqlite").unwrap();
        fs::create_dir_all(workspace.tantivy_dir()).unwrap();
        fs::write(workspace.tantivy_dir().join("old"), "old tantivy").unwrap();
        fs::write(workspace.vector_path(), "old vector").unwrap();

        let staging = FreshIndexStaging::create(&workspace).unwrap();
        fs::write(&staging.sqlite_path, "new sqlite").unwrap();
        fs::create_dir_all(&staging.tantivy_dir).unwrap();
        fs::write(staging.tantivy_dir.join("new"), "new tantivy").unwrap();

        assert!(staging.promote(&workspace).is_err());
        assert_eq!(fs::read_to_string(workspace.lock_path()).unwrap(), "locked");
        assert_eq!(
            fs::read_to_string(workspace.sqlite_path()).unwrap(),
            "old sqlite"
        );
        assert!(workspace.tantivy_dir().join("old").exists());
        assert_eq!(
            fs::read_to_string(workspace.vector_path()).unwrap(),
            "old vector"
        );
    }
}
