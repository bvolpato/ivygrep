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

fn indexed_roots(home: &Path) -> Vec<PathBuf> {
    fs::read_dir(home.join("indexes"))
        .unwrap()
        .filter_map(|entry| fs::read(entry.unwrap().path().join("workspace.json")).ok())
        .map(|raw| serde_json::from_slice::<Value>(&raw).unwrap())
        .map(|metadata| PathBuf::from(metadata["root"].as_str().unwrap()))
        .collect()
}

fn clone_with_separate_git_dir(top: &Path) {
    init_repo(&top.join("upstream"), "upstream_marker");
    let git_dir = top.join("separate.git");
    git(
        top,
        &[
            "clone",
            "--separate-git-dir",
            git_dir.to_str().unwrap(),
            "upstream",
            "separate",
        ],
    );
}

#[test]
fn submodules_and_separate_git_dirs_are_standalone_workspaces() {
    let fixture = tempdir().unwrap();
    let top = fixture.path();
    clone_with_separate_git_dir(top);
    let parent = top.join("parent");
    init_repo(&parent, "parent_marker");
    git(
        &parent,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "../upstream",
            "vendor/sub",
        ],
    );
    let mut roots = Vec::new();
    for checkout in [parent.join("vendor/sub"), top.join("separate")] {
        let name = format!("worktree-{}", roots.len());
        let worktree = top.join(&name);
        git(
            &checkout,
            &["worktree", "add", "-b", &name, worktree.to_str().unwrap()],
        );
        roots.extend([checkout, worktree]);
    }

    for root in roots {
        let home = tempdir().unwrap();
        // The second search reads the index that the first one built.
        assert_source(home.path(), &root, "upstream_marker");
        assert_source(home.path(), &root, "upstream_marker");
        assert_eq!(indexed_roots(home.path()), [root.canonicalize().unwrap()]);
        let status = cli(home.path(), &root, &["--status"]);
        assert!(status[0]["base_repo_root"].is_null(), "{status}");
    }
}

#[test]
fn local_cli_drops_a_git_directory_that_was_the_base_of_its_checkout() {
    let fixture = tempdir().unwrap();
    let top = fixture.path();
    clone_with_separate_git_dir(top);
    let home = tempdir().unwrap();
    let home = home.path();
    let git_dir = top.join("separate.git");
    let separate = top.join("separate");
    cli(home, top, &["--add", "separate.git"]);
    cli(home, &separate, &["--add", "."]);
    fs::write(
        saved_index(home, &separate).join("base_ref.json"),
        serde_json::json!({
            "base_index_dir": saved_index(home, &git_dir),
            "base_workspace_root": git_dir.canonicalize().unwrap(),
            "base_generation": 1,
        })
        .to_string(),
    )
    .unwrap();

    assert_source(home, &separate, "upstream_marker");
    assert_eq!(indexed_roots(home), [separate.canonicalize().unwrap()]);
}
