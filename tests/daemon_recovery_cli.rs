#[cfg(unix)]
mod tests {
    use std::collections::BTreeSet;
    use std::process::{Child, Command, Stdio};
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

    struct Daemon(Child);

    impl Drop for Daemon {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    #[serial]
    fn add_and_remove_json_have_one_schema_with_and_without_a_daemon() {
        let home = tempdir().unwrap();
        let repo = tempdir().unwrap();
        std::fs::write(repo.path().join("lib.rs"), "pub fn schema_marker() {}\n").unwrap();
        let ig = |args: &[&str]| {
            let output = Command::new(cargo_bin!("ig"))
                .current_dir(repo.path())
                .env("IVYGREP_HOME", home.path())
                .env("IVYGREP_NO_AUTOSPAWN", "1")
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
        };
        let keys = |value: &serde_json::Value| {
            value
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>()
        };
        let add = ["--add", ".", "--no-watch", "--json"];
        let local = keys(&ig(&add));
        let removed = ig(&["--rm", ".", "--json"]);

        let _daemon = Daemon(
            Command::new(cargo_bin!("ig"))
                .env("IVYGREP_HOME", home.path())
                .arg("--daemon")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        for _ in 0..400 {
            if home.path().join("daemon.sock").exists() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        for indexed_files in [1, 0] {
            let added = ig(&add);
            assert_eq!(keys(&added), local);
            assert_eq!(added["indexed_files"], indexed_files);
        }
        assert_eq!(ig(&["--rm", ".", "--json"]), removed);

        // The daemon finds the index of a directory that is gone.
        std::fs::create_dir(repo.path().join("deleted")).unwrap();
        ig(&["--add", "deleted", "--no-watch", "--json"]);
        std::fs::remove_dir(repo.path().join("deleted")).unwrap();
        ig(&["--rm", "deleted", "--json"]);
        assert_eq!(ig(&["--status", "--json"]), serde_json::json!([]));
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
}
