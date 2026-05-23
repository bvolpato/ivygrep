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
    let home = app_home()?;
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(indexes_root()?)?;
    // Restrict the app home to its owner. It holds the daemon socket and the
    // index (which stores decompressed source of every indexed repo, possibly
    // including secrets). 0700 prevents other local users on a shared host from
    // reading indexed content off disk or reaching the daemon socket.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Fail closed: if we can't tighten the app home, the index/socket would
        // be left readable by other local users, so surface the error rather
        // than silently continuing with weak permissions.
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).with_context(
            || {
                format!(
                    "failed to restrict app home permissions to 0700: {}",
                    home.display()
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
