use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
}

impl Fixture {
    fn new(git: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repo");
        std::fs::create_dir(&root).unwrap();
        let fixture = Self {
            root,
            home: temp.path().join("home"),
            _temp: temp,
        };
        if git {
            fixture.git(&["init", "-q", "-b", "main"]);
        }
        std::fs::write(
            fixture.root.join("main.py"),
            "from private_credentials import authentication_value\ndef authentication():\n    return authentication_value\n",
        )
        .unwrap();
        fixture
    }

    fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
            ])
            .args(args)
            .current_dir(&self.root)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }

    fn command(&self) -> Command {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
        command
            .current_dir(&self.root)
            .env("IVYGREP_HOME", &self.home)
            .env("IVYGREP_NO_AUTOSPAWN", "1")
            .env("HF_HUB_OFFLINE", "1")
            .env_remove("RUST_LOG");
        command
    }

    fn context(&self, task: &str, skip_ignored: bool) -> Value {
        let mut command = self.command();
        command.args(["context", task, "--hash", "--json"]);
        if skip_ignored {
            command.arg("--skip-gitignore");
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "{output:?}");
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn mcp_context(&self, skip_ignored: bool) -> Value {
        let mut child = self
            .command()
            .arg("--mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        writeln!(
            child.stdin.take().unwrap(),
            "{}",
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "ig_search", "arguments": {
                    "query": "fix authentication", "path": self.root,
                    "output": "context_pack", "skip_gitignore": skip_ignored,
                }}
            })
        )
        .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{output:?}");
        let response: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_ne!(response["result"]["isError"], true, "{response}");
        response["result"]["structuredContent"]["context_pack"].clone()
    }
}

fn contains_private_source(bundle: &Value) -> bool {
    bundle["items"].as_array().unwrap().iter().any(|item| {
        item["preview"]
            .as_str()
            .unwrap()
            .contains("excluded_context_marker")
    })
}

#[test]
fn context_excludes_ignore_file_seeds_in_cli_and_mcp() {
    let fixture = Fixture::new(true);
    std::fs::write(fixture.root.join(".ignore"), "private_credentials.py\n").unwrap();
    fixture.git(&["add", "."]);
    fixture.git(&["commit", "-qm", "fixture"]);
    std::fs::write(
        fixture.root.join("private_credentials.py"),
        "authentication_value = 'excluded_context_marker'\n",
    )
    .unwrap();

    for bundle in [
        fixture.context("fix authentication", false),
        fixture.mcp_context(false),
    ] {
        assert!(!contains_private_source(&bundle), "{bundle}");
        assert_eq!(bundle["change_scope"]["total_changes"], 0);
    }
    assert!(contains_private_source(
        &fixture.context("fix authentication", true)
    ));
    assert!(contains_private_source(&fixture.mcp_context(true)));
}

#[test]
fn context_excludes_explicit_paths_and_live_dependencies_without_git() {
    let fixture = Fixture::new(false);
    std::fs::write(fixture.root.join(".gitignore"), "private_credentials.py\n").unwrap();
    std::fs::write(
        fixture.root.join("private_credentials.py"),
        "authentication_value = 'excluded_context_marker'\n",
    )
    .unwrap();
    for task in [
        "fix authentication in private_credentials.py",
        "fix authentication in main.py",
    ] {
        let bundle = fixture.context(task, false);
        assert!(!contains_private_source(&bundle), "{bundle}");
        assert!(
            !bundle["referenced_paths"]
                .as_array()
                .unwrap()
                .iter()
                .any(|path| {
                    path["file_path"].as_str().map(Path::new)
                        == Some(Path::new("private_credentials.py"))
                })
        );
    }
    assert!(contains_private_source(
        &fixture.context("fix authentication in private_credentials.py", true)
    ));
}

#[test]
#[cfg(unix)]
fn context_applies_ignore_rules_to_deleted_tracked_files() {
    let fixture = Fixture::new(true);
    std::fs::write(
        fixture.root.join("private_credentials.py"),
        "authentication_value = 'excluded_context_marker'\n",
    )
    .unwrap();
    fixture.git(&["add", "."]);
    fixture.git(&["commit", "-qm", "fixture"]);
    std::fs::write(fixture.root.join(".ignore"), "private_credentials.py\n").unwrap();
    std::fs::remove_file(fixture.root.join("private_credentials.py")).unwrap();
    assert!(!contains_private_source(
        &fixture.context("fix authentication", false)
    ));
    assert!(contains_private_source(
        &fixture.context("fix authentication", true)
    ));
}
