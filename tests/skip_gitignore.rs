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
#[test]
fn failed_skip_gitignore_reindex_returns_error_and_retries() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    struct RestorePermissions(std::path::PathBuf, fs::Permissions);
    impl Drop for RestorePermissions {
        fn drop(&mut self) {
            fs::set_permissions(&self.0, self.1.clone()).unwrap();
        }
    }

    let tmp = tempdir().unwrap();
    let repo = tmp.path().join("repo");
    let home = tmp.path().join("home");
    let locked = repo.join("locked");
    fs::create_dir_all(repo.join(".git")).unwrap();
    fs::create_dir(&locked).unwrap();
    fs::write(repo.join(".gitignore"), "secret.txt\n").unwrap();
    fs::write(repo.join("secret.txt"), "hidden-secret-string\n").unwrap();
    fs::write(locked.join("visible.txt"), "public data\n").unwrap();

    let mut add = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    add.current_dir(&repo)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--add", ".", "--hash", "--no-watch"])
        .assert()
        .success();

    let restore = RestorePermissions(locked.clone(), fs::metadata(&locked).unwrap().permissions());
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read_dir(&locked).is_ok() {
        return;
    }

    let search = || {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
        command
            .current_dir(&repo)
            .env("IVYGREP_HOME", &home)
            .env("IVYGREP_NO_AUTOSPAWN", "1")
            .args([
                "--literal",
                "--skip-gitignore",
                "--no-watch",
                "--file-name-only",
                "--json",
                "hidden-secret-string",
            ]);
        command
    };

    let failed = search().assert().failure();
    assert!(
        String::from_utf8_lossy(&failed.get_output().stderr).contains("Permission denied"),
        "{failed:?}"
    );
    search().assert().failure();

    drop(restore);
    let result = search().assert().success().get_output().stdout.clone();
    let files: Vec<String> = serde_json::from_slice(&result).unwrap();
    assert_eq!(files, ["secret.txt"]);
}
