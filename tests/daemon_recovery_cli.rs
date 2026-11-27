#[cfg(unix)]
mod tests {
    use std::process::Command;
    use std::thread;
    use std::time::Duration;

    use assert_cmd::assert::OutputAssertExt;
    use assert_cmd::cargo::cargo_bin;
    use ivygrep::jobs::{JobKind, WATCHER_HEARTBEAT_TTL_SECS, job_status};
    use ivygrep::workspace::{Workspace, list_workspaces};
    use serial_test::serial;
    use tempfile::tempdir;

    fn wait_for_watcher_state(workspace: &Workspace, expected_alive: bool) -> bool {
        for _ in 0..50 {
            if list_workspaces()
                .unwrap_or_default()
                .into_iter()
                .find(|status| status.id == workspace.id)
                .is_some_and(|status| status.watcher_alive == expected_alive)
            {
                return true;
            }
            thread::sleep(Duration::from_millis(100));
        }
        false
    }

    fn kill_watcher_pid(workspace: &Workspace) {
        let status = job_status(workspace, JobKind::Watcher, WATCHER_HEARTBEAT_TTL_SECS);
        assert!(
            status.active(),
            "watcher should be active before termination"
        );
        let pid = status
            .record
            .and_then(|record| record.pid)
            .expect("active watcher must have a recorded PID");
        let pid = i32::try_from(pid).unwrap();
        let rc = unsafe { libc::kill(pid, libc::SIGKILL) };
        assert_eq!(rc, 0, "failed to kill daemon pid {pid}");
    }

    #[test]
    #[serial]
    fn query_revives_watcher_after_daemon_goes_offline() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        let repo = tempdir().unwrap();
        std::fs::write(
            repo.path().join("lib.rs"),
            "pub fn daemon_recovery_marker() -> bool { true }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(repo.path()).unwrap();

        Command::new(cargo_bin!("ig"))
            .env("IVYGREP_HOME", home.path())
            .env_remove("IVYGREP_NO_AUTOSPAWN")
            .args(["--add", repo.path().to_str().unwrap()])
            .assert()
            .success();

        assert!(
            wait_for_watcher_state(&workspace, true),
            "watcher should be live after `ig --add` when watch is enabled"
        );

        kill_watcher_pid(&workspace);
        assert!(
            wait_for_watcher_state(&workspace, false),
            "status should observe the killed watcher as offline"
        );

        Command::new(cargo_bin!("ig"))
            .current_dir(repo.path())
            .env("IVYGREP_HOME", home.path())
            .env_remove("IVYGREP_NO_AUTOSPAWN")
            .arg("daemon recovery marker")
            .assert()
            .success();

        assert!(
            wait_for_watcher_state(&workspace, true),
            "query should revive the watcher for a watch-configured workspace"
        );

        kill_watcher_pid(&workspace);
        ivygrep::ipc::cleanup_socket();
    }

    #[test]
    #[serial]
    fn spawned_daemon_does_not_hold_the_stdin_of_the_query() {
        use std::io::Write;

        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), "pub fn stdin_marker() {}\n").unwrap();

        let mut query = Command::new(cargo_bin!("ig"))
            .current_dir(repo.path())
            .env("IVYGREP_HOME", home.path())
            .env_remove("IVYGREP_NO_AUTOSPAWN")
            .arg("stdin_marker")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut stdin = query.stdin.take().unwrap();
        assert!(query.wait().unwrap().success());
        let pid: i32 = std::fs::read_to_string(home.path().join("daemon.pid"))
            .expect("the query must have started a daemon")
            .parse()
            .unwrap();
        // Rust ignores SIGPIPE, so a pipe without readers is an error here.
        let write = stdin.write_all(b"more input");
        unsafe { libc::kill(pid, libc::SIGKILL) };
        ivygrep::ipc::cleanup_socket();
        assert_eq!(
            write.unwrap_err().kind(),
            std::io::ErrorKind::BrokenPipe,
            "a producer must see that the query ended"
        );
    }
}
