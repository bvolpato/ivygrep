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
