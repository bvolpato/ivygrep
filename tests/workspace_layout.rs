//! Local CLI recovery when the checkout at a tracked path changes its Git role.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tempfile::tempdir;

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "commit.gpgSign=false"])
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
}

fn init_repo(root: &Path, marker: &str) {
    fs::create_dir_all(root).unwrap();
    git(root, &["init", "-b", "main"]);
    fs::write(root.join("source.rs"), format!("pub fn {marker}() {{}}\n")).unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "initial source"]);
}

fn cli(home: &Path, root: &Path, args: &[&str]) -> Value {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("ig"))
        .current_dir(root)
        .env("IVYGREP_HOME", home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .env("IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT", "1")
        .args(["--hash", "--no-watch", "--skip-gitignore", "--json"])
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "ig {args:?}: {output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn saved_index(home: &Path, root: &Path) -> PathBuf {
    let root = root.canonicalize().unwrap();
    fs::read_dir(home.join("indexes"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|index| {
            let metadata = fs::read(index.join("workspace.json")).unwrap();
            let metadata: Value = serde_json::from_slice(&metadata).unwrap();
            metadata["root"].as_str() == root.to_str()
        })
        .unwrap()
}

fn metadata(index: &Path) -> Value {
    serde_json::from_slice(&fs::read(index.join("workspace.json")).unwrap()).unwrap()
}

fn assert_source(home: &Path, root: &Path, marker: &str) {
    let hits = cli(home, root, &["--literal", marker]);
    assert_eq!(hits.as_array().unwrap().len(), 1, "{hits}");
    assert_eq!(hits[0]["file_path"], "source.rs");
    assert!(
        hits[0]["hits"][0]["preview"]
            .as_str()
            .unwrap()
            .contains(marker)
    );
}

fn assert_settings_preserved(index: &Path, before: &Value) {
    let after = metadata(index);
    for field in [
        "id",
        "root",
        "created_at_unix",
        "watch_enabled",
        "skip_gitignore",
    ] {
        assert_eq!(after[field], before[field], "changed {field}");
    }
    assert!(after["last_indexed_at_unix"].is_number());
}

#[test]
fn local_cli_recovers_overlay_replaced_by_main_or_plain_directory() {
    for new_git_repo in [true, false] {
        for explicit_add in [true, false] {
            let fixture = tempdir().unwrap();
            let home = fixture.path().join("home");
            let main = fixture.path().join("main");
            let checkout = fixture.path().join("checkout");
            init_repo(&main, "old_layout_marker");
            git(
                &main,
                &[
                    "worktree",
                    "add",
                    "-b",
                    "linked",
                    checkout.to_str().unwrap(),
                ],
            );
            cli(&home, &checkout, &["--add", "."]);
            let index = saved_index(&home, &checkout);
            let before = metadata(&index);
            assert!(index.join("base_ref.json").exists());

            let retired = fixture.path().join("retired");
            git(
                &main,
                &[
                    "worktree",
                    "move",
                    checkout.to_str().unwrap(),
                    retired.to_str().unwrap(),
                ],
            );
            if new_git_repo {
                init_repo(&checkout, "new_layout_marker");
            } else {
                fs::create_dir(&checkout).unwrap();
                fs::write(
                    checkout.join("source.rs"),
                    "pub fn new_layout_marker() {}\n",
                )
                .unwrap();
            }
            if explicit_add {
                cli(&home, &checkout, &["--add", "."]);
            }
            assert_source(&home, &checkout, "new_layout_marker");
            assert_eq!(
                cli(&home, &checkout, &["--literal", "old_layout_marker"]),
                serde_json::json!([])
            );
            assert!(index.join("metadata.sqlite3").exists());
            assert!(!index.join("base_ref.json").exists());
            assert!(!index.join("overlay.sqlite3").exists());
            assert_settings_preserved(&index, &before);
            assert_source(&home, &main, "old_layout_marker");
        }
    }
}

#[test]
fn local_cli_recovers_main_replaced_by_linked_worktree() {
    for explicit_add in [false, true] {
        let fixture = tempdir().unwrap();
        let home = fixture.path().join("home");
        let checkout = fixture.path().join("checkout");
        init_repo(&checkout, "old_layout_marker");
        cli(&home, &checkout, &["--add", "."]);
        let index = saved_index(&home, &checkout);
        let before = metadata(&index);
        assert!(index.join("metadata.sqlite3").exists());

        fs::rename(&checkout, fixture.path().join("retired")).unwrap();
        let main = fixture.path().join("main");
        init_repo(&main, "new_layout_marker");
        git(
            &main,
            &[
                "worktree",
                "add",
                "-b",
                "linked",
                checkout.to_str().unwrap(),
            ],
        );
        if explicit_add {
            cli(&home, &checkout, &["--add", "."]);
        }
        assert_source(&home, &checkout, "new_layout_marker");
        assert_eq!(
            cli(&home, &checkout, &["--literal", "old_layout_marker"]),
            serde_json::json!([])
        );
        assert!(index.join("base_ref.json").exists());
        assert!(index.join("overlay.sqlite3").exists());
        assert!(!index.join("metadata.sqlite3").exists());
        assert!(!index.join("tantivy").exists());
        assert!(!index.join("vectors.usearch").exists());
        assert_settings_preserved(&index, &before);
        assert_source(&home, &main, "new_layout_marker");
    }
}
