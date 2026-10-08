use std::{fs, path::Path, process::Command};

use ivygrep::{
    EMBEDDING_DIMENSIONS,
    embedding::HashEmbeddingModel,
    indexer::index_workspace_for_watcher,
    search::{SearchOptions, literal_search},
    workspace::Workspace,
};
use serial_test::serial;
use tempfile::tempdir;

fn git(root: &Path, args: &[&str]) {
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
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_clean(root: &Path) {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty(), "expected a clean checkout");
}

fn assert_found(workspace: &Workspace, query: &str, expected: bool) {
    let hits = literal_search(workspace, query, &SearchOptions::default()).unwrap();
    assert_eq!(!hits.is_empty(), expected, "{query}: {hits:?}");
}

#[test]
#[serial]
fn git_reuse_retains_clean_checkout_shortcut_with_exclusion_only_rules() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let root = tempdir().unwrap();
    git(root.path(), &["init", "-b", "main"]);
    fs::write(root.path().join("lib.rs"), "pub fn unchanged_marker() {}\n").unwrap();
    fs::write(
        root.path().join(".ignore"),
        "# ! is only a comment\nexcluded/\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "initial"]);
    let workspace = Workspace::resolve(root.path()).unwrap();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    index_workspace_for_watcher(&workspace, &model).unwrap();
    let state = fs::read(workspace.index_dir.join("indexed_git_state")).unwrap();
    let generation = workspace.read_metadata().unwrap().unwrap().index_generation;
    let stats = index_workspace_for_watcher(&workspace, &model).unwrap();
    assert_eq!(stats.indexed_files, 0);
    assert_eq!(
        fs::read(workspace.index_dir.join("indexed_git_state")).unwrap(),
        state
    );
    assert_eq!(
        workspace.read_metadata().unwrap().unwrap().index_generation,
        generation
    );
    assert_found(&workspace, "unchanged_marker", true);
}

#[test]
#[serial]
fn git_reuse_retains_clean_checkout_shortcut_beside_ignored_directories() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let root = tempdir().unwrap();
    git(root.path(), &["init", "-b", "main"]);
    fs::write(root.path().join("lib.rs"), "pub fn unchanged_marker() {}\n").unwrap();
    fs::write(root.path().join(".gitignore"), "deps/\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "initial"]);
    // An ignored directory with its own ignore file and a nested repository,
    // as a dependency checkout or an agent worktree leaves behind.
    let nested = root.path().join("deps/library");
    fs::create_dir_all(&nested).unwrap();
    fs::write(root.path().join("deps/.gitignore"), "*.o\n").unwrap();
    git(&nested, &["init", "-b", "main"]);
    fs::write(nested.join("source.rs"), "pub fn dependency_marker() {}\n").unwrap();
    assert_clean(root.path());

    let workspace = Workspace::resolve(root.path()).unwrap();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    index_workspace_for_watcher(&workspace, &model).unwrap();
    let state = fs::read(workspace.index_dir.join("indexed_git_state")).unwrap();
    let generation = workspace.read_metadata().unwrap().unwrap().index_generation;

    // The walker does not enter `deps/`, so its ignore file is not an input.
    fs::write(root.path().join("deps/.gitignore"), "*.obj\n").unwrap();
    let stats = index_workspace_for_watcher(&workspace, &model).unwrap();
    assert_eq!(stats.indexed_files, 0);
    assert_eq!(
        fs::read(workspace.index_dir.join("indexed_git_state")).unwrap(),
        state
    );
    assert_eq!(
        workspace.read_metadata().unwrap().unwrap().index_generation,
        generation
    );
    assert_found(&workspace, "unchanged_marker", true);
    assert_found(&workspace, "dependency_marker", false);
}

/// Puts a `git` first in `PATH` that records each call and then runs Git.
#[cfg(unix)]
struct RecordedGit {
    path: Option<std::ffi::OsString>,
    log: std::path::PathBuf,
    _directory: tempfile::TempDir,
}

