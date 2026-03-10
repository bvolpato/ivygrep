use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub fn app_home() -> Result<PathBuf> {
    if let Ok(path) = env::var("IVYGREP_HOME") {
        return Ok(PathBuf::from(path));
    }

    let base = dirs::data_local_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".local/share")))
        .context("unable to resolve local data directory")?;

    Ok(base.join("ivygrep"))
}

pub fn indexes_root() -> Result<PathBuf> {
    Ok(app_home()?.join("indexes"))
}

pub fn socket_path() -> Result<PathBuf> {
    Ok(app_home()?.join("daemon.sock"))
}

pub fn ensure_app_dirs() -> Result<()> {
    std::fs::create_dir_all(indexes_root()?)?;
    Ok(())
}

pub fn canonicalize_lossy(path: &Path) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize path {}", path.to_string_lossy()))?;
    Ok(canonical)
}
