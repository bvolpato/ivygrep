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

    pub(super) fn create_overlay(workspace: &Workspace) -> Result<Self> {
        let mut staging = Self::create(workspace)?;
        let staged_workspace = staging.workspace(workspace);
        staging.sqlite_path = staged_workspace.overlay_sqlite_path();
        staging.tantivy_dir = staged_workspace.overlay_tantivy_dir();
        staging.vector_path = staged_workspace.overlay_vector_path();
        Ok(staging)
    }

    pub(super) fn workspace(&self, workspace: &Workspace) -> Workspace {
        let mut staged = workspace.clone();
        staged.index_dir = self.dir.clone();
        staged
    }

    pub(super) fn promote(self, workspace: &Workspace) -> Result<()> {
        let promotions = vec![
            (self.sqlite_path.clone(), workspace.sqlite_path()),
            (self.tantivy_dir.clone(), workspace.tantivy_dir()),
            (self.vector_path.clone(), workspace.vector_path()),
        ];
        self.promote_artifacts(workspace, main_store_artifacts(workspace), promotions)
    }

    pub(super) fn promote_overlay(self, workspace: &Workspace) -> Result<()> {
        let staged = self.workspace(workspace);
        let mut promotions = vec![
            (self.sqlite_path.clone(), workspace.overlay_sqlite_path()),
            (self.tantivy_dir.clone(), workspace.overlay_tantivy_dir()),
            (self.vector_path.clone(), workspace.overlay_vector_path()),
            (staged.base_ref_path(), workspace.base_ref_path()),
            (
                staged.merkle_snapshot_path(),
                workspace.merkle_snapshot_path(),
            ),
            (
                staged.index_format_version_path(),
                workspace.index_format_version_path(),
            ),
        ];
        let verified = staged.merkle_snapshot_path().with_extension("verified");
        if verified.is_file() {
            promotions.push((
                verified,
                workspace.merkle_snapshot_path().with_extension("verified"),
            ));
        }
        // Completion is published last. Its backup is moved first, so health
        // checks cannot accept a partially promoted or rolled-back overlay.
        promotions.push((staged.metadata_path(), workspace.metadata_path()));
        for (path, _) in &promotions {
            anyhow::ensure!(
                path.exists(),
                "staged overlay artifact is missing: {}",
                path.display()
            );
        }
        self.promote_artifacts(workspace, overlay_store_artifacts(workspace), promotions)
    }

    fn promote_artifacts(
        mut self,
        workspace: &Workspace,
        artifacts: Vec<PathBuf>,
        mut promotions: Vec<(PathBuf, PathBuf)>,
    ) -> Result<()> {
        anyhow::ensure!(self.sqlite_path.is_file(), "staged SQLite index is missing");
        anyhow::ensure!(self.tantivy_dir.is_dir(), "staged Tantivy index is missing");
        anyhow::ensure!(self.vector_path.is_file(), "staged vector index is missing");

        // Rotate with the stores, including on overlay recreation. Generation
        // numbers alone can repeat after removal, and an old enhancer may still
        // hold the replaced SQLite file open. Rollback restores the old identity.
        let staged_incarnation = self.dir.join("index_incarnation");
        fs::write(&staged_incarnation, uuid::Uuid::new_v4().to_string())?;
        promotions.insert(0, (staged_incarnation, workspace.index_incarnation_path()));

        let backup_dir = workspace
            .index_dir
            .join(format!(".fresh-index-backup-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&backup_dir)?;
        let mut backups = Vec::new();
        for (index, live_path) in artifacts.into_iter().enumerate() {
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

        let mut promoted = Vec::<PathBuf>::new();
        for (staged_path, live_path) in promotions {
            if let Err(error) = promote_path(&staged_path, &live_path) {
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

fn promote_path(staged: &Path, live: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    test_support::check_publication(live)?;
    fs::rename(staged, live)
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

fn main_store_artifacts(workspace: &Workspace) -> Vec<PathBuf> {
    let sqlite_path = workspace.sqlite_path();
    vec![
        workspace.index_incarnation_path(),
        sqlite_path.clone(),
        sqlite_sidecar_path(&sqlite_path, "-wal"),
        sqlite_sidecar_path(&sqlite_path, "-shm"),
        workspace.tantivy_dir(),
        workspace.vector_path(),
        workspace.vector_path().with_extension("usearch.bak"),
        workspace.vector_path().with_extension("usearch.tmp"),
        // A complete replacement invalidates derived vectors and deletion
        // journals too. Back them up with the stores so a failed promotion
        // restores the entire previous generation.
        workspace.vector_neural_path(),
        workspace.vector_neural_path().with_extension("usearch.bak"),
        workspace.vector_neural_path().with_extension("usearch.tmp"),
        workspace.neural_model_path(),
        workspace.neural_profile_path(),
        workspace.neural_backend_path(),
        workspace.hash_tombstones_path(),
        workspace.hash_tombstones_processing_path(),
        workspace.hash_enhanced_generation_path(),
        workspace.neural_tombstones_path(),
        workspace.neural_tombstones_processing_path(),
        workspace.neural_enhanced_generation_path(),
    ]
}

fn overlay_store_artifacts(workspace: &Workspace) -> Vec<PathBuf> {
    let sqlite = workspace.overlay_sqlite_path();
    let vectors = workspace.overlay_vector_path();
    let neural = workspace.vector_neural_path();
    vec![
        workspace.metadata_path(),
        workspace.index_incarnation_path(),
        sqlite.clone(),
        sqlite_sidecar_path(&sqlite, "-wal"),
        sqlite_sidecar_path(&sqlite, "-shm"),
        workspace.overlay_tantivy_dir(),
        vectors.clone(),
        vectors.with_extension("usearch.bak"),
        vectors.with_extension("usearch.tmp"),
        neural.clone(),
        neural.with_extension("usearch.bak"),
        neural.with_extension("usearch.tmp"),
        workspace.neural_model_path(),
        workspace.neural_profile_path(),
        workspace.neural_backend_path(),
        workspace.hash_tombstones_path(),
        workspace.hash_tombstones_processing_path(),
        workspace.neural_tombstones_path(),
        workspace.neural_tombstones_processing_path(),
        workspace.hash_enhanced_generation_path(),
        workspace.neural_enhanced_generation_path(),
        workspace.base_ref_path(),
        workspace.merkle_snapshot_path(),
        workspace.merkle_snapshot_path().with_extension("verified"),
        workspace.index_format_version_path(),
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
pub(super) mod test_support {
    use std::collections::HashSet;
    use std::sync::LazyLock;

    use parking_lot::Mutex;

    use super::*;

    static FAILED_PUBLICATIONS: LazyLock<Mutex<HashSet<PathBuf>>> =
        LazyLock::new(|| Mutex::new(HashSet::new()));

    pub(crate) struct PublicationFailure(PathBuf);

    impl Drop for PublicationFailure {
        fn drop(&mut self) {
            FAILED_PUBLICATIONS.lock().remove(&self.0);
        }
    }

    pub(crate) fn fail_publication(path: &Path) -> PublicationFailure {
        assert!(FAILED_PUBLICATIONS.lock().insert(path.to_path_buf()));
        PublicationFailure(path.to_path_buf())
    }

    pub(crate) fn check_publication(path: &Path) -> std::io::Result<()> {
        if FAILED_PUBLICATIONS.lock().contains(path) {
            return Err(std::io::Error::other(
                "injected workspace publication failure",
            ));
        }
        Ok(())
    }
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
