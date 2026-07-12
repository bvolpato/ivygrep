use std::fs;
use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn cursor_install_preserves_config_and_doctor_runs_real_search() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let bin = temp.path().join("bin");
    fs::create_dir_all(home.join(".cursor")).unwrap();
    fs::create_dir_all(&bin).unwrap();
    fs::write(
        home.join(".cursor").join("mcp.json"),
        r#"{"theme":"dark","mcpServers":{"other":{"command":"other","args":[]}}}"#,
    )
    .unwrap();

    #[cfg(unix)]
    let client = bin.join("cursor");
    #[cfg(windows)]
    let client = bin.join("cursor.exe");
    fs::write(&client, "").unwrap();
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&client).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&client, permissions).unwrap();
    }

    let path = test_path(&bin);
    let mut install = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    install
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("IVYGREP_AGENT_HOME", &home)
        .env("PATH", &path)
        .args(["agent", "install", "cursor"])
        .assert()
        .success()
        .stdout(predicates::str::contains("MCP handshake: 2025-11-25"))
        .stdout(predicates::str::contains("Real search: 1 result(s)"));

    let config_path = home.join(".cursor").join("mcp.json");
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
    assert_eq!(config["theme"], "dark");
    assert_eq!(config["mcpServers"]["other"]["command"], "other");
    assert_eq!(
        config["mcpServers"]["ig"]["args"],
        serde_json::json!(["--mcp"])
    );

    let mut repeat = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    repeat
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("IVYGREP_AGENT_HOME", &home)
        .env("PATH", &path)
        .args(["agent", "install", "cursor"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Config already current Cursor"));

    let mut doctor = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    doctor
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("IVYGREP_AGENT_HOME", &home)
        .env("PATH", &path)
        .args(["agent", "doctor"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Cursor: configured"))
        .stdout(predicates::str::contains("Tool discovery: ig_search"));
}

#[test]
fn agent_help_is_focused() {
    let temp = tempfile::tempdir().unwrap();
    let blocked_home = temp.path().join("not-a-directory");
    fs::write(&blocked_home, "blocked").unwrap();

    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .env("IVYGREP_HOME", blocked_home)
        .args(["agent", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("install"))
        .stdout(predicates::str::contains("doctor"))
        .stdout(predicates::str::contains("--type").not());
}

#[test]
fn missing_client_prints_exact_remediation() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&bin).unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("IVYGREP_AGENT_HOME", &home)
        .env("PATH", &bin)
        .args(["agent", "install", "claude"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "Claude Code not detected. Install Claude Code, then rerun `ig agent install claude`.",
        ));
}

fn test_path(bin: &std::path::Path) -> std::ffi::OsString {
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![PathBuf::from(bin)];
    paths.extend(std::env::split_paths(&existing));
    std::env::join_paths(paths).unwrap()
}
