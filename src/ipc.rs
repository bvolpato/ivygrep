#[cfg(unix)]
pub use unix::*;

#[cfg(not(unix))]
pub use windows::*;

#[cfg(unix)]
mod unix {
    use crate::config;
    use anyhow::{Context, Result};
    use std::path::PathBuf;
    pub use tokio::net::{UnixListener as IpcListener, UnixStream as IpcStream};

    pub fn socket_path() -> Result<PathBuf> {
        Ok(config::app_home()?.join("daemon.sock"))
    }

    pub async fn bind() -> Result<(IpcListener, PathBuf)> {
        let path = socket_path()?;
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        let listener = IpcListener::bind(&path)
            .with_context(|| format!("failed to bind socket {}", path.display()))?;
        Ok((listener, path))
    }

    pub async fn connect() -> std::io::Result<IpcStream> {
        let path = socket_path().map_err(|e| std::io::Error::other(e.to_string()))?;
        IpcStream::connect(path).await
    }

    pub fn cleanup_socket() {
        if let Ok(path) = socket_path() {
            let _ = std::fs::remove_file(path);
        }
    }

    pub fn socket_exists() -> bool {
        socket_path().map(|p| p.exists()).unwrap_or(false)
    }
}

#[cfg(not(unix))]
mod windows {
    use crate::config;
    use anyhow::{Context, Result};
    use std::path::PathBuf;
    pub use tokio::net::{TcpListener as IpcListener, TcpStream as IpcStream};

    pub fn socket_path() -> Result<PathBuf> {
        Ok(config::app_home()?.join("daemon.port"))
    }

    pub async fn bind() -> Result<(IpcListener, PathBuf)> {
        let path = socket_path()?;
        let listener = IpcListener::bind("127.0.0.1:0")
            .await
            .context("failed to bind tcp listener")?;
        let port = listener.local_addr()?.port();
        std::fs::write(&path, port.to_string()).context("failed to write daemon port file")?;
        Ok((listener, path))
    }

    pub async fn connect() -> std::io::Result<IpcStream> {
        let path = socket_path().map_err(|e| std::io::Error::other(e.to_string()))?;
        let port_str = std::fs::read_to_string(path)?;
        let port: u16 = port_str
            .trim()
            .parse()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid port"))?;
        IpcStream::connect(("127.0.0.1", port)).await
    }

    pub fn cleanup_socket() {
        if let Ok(path) = socket_path() {
            let _ = std::fs::remove_file(path);
        }
    }

    pub fn socket_exists() -> bool {
        socket_path().map(|p| p.exists()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bind_and_cleanup() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", tmp.path()) };
        let _ = crate::config::ensure_app_dirs();

        let (listener, path) = match bind().await {
            Ok(bound) => bound,
            Err(err)
                if err
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::PermissionDenied) =>
            {
                return;
            }
            Err(err) => panic!("bind failed unexpectedly: {err:#}"),
        };

        assert!(socket_exists(), "socket/port file should exist after bind");
        assert!(path.exists());

        drop(listener);
        cleanup_socket();

        assert!(!socket_exists(), "socket/port file should be cleaned up");
    }
}
