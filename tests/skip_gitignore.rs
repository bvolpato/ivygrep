use assert_cmd::Command;
use serial_test::serial;
use tempfile::tempdir;

#[test]
#[serial]
fn test_skip_gitignore_overrides_exclusions() {
    let tmp = tempdir().unwrap();
    let repo_root = tmp.path().join("repo");
    let home = tmp.path().join("ivygrep_home");

    std::fs::create_dir_all(&repo_root).unwrap();

    // Create a git repo
    std::fs::create_dir_all(repo_root.join(".git")).unwrap();

    // Create a .gitignore that ignores secret.txt
    std::fs::write(repo_root.join(".gitignore"), "secret.txt\n").unwrap();

    // Create an ignored file
    std::fs::write(
        repo_root.join("secret.txt"),
        "This is a hidden-secret-string inside an ignored file.\n",
    )
    .unwrap();

    // Create a regular file
    std::fs::write(repo_root.join("public.txt"), "This is public data.\n").unwrap();

    // Step 1: Initial run to index the repository without --skip-gitignore
    // We search for "public" to force indexing. The index will NOT contain secret.txt.
    let mut cmd1 = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    cmd1.current_dir(&repo_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "public"])
        .assert()
        .success();

    // Step 2: Search for the secret WITHOUT --skip-gitignore.
    // It should yield zero results because secret.txt is gitignored.
    let mut cmd2 = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output2 = cmd2
        .current_dir(&repo_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "hidden-secret-string"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json2: serde_json::Value = serde_json::from_slice(&output2).unwrap();
    let files2 = json2.as_array().unwrap();
    let has_secret = files2
        .iter()
        .any(|f| f.get("file_path").and_then(|v| v.as_str()) == Some("secret.txt"));
    assert!(
        !has_secret,
        "Secret file 'secret.txt' should not be found when respecting gitignore. Found: {:?}",
        files2
    );

    // Step 3: Search for the secret WITH --skip-gitignore.
    // This should correctly fallback to regex and find the secret text.
    let mut cmd3 = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output3 = cmd3
        .current_dir(&repo_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--json",
            "--hash",
            "--skip-gitignore",
            "hidden-secret-string",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json3: serde_json::Value = serde_json::from_slice(&output3).unwrap();
    let files3 = json3.as_array().unwrap();
    assert!(
        !files3.is_empty(),
        "Secret must be found when using --skip-gitignore"
    );

    let file_path = files3[0].get("file_path").unwrap().as_str().unwrap();
    assert_eq!(file_path, "secret.txt");
}

#[cfg(unix)]
fn indexed_ignored_workspace(tmp: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let repo = tmp.join("repo");
    let home = tmp.join("home");
    let locked = repo.join("locked");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::create_dir(&locked).unwrap();
    std::fs::write(repo.join(".gitignore"), "secret.txt\n").unwrap();
    std::fs::write(repo.join("secret.txt"), "hidden-secret-string\n").unwrap();
    std::fs::write(locked.join("visible.txt"), "public data\n").unwrap();

    let mut add = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    add.current_dir(&repo)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .env("RAYON_NUM_THREADS", "2")
        .env("TOKIO_WORKER_THREADS", "2")
        .args(["--add", ".", "--hash", "--no-watch"])
        .assert()
        .success();
    (repo, home)
}

#[cfg(unix)]
fn skip_gitignore_search(repo: &std::path::Path, home: &std::path::Path, literal: bool) -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    command
        .current_dir(repo)
        .env("IVYGREP_HOME", home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .env("RAYON_NUM_THREADS", "2")
        .env("TOKIO_WORKER_THREADS", "2")
        .env("RUST_LOG", "off")
        .args([
            "--skip-gitignore",
            "--no-watch",
            "--file-name-only",
            "--json",
            "--hash",
            "hidden-secret-string",
        ]);
    if literal {
        command.arg("--literal");
    }
    command
}

#[cfg(unix)]
fn assert_failed_skip_gitignore_reindex_returns_error_and_retries(use_daemon: bool) {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Child, Stdio};
    use std::time::{Duration, Instant};

    struct RestorePermissions(std::path::PathBuf, fs::Permissions);
    impl Drop for RestorePermissions {
        fn drop(&mut self) {
            fs::set_permissions(&self.0, self.1.clone()).unwrap();
        }
    }
    struct Daemon(Child);
    impl Drop for Daemon {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    let tmp = tempfile::tempdir_in("/tmp").unwrap();
    let (repo, home) = indexed_ignored_workspace(tmp.path());
    let locked = repo.join("locked");
    let restore = RestorePermissions(locked.clone(), fs::metadata(&locked).unwrap().permissions());
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read_dir(&locked).is_ok() {
        return;
    }

    let _daemon = if use_daemon {
        let child = std::process::Command::new(assert_cmd::cargo::cargo_bin!("ig"))
            .current_dir(&repo)
            .env("IVYGREP_HOME", &home)
            .env("IVYGREP_NO_AUTOSPAWN", "1")
            .env("RAYON_NUM_THREADS", "2")
            .env("TOKIO_WORKER_THREADS", "2")
            .arg("--daemon")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut daemon = Daemon(child);
        let deadline = Instant::now() + Duration::from_secs(20);
        while !home.join("daemon.sock").exists() {
            assert!(
                daemon.0.try_wait().unwrap().is_none(),
                "daemon exited early"
            );
            assert!(Instant::now() < deadline, "daemon did not become ready");
            std::thread::sleep(Duration::from_millis(25));
        }
        Some(daemon)
    } else {
        None
    };

    for literal in [true, false] {
        let failed = skip_gitignore_search(&repo, &home, literal)
            .assert()
            .failure();
        assert!(
            String::from_utf8_lossy(&failed.get_output().stderr).contains("Permission denied"),
            "{failed:?}"
        );
        skip_gitignore_search(&repo, &home, literal)
            .assert()
            .failure();
    }

    drop(restore);
    for literal in [true, false] {
        let result = skip_gitignore_search(&repo, &home, literal)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let files: Vec<String> = serde_json::from_slice(&result).unwrap();
        assert_eq!(files, ["secret.txt"]);
    }
}

#[cfg(unix)]
#[test]
fn failed_local_skip_gitignore_reindex_returns_error_and_retries() {
    assert_failed_skip_gitignore_reindex_returns_error_and_retries(false);
}

#[cfg(unix)]
#[test]
fn failed_daemon_skip_gitignore_reindex_returns_error_and_retries() {
    assert_failed_skip_gitignore_reindex_returns_error_and_retries(true);
}

#[cfg(unix)]
#[test]
fn disconnected_daemon_does_not_fall_back_to_an_incomplete_index() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;

    let tmp = tempfile::tempdir_in("/tmp").unwrap();
    let (repo, home) = indexed_ignored_workspace(tmp.path());

    for literal in [true, false] {
        let socket = home.join("daemon.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let daemon = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = String::new();
            BufReader::new(&stream).read_line(&mut request).unwrap();
            let request: serde_json::Value = serde_json::from_str(&request).unwrap();
            assert_eq!(request["type"], "version");
            drop(listener);
            std::fs::remove_file(socket).unwrap();
            writeln!(
                stream,
                "{{\"type\":\"version\",\"version\":\"{}\"}}",
                env!("CARGO_PKG_VERSION")
            )
            .unwrap();
        });

        let failed = skip_gitignore_search(&repo, &home, literal)
            .assert()
            .failure();
        assert!(
            String::from_utf8_lossy(&failed.get_output().stderr)
                .contains("the --skip-gitignore index is incomplete"),
            "{failed:?}"
        );
        daemon.join().unwrap();
    }

    let output = skip_gitignore_search(&repo, &home, true)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let files: Vec<String> = serde_json::from_slice(&output).unwrap();
    assert_eq!(files, ["secret.txt"]);
}