#[cfg(unix)]
impl RecordedGit {
    fn install() -> Self {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::var_os("PATH");
        let git = std::env::split_paths(path.as_deref().unwrap_or_default())
            .map(|directory| directory.join("git"))
            .find(|candidate| candidate.is_file())
            .expect("git is in PATH");
        let directory = tempdir().unwrap();
        let log = directory.path().join("calls");
        let shim = directory.path().join("git");
        fs::write(
            &shim,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
                log.display(),
                git.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
        let mut directories = vec![directory.path().to_path_buf()];
        directories.extend(std::env::split_paths(path.as_deref().unwrap_or_default()));
        unsafe { std::env::set_var("PATH", std::env::join_paths(directories).unwrap()) };
        Self {
            path,
            log,
            _directory: directory,
        }
    }

    fn calls(&self) -> Vec<String> {
        fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }
}

#[cfg(unix)]
impl Drop for RecordedGit {
    fn drop(&mut self) {
        match &self.path {
            Some(path) => unsafe { std::env::set_var("PATH", path) },
            None => unsafe { std::env::remove_var("PATH") },
        }
    }
}

#[cfg(unix)]
#[test]
#[serial]
fn git_reuse_starts_six_git_processes_for_an_unchanged_checkout() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let root = tempdir().unwrap();
    git(root.path(), &["init", "-b", "main"]);
    fs::write(root.path().join("lib.rs"), "pub fn unchanged_marker() {}\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "initial"]);
    let workspace = Workspace::resolve(root.path()).unwrap();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    index_workspace_for_watcher(&workspace, &model).unwrap();

    // Five processes read the state. One checks that it did not change.
    let recorded = RecordedGit::install();
    let stats = index_workspace_for_watcher(&workspace, &model).unwrap();
    let calls = recorded.calls();
    drop(recorded);
    assert_eq!(stats.indexed_files, 0);
    assert_eq!(calls.len(), 6, "{calls:#?}");

    // A changed file needs the same six, around the index update.
    fs::write(root.path().join("lib.rs"), "pub fn changed_marker() {}\n").unwrap();
    git(root.path(), &["commit", "-am", "change"]);
    let recorded = RecordedGit::install();
    let stats = index_workspace_for_watcher(&workspace, &model).unwrap();
    let calls = recorded.calls();
    drop(recorded);
    assert_eq!(stats.indexed_files, 1);
    assert_eq!(calls.len(), 6, "{calls:#?}");
    assert_found(&workspace, "changed_marker", true);
}

#[test]
#[serial]
fn git_reuse_observes_ancestor_ignore_changes() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let parent = tempdir().unwrap();
    let root = parent.path().join("repo");
    fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    fs::write(root.join("a.rs"), "pub fn ancestor_rule_marker() {}\n").unwrap();
    fs::write(root.join("b.rs"), "pub fn stable_marker() {}\n").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);
    let workspace = Workspace::resolve(&root).unwrap();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    for control in [".ignore", ".gitignore"] {
        fs::write(parent.path().join(control), "a.rs\n").unwrap();
        index_workspace_for_watcher(&workspace, &model).unwrap();
        assert_found(&workspace, "ancestor_rule_marker", false);
        fs::remove_file(parent.path().join(control)).unwrap();
        assert_clean(&root);
        index_workspace_for_watcher(&workspace, &model).unwrap();
        assert_found(&workspace, "ancestor_rule_marker", true);
        assert_found(&workspace, "stable_marker", true);
    }
}

