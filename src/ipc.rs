#[cfg(unix)]
pub use unix::*;

#[cfg(not(unix))]
pub use windows::*;

fn open_daemon_lock() -> anyhow::Result<std::fs::File> {
    let path = crate::config::app_home()?.join("daemon.lock");
    Ok(std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        // The lock file is a pure flock anchor; never truncate its contents.
        .truncate(false)
        .open(&path)?)
}

fn is_lock_contended(error: &std::io::Error) -> bool {
    let fs2_raw_error = fs2::lock_contended_error().raw_os_error();
    error.kind() == std::io::ErrorKind::WouldBlock
        || (fs2_raw_error.is_some() && error.raw_os_error() == fs2_raw_error)
}

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
    let file = open_daemon_lock()?;
    for _ in 0..20 {
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => return Ok(Some(file)),
            Err(e) if is_lock_contended(&e) => {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(None)
}

/// Remove a stale daemon endpoint only after proving no daemon owns the
/// single-instance lock. Returns `false` when a live daemon still owns it.
pub fn cleanup_stale_socket() -> anyhow::Result<bool> {
    let file = open_daemon_lock()?;
    match fs2::FileExt::try_lock_exclusive(&file) {
        Ok(()) => {
            let path = socket_path()?;
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err.into()),
            }
            Ok(true)
        }
        Err(err) if is_lock_contended(&err) => Ok(false),
        Err(err) => Err(err.into()),
    }
}

pub struct DaemonPidGuard {
    path: std::path::PathBuf,
}

impl Drop for DaemonPidGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn daemon_pid_path() -> anyhow::Result<std::path::PathBuf> {
    Ok(crate::config::app_home()?.join("daemon.pid"))
}

pub fn write_daemon_pid() -> anyhow::Result<DaemonPidGuard> {
    let path = daemon_pid_path()?;
    std::fs::write(&path, std::process::id().to_string())?;
    Ok(DaemonPidGuard { path })
}

