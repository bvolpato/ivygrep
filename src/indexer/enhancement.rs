//! Enrichment computes outside the index lock, then publishes only into the
//! same store incarnation. A separate lock serializes vector writers without
//! stopping lexical indexing while a model is running or paused.
use std::fs::{self, File, OpenOptions};

use anyhow::{Context, Result};

use crate::workspace::Workspace;

pub(crate) struct IndexLock(File);

impl IndexLock {
    pub(super) fn acquire(workspace: &Workspace) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(workspace.lock_path())?;
        fs2::FileExt::lock_exclusive(&file).context("lock index for enhancement publication")?;
        Ok(Self(file))
    }
}

impl Drop for IndexLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

pub(super) fn lock_worker(workspace: &Workspace) -> Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(workspace.index_dir.join("enhancement.lock"))?;
    fs2::FileExt::lock_exclusive(&file).context("lock background enhancement worker")?;
    Ok(file)
}

#[derive(Debug)]
pub(crate) struct EnhancementSuperseded;

impl std::fmt::Display for EnhancementSuperseded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("index was replaced during background enhancement; discard obsolete vectors")
    }
}

impl std::error::Error for EnhancementSuperseded {}

pub(crate) struct EnhancementSnapshot {
    incarnation: String,
    pub(super) generation: u64,
}

impl EnhancementSnapshot {
    pub(crate) fn begin(workspace: &Workspace) -> Result<(Self, IndexLock)> {
        let lock = IndexLock::acquire(workspace)?;
        Ok((Self::capture(workspace)?, lock))
    }

    /// Caller holds the index lock until all initial stores and journals have
    /// been opened. Older indexes acquire an incarnation lazily under that lock.
    pub(super) fn capture(workspace: &Workspace) -> Result<Self> {
        let incarnation = match workspace.read_index_incarnation()? {
            Some(value) => value,
            None => {
                let value = uuid::Uuid::new_v4().to_string();
                fs::write(workspace.index_incarnation_path(), &value)?;
                value
            }
        };
        let generation = workspace
            .read_metadata()?
            .map(|metadata| metadata.index_generation)
            .unwrap_or(0);
        Ok(Self {
            incarnation,
            generation,
        })
    }

    pub(crate) fn lock_current(&self, workspace: &Workspace) -> Result<IndexLock> {
        let lock = IndexLock::acquire(workspace)?;
        self.verify_current(workspace)?;
        Ok(lock)
    }

    /// Caller holds index.lock, including when binding a CLI job across stages.
    pub(super) fn verify_current(&self, workspace: &Workspace) -> Result<()> {
        if workspace.read_index_incarnation()?.as_deref() != Some(self.incarnation.as_str()) {
            return Err(EnhancementSuperseded.into());
        }
        Ok(())
    }
}