#[test]
#[serial]
fn git_reuse_observes_whitelisted_untracked_sources_and_base_reuse() {
    for nested in [false, true] {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let parent = tempdir().unwrap();
        let root = parent.path().join("repo");
        fs::create_dir(&root).unwrap();
        git(&root, &["init", "-b", "main"]);
        let directory = if nested {
            root.join("generated")
        } else {
            root.clone()
        };
        fs::create_dir_all(&directory).unwrap();
        fs::write(root.join(".gitignore"), "*.rs\n").unwrap();
        fs::write(directory.join(".ignore"), "!*.rs\n").unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "ignore policy"]);
        let source = directory.join("generated.rs");
        fs::write(&source, "pub fn original_generated_marker() {}\n").unwrap();
        let workspace = Workspace::resolve(&root).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace_for_watcher(&workspace, &model).unwrap();
        assert_found(&workspace, "original_generated_marker", true);
        fs::write(&source, "pub fn updated_generated_marker() {}\n").unwrap();
        assert_clean(&root);
        index_workspace_for_watcher(&workspace, &model).unwrap();
        assert_found(&workspace, "updated_generated_marker", true);
        fs::write(
            directory.join("added.rs"),
            "pub fn added_generated_marker() {}\n",
        )
        .unwrap();
        index_workspace_for_watcher(&workspace, &model).unwrap();
        assert_found(&workspace, "added_generated_marker", true);

        // A new worktree must not bless stale base contents as current merely
        // because Git does not report the whitelisted generated file.
        fs::write(&source, "pub fn inherited_generated_marker() {}\n").unwrap();
        let linked = parent.path().join("linked");
        git(
            &root,
            &[
                "worktree",
                "add",
                "--detach",
                linked.to_str().unwrap(),
                "HEAD",
            ],
        );
        let linked_directory = if nested {
            linked.join("generated")
        } else {
            linked.clone()
        };
        fs::write(
            linked_directory.join("generated.rs"),
            "pub fn inherited_generated_marker() {}\n",
        )
        .unwrap();
        let overlay = Workspace::resolve(&linked).unwrap();
        index_workspace_for_watcher(&overlay, &model).unwrap();
        assert_found(&workspace, "inherited_generated_marker", true);
        assert_found(&overlay, "inherited_generated_marker", true);
    }
}

#[test]
#[serial]
fn git_reuse_observes_assume_unchanged_and_present_skip_worktree_files() {
    for flag in ["--assume-unchanged", "--skip-worktree"] {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let root = tempdir().unwrap();
        git(root.path(), &["init", "-b", "main"]);
        let source = root.path().join("lib.rs");
        fs::write(&source, "pub fn initial_flag_marker() {}\n").unwrap();
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-m", "initial"]);
        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace_for_watcher(&workspace, &model).unwrap();
        git(root.path(), &["update-index", flag, "lib.rs"]);
        fs::write(&source, "pub fn updated_flag_marker() {}\n").unwrap();
        assert_clean(root.path());
        index_workspace_for_watcher(&workspace, &model).unwrap();
        assert_found(&workspace, "updated_flag_marker", true);
        fs::write(&source, "pub fn next_flag_marker() {}\n").unwrap();
        index_workspace_for_watcher(&workspace, &model).unwrap();
        assert_found(&workspace, "next_flag_marker", true);
    }
}

#[test]
#[serial]
fn git_reuse_observes_edits_hidden_by_submodule_ignore_settings() {
    for ignore in ["dirty", "all"] {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let parent = tempdir().unwrap();
        let upstream = parent.path().join("upstream");
        let root = parent.path().join("repo");
        for directory in [&upstream, &root] {
            fs::create_dir(directory).unwrap();
            git(directory, &["init", "-b", "main"]);
        }
        fs::write(
            upstream.join("lib.rs"),
            "pub fn initial_submodule_marker() {}\n",
        )
        .unwrap();
        git(&upstream, &["add", "."]);
        git(&upstream, &["commit", "-m", "initial"]);
        fs::write(root.join("main.rs"), "pub fn superproject_marker() {}\n").unwrap();
        git(
            &root,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                upstream.to_str().unwrap(),
                "vendor/sub",
            ],
        );
        git(
            &root,
            &[
                "config",
                "-f",
                ".gitmodules",
                "submodule.vendor/sub.ignore",
                ignore,
            ],
        );
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "add submodule"]);
        let workspace = Workspace::resolve(&root).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        index_workspace_for_watcher(&workspace, &model).unwrap();
        assert_found(&workspace, "initial_submodule_marker", true);

        // The walker indexes submodule sources even when these settings hide
        // their edits from the superproject's default status.
        fs::write(
            root.join("vendor/sub/lib.rs"),
            "pub fn updated_submodule_marker() {}\n",
        )
        .unwrap();
        assert_clean(&root);
        index_workspace_for_watcher(&workspace, &model).unwrap();
        assert_found(&workspace, "updated_submodule_marker", true);
    }
}