pub fn cleanup_daemon_pid() {
    if let Ok(path) = daemon_pid_path() {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(target_os = "linux")]
pub fn terminate_recorded_daemon(timeout: std::time::Duration) -> bool {
    let Some(pid) = read_recorded_pid() else {
        return false;
    };
    if pid <= 1 || pid == std::process::id() as i32 {
        return false;
    }
    if !recorded_pid_is_ig(pid) {
        if !pid_alive(pid) {
            cleanup_daemon_pid();
        }
        return false;
    }
    if !pid_alive(pid) {
        cleanup_daemon_pid();
        return false;
    }

    let _ = unsafe { libc::kill(pid, libc::SIGTERM) };
    if wait_for_pid_exit(pid, timeout) {
        cleanup_daemon_pid();
        return true;
    }

    let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
    let stopped = wait_for_pid_exit(pid, std::time::Duration::from_secs(1));
    if stopped {
        cleanup_daemon_pid();
    }
    stopped
}

#[cfg(not(target_os = "linux"))]
pub fn terminate_recorded_daemon(_timeout: std::time::Duration) -> bool {
    false
}

#[cfg(target_os = "linux")]
fn read_recorded_pid() -> Option<i32> {
    let path = daemon_pid_path().ok()?;
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

#[cfg(target_os = "linux")]
fn recorded_pid_is_ig(pid: i32) -> bool {
    let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok();
    exe.as_deref()
        .and_then(std::path::Path::file_stem)
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("ig"))
}

#[cfg(target_os = "linux")]
fn pid_alive(pid: i32) -> bool {
    let rc = unsafe { libc::kill(pid, 0) };
    rc == 0
        || std::io::Error::last_os_error()
            .raw_os_error()
            .is_some_and(|code| code == libc::EPERM)
}

#[cfg(target_os = "linux")]
fn wait_for_pid_exit(pid: i32, timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    !pid_alive(pid)
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
        // Fail closed: refuse to serve rather than expose an unprotected
        // control socket if the chmod can't be applied.
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).with_context(
            || {
                format!(
                    "failed to restrict socket permissions to 0600: {}",
                    path.display()
                )
            },
        )?;
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
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    pub use tokio::net::TcpStream as IpcStream;

    const TOKEN_LENGTH: usize = 32;
    const HANDSHAKE_LENGTH: usize = TOKEN_LENGTH + 1;

    pub struct IpcListener {
        inner: TcpListener,
        token: [u8; TOKEN_LENGTH],
    }

    impl IpcListener {
        pub async fn accept(&self) -> std::io::Result<(IpcStream, SocketAddr)> {
            loop {
                let (mut stream, address) = self.inner.accept().await?;
                let mut handshake = [0u8; HANDSHAKE_LENGTH];
                let authenticated =
                    tokio::time::timeout(Duration::from_secs(1), stream.read_exact(&mut handshake))
                        .await
                        .is_ok_and(|result| result.is_ok())
                        && handshake[..TOKEN_LENGTH] == self.token
                        && handshake[TOKEN_LENGTH] == b'\n';
                if authenticated {
                    return Ok((stream, address));
                }
            }
        }
    }

    pub fn socket_path() -> Result<PathBuf> {
        Ok(config::app_home()?.join("daemon.port"))
    }

    pub async fn bind() -> Result<(IpcListener, PathBuf)> {
        let path = socket_path()?;
        let inner = TcpListener::bind("127.0.0.1:0")
            .await
            .context("failed to bind tcp listener")?;
        let port = inner.local_addr()?.port();
        let token_string = uuid::Uuid::new_v4().simple().to_string();
        let token: [u8; TOKEN_LENGTH] = token_string
            .as_bytes()
            .try_into()
            .expect("UUID simple format has 32 bytes");
        std::fs::write(&path, format!("{port}\n{token_string}\n"))
            .context("failed to write daemon endpoint file")?;
        let listener = IpcListener { inner, token };
        Ok((listener, path))
    }

    pub async fn connect() -> std::io::Result<IpcStream> {
        let path = socket_path().map_err(|e| std::io::Error::other(e.to_string()))?;
        let endpoint = std::fs::read_to_string(path)?;
        let mut lines = endpoint.lines();
        let port: u16 = lines
            .next()
            .unwrap_or_default()
            .trim()
            .parse()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid port"))?;
        let token = lines.next().unwrap_or_default().trim();
        if token.len() != TOKEN_LENGTH || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid daemon authentication token",
            ));
        }
        let mut stream = IpcStream::connect(("127.0.0.1", port)).await?;
        stream.write_all(token.as_bytes()).await?;
        stream.write_all(b"\n").await?;
        stream.flush().await?;
        Ok(stream)
    }

    pub fn cleanup_socket() {
        if let Ok(path) = socket_path() {
            let _ = std::fs::remove_file(path);
        }
    }

    pub fn socket_exists() -> bool {
        socket_path().map(|p| p.exists()).unwrap_or(false)
    }

    /// The listener validates the per-daemon token before returning the stream.
    pub fn peer_is_owner(_stream: &IpcStream) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_lock_contention_error_is_recognized() {
        assert!(is_lock_contended(&fs2::lock_contended_error()));
        assert!(!is_lock_contended(&std::io::Error::other("unrelated")));
    }
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

        // The socket must be owner-only (0600), and the ivygrep-owned index
        // dir must be 0700, so other local users can't connect to the daemon
        // or read indexed source.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let sock_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(sock_mode, 0o600, "daemon socket must be mode 0600");
            let idx = crate::config::indexes_root().unwrap();
            let idx_mode = std::fs::metadata(&idx).unwrap().permissions().mode() & 0o777;
            assert_eq!(idx_mode, 0o700, "index dir must be mode 0700");
        }

        drop(listener);
        cleanup_socket();

        assert!(!socket_exists(), "socket/port file should be cleaned up");
    }

    #[test]
    #[serial]
    fn daemon_pid_guard_writes_and_cleans() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", tmp.path()) };
        let _ = crate::config::ensure_app_dirs();

        {
            let _guard = write_daemon_pid().unwrap();
            assert_eq!(
                std::fs::read_to_string(daemon_pid_path().unwrap()).unwrap(),
                std::process::id().to_string()
            );
        }

        assert!(!daemon_pid_path().unwrap().exists());
    }

    fn describe_lock_holders(path: &std::path::Path) -> String {
        let lsof = std::process::Command::new("lsof")
            .arg("--")
            .arg(path)
            .output()
            .map(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if stdout.is_empty() {
                    format!("lsof reported no open handles (status {})", output.status)
                } else {
                    stdout
                }
            })
            .unwrap_or_else(|err| format!("lsof unavailable: {err}"));
        format!("pid {} in-process; {lsof}", std::process::id())
    }

    #[test]
    #[serial]
    fn stale_socket_cleanup_requires_daemon_lock_ownership() {
        let tmp = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", tmp.path()) };
        crate::config::ensure_app_dirs().unwrap();

        let daemon_lock = acquire_daemon_lock()
            .unwrap()
            .expect("test should acquire daemon lock");
        let endpoint = socket_path().unwrap();
        std::fs::write(&endpoint, b"stale endpoint").unwrap();

        assert!(!cleanup_stale_socket().unwrap());
        assert!(endpoint.exists(), "held daemon endpoint must be preserved");

        drop(daemon_lock);
        // The lock is only ever held briefly by a concurrent endpoint probe in
        // this process (a failed connect runs the same cleanup); a holder that
        // persists across the retries is the failure this test is about, and
        // the failure names it so a CI flake is diagnosable from the log.
        let mut cleaned = false;
        for _ in 0..50 {
            if cleanup_stale_socket().unwrap() {
                cleaned = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            cleaned,
            "released daemon lock must become acquirable; holders: {}",
            describe_lock_holders(&crate::config::app_home().unwrap().join("daemon.lock"))
        );
        assert!(
            !endpoint.exists(),
            "unowned stale endpoint should be removed"
        );
    }
}
