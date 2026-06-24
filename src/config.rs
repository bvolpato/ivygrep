use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub fn app_home() -> Result<PathBuf> {
    if let Some(path) = non_empty_env_path("IVYGREP_HOME") {
        return Ok(path);
    }

    if let Some(xdg_data_home) = non_empty_env_path("XDG_DATA_HOME") {
        return Ok(xdg_data_home.join("ivygrep"));
    }

    let home = dirs::home_dir().context("unable to resolve home directory")?;
    Ok(home.join(".local/share/ivygrep"))
}

pub fn indexes_root() -> Result<PathBuf> {
    Ok(app_home()?.join("indexes"))
}

pub fn ensure_app_dirs() -> Result<()> {
    std::fs::create_dir_all(app_home()?)?;
    let indexes = indexes_root()?;
    std::fs::create_dir_all(&indexes)?;
    // Tighten the ivygrep-owned index directory to 0700 — it stores the
    // decompressed source of every indexed repo (possibly including secrets),
    // so other local users on a shared host must not read it. We deliberately
    // do NOT chmod the app home itself: IVYGREP_HOME may point at a
    // pre-existing/shared directory the user controls, and the daemon socket is
    // protected independently (0600 + peer-cred). Fail closed if the chmod of
    // our own index dir fails rather than leave it world-readable.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&indexes, std::fs::Permissions::from_mode(0o700)).with_context(
            || {
                format!(
                    "failed to restrict index directory permissions to 0700: {}",
                    indexes.display()
                )
            },
        )?;
    }
    Ok(())
}

pub fn canonicalize_lossy(path: &Path) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize path {}", path.to_string_lossy()))?;
    Ok(canonical)
}

fn non_empty_env_path(key: &str) -> Option<PathBuf> {
    env::var(key).ok().and_then(|value| parse_env_path(&value))
}

fn parse_env_path(value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

pub fn background_enhancement_enabled() -> bool {
    env::var_os("IVYGREP_NO_AUTOSPAWN").is_none()
        && env::var_os("IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT").is_none()
}

pub fn query_result_cache_enabled() -> bool {
    env::var_os("IVYGREP_DISABLE_QUERY_CACHE").is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_env_path_rejects_empty_values() {
        assert_eq!(parse_env_path(""), None);
        assert_eq!(parse_env_path("   "), None);
    }

    #[test]
    fn parse_env_path_accepts_trimmed_path() {
        let path = parse_env_path("  /tmp/ivygrep-home  ").unwrap();
        assert_eq!(path, PathBuf::from("/tmp/ivygrep-home"));
    }
}
