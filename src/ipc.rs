#[cfg(unix)]
pub use unix::*;

#[cfg(not(unix))]
pub use windows::*;

/// Acquire the daemon single-instance lock for this app home.
///
/// Returns `Some(file)` when the lock is acquired — the caller must hold the
/// returned handle for the daemon's lifetime (dropping it, or process exit,
/// releases the lock). Returns `None` if another live daemon already holds it,
/// in which case the caller should exit without binding the socket.
///
/// This is what prevents a restart/auto-spawn race from leaving two daemons
/// both bound to (and stealing) the socket, where the first becomes a zombie
/// still holding file watchers. We retry briefly so a daemon started during
/// the restart handover (while the outgoing daemon is still exiting) can take
/// over instead of giving up.
pub fn acquire_daemon_lock() -> anyhow::Result<Option<std::fs::File>> {
    let path = crate::config::app_home()?.join("daemon.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        // The lock file is a pure flock anchor; never truncate its contents.
        .truncate(false)
        .open(&path)?;
    for _ in 0..20 {
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => return Ok(Some(file)),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(None)
}

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
        // Restrict the socket to the owner so other local users can't connect.
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        Ok((listener, path))
    }

    /// Returns true if the connecting peer is the same uid as this process.
    /// The daemon only ever serves its own user; reject anyone else (the
    /// socket exposes cross-workspace search/index/delete).
    pub fn peer_is_owner(stream: &IpcStream) -> bool {
        match stream.peer_cred() {
            Ok(cred) => cred.uid() == unsafe { libc::geteuid() },
            // If we can't verify the peer, fail closed.
            Err(_) => false,
        }
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

    /// Windows uses a loopback TCP port; peer-uid checks don't apply.
    pub fn peer_is_owner(_stream: &IpcStream) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
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

        // The socket and app home must be owner-only so other local users
        // can't connect to the daemon or read the index.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let sock_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(sock_mode, 0o600, "daemon socket must be mode 0600");
            let home_mode = std::fs::metadata(tmp.path()).unwrap().permissions().mode() & 0o777;
            assert_eq!(home_mode, 0o700, "app home must be mode 0700");
        }

        drop(listener);
        cleanup_socket();

        assert!(!socket_exists(), "socket/port file should be cleaned up");
    }
}
