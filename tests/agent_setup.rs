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

    stub_client(&bin, "cursor");

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
fn install_writes_the_config_that_client_variables_select() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&bin).unwrap();
    for (client, variable, config) in [
        ("codex", "CODEX_HOME", "config.toml"),
        ("claude", "CLAUDE_CONFIG_DIR", ".claude.json"),
    ] {
        stub_client(&bin, client);
        let config_dir = temp.path().join(variable);
        fs::create_dir_all(&config_dir).unwrap();

        ig_command(temp.path(), &["agent", "install", client])
            .current_dir(&home)
            // An empty value does not select a home directory.
            .env("IVYGREP_AGENT_HOME", "")
            .env(variable, &config_dir)
            .assert()
            .success();
        let written = fs::read_to_string(config_dir.join(config)).unwrap();
        assert!(written.contains("--mcp"), "{client}: {written}");
        assert_eq!(fs::read_dir(&home).unwrap().count(), 0, "{client}");
    }
}

#[test]
fn doctor_fails_when_a_detected_client_has_an_invalid_config() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let bin = temp.path().join("bin");
    fs::create_dir_all(home.join(".cursor")).unwrap();
    fs::create_dir_all(&bin).unwrap();
    stub_client(&bin, "cursor");
    let ig = |args: &[&str]| ig_command(temp.path(), args);
    ig(&["agent", "install", "cursor"]).assert().success();
    for source in ["{broken", ""] {
        fs::write(home.join(".claude.json"), source).unwrap();

        ig(&["agent", "doctor"])
            .assert()
            .failure()
            .stdout(predicates::str::contains("Cursor: configured"))
            .stdout(predicates::str::contains("Agent setup healthy").not())
            .stderr(predicates::str::contains("invalid config for Claude Code"));
    }
}

#[test]
fn cursor_config_with_comments_is_checked_but_not_edited() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let bin = temp.path().join("bin");
    let config = home.join(".cursor").join("mcp.json");
    fs::create_dir_all(home.join(".cursor")).unwrap();
    fs::create_dir_all(&bin).unwrap();
    stub_client(&bin, "cursor");
    let ig = |args: &[&str]| ig_command(temp.path(), args);
    let executable = serde_json::to_string(assert_cmd::cargo::cargo_bin!("ig")).unwrap();
    let commented = format!(
        r#"// The "servers" of this host
{{
  "mcpServers": {{
    "ig": {{"command": {executable}, "args": ["--mcp"]}}, /* local, */
    "docs": {{"url": "https://example.com/mcp", "headers": {{"note": "a \" // b"}}}}, // remote
  }},
}}
"#
    );
    fs::write(&config, &commented).unwrap();

    ig(&["agent", "doctor"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Cursor: configured"));
    ig(&["agent", "install", "cursor"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("comments or trailing commas"));
    assert_eq!(fs::read_to_string(&config).unwrap(), commented);

    fs::write(&config, format!("{commented}}}")).unwrap();
    ig(&["agent", "doctor"])
        .assert()
        .failure()
        .stdout(predicates::str::contains("config invalid"));
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

/// `ig` with each configuration home inside `temp`.
fn ig_command(temp: &std::path::Path, args: &[&str]) -> Command {
    let home = temp.join("home");
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    command
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("CLAUDE_CONFIG_DIR", home.join(".claude"))
        .env("CODEX_HOME", home.join(".codex"))
        .env("IVYGREP_AGENT_HOME", &home)
        .env("IVYGREP_HOME", temp.join("ivygrep"))
        .env("PATH", test_path(&temp.join("bin")))
        .args(args);
    command
}

fn stub_client(bin: &std::path::Path, name: &str) {
    #[cfg(unix)]
    let client = bin.join(name);
    #[cfg(windows)]
    let client = bin.join(format!("{name}.exe"));
    fs::write(&client, "").unwrap();
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&client).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&client, permissions).unwrap();
    }
}

fn test_path(bin: &std::path::Path) -> std::ffi::OsString {
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![PathBuf::from(bin)];
    paths.extend(std::env::split_paths(&existing));
    std::env::join_paths(paths).unwrap()
}
