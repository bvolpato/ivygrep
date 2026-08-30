use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

use crate::embedding::NeuralModelIdentity;

pub(crate) fn read_identity(path: &Path) -> Result<Option<NeuralModelIdentity>> {
    let contents = match fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("read neural model metadata {}", path.display()));
        }
    };
    serde_json::from_slice(&contents)
        .with_context(|| format!("parse neural model metadata {}", path.display()))
        .map(Some)
}

pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(contents)?;
        drop(file);
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.with_context(|| format!("publish neural metadata {}", path.display()))
}
