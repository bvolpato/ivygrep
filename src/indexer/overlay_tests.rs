use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use serial_test::serial;
use tempfile::tempdir;

use super::*;
use crate::embedding::HashEmbeddingModel;
use crate::search::{SearchOptions, literal_search};

#[derive(Clone, Copy)]
enum PublicationFailure {
    Tantivy,
    Completion,
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn linked_workspace(parent: &Path) -> (PathBuf, PathBuf) {
    let base = parent.join("base");
    let linked = parent.join("linked");
    fs::create_dir_all(&base).unwrap();
    fs::create_dir_all(&linked).unwrap();
    git(&base, &["init", "--quiet", "-b", "main"]);

    // Keep the fixture unborn without requiring newer Git worktree flags.
    // Use a relative gitfile and Git's own path spelling for the backlink,
    // never canonical Windows verbatim paths from Path::display().
    let git_root = git(&base, &["rev-parse", "--show-toplevel"]);
    let git_parent = Path::new(git_root.trim()).parent().unwrap();
    let admin = base.join(".git/worktrees/linked");
    fs::create_dir_all(&admin).unwrap();
    fs::write(admin.join("commondir"), "../..\n").unwrap();
    fs::write(admin.join("HEAD"), "ref: refs/heads/linked\n").unwrap();
    fs::write(
        admin.join("gitdir"),
        format!("{}/linked/.git\n", git_parent.display()),
    )
    .unwrap();
    fs::write(
        linked.join(".git"),
        "gitdir: ../base/.git/worktrees/linked\n",
    )
    .unwrap();
    git(&linked, &["worktree", "list", "--porcelain"]);
    for (path, content) in [
        ("lib.rs", "pub fn base_overlay_marker() {}\n"),
        ("stable.rs", "pub fn shared_overlay_marker() {}\n"),
        ("inherited.rs", "pub fn inherited_overlay_marker() {}\n"),
        ("removed.rs", "pub fn removed_base_marker() {}\n"),
    ] {
        fs::write(base.join(path), content).unwrap();
        fs::write(linked.join(path), content).unwrap();
    }
    fs::write(linked.join("lib.rs"), "pub fn old_overlay_marker() {}\n").unwrap();
    fs::remove_file(linked.join("removed.rs")).unwrap();
    (base, linked)
}

fn stored_state(workspace: &Workspace, overlay: bool) -> BTreeMap<PathBuf, Vec<u8>> {
    let (sqlite, tantivy, vectors) = if overlay {
        (
            workspace.overlay_sqlite_path(),
            workspace.overlay_tantivy_dir(),
            workspace.overlay_vector_path(),
        )
    } else {
        (
            workspace.sqlite_path(),
            workspace.tantivy_dir(),
            workspace.vector_path(),
        )
    };
    let mut paths = vec![
        workspace.metadata_path(),
        workspace.merkle_snapshot_path(),
        workspace.index_format_version_path(),
        sqlite,
        vectors,
    ];
    if overlay {
        paths.push(workspace.base_ref_path());
    }
    if tantivy.is_dir() {
        paths.extend(
            fs::read_dir(tantivy)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| {
                    path.is_file()
                        && !path
                            .file_name()
                            .unwrap()
                            .to_string_lossy()
                            .ends_with(".lock")
                }),
        );
    }
    paths
        .into_iter()
        .filter(|path| path.is_file())
        .map(|path| {
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}

fn assert_state_preserved(
    workspace: &Workspace,
    overlay: bool,
    expected: &BTreeMap<PathBuf, Vec<u8>>,
) {
    let actual = stored_state(workspace, overlay);
    let changed = actual
        .keys()
        .chain(expected.keys())
        .filter(|path| actual.get(*path) != expected.get(*path))
        .collect::<BTreeSet<_>>();
    assert!(changed.is_empty(), "stored artifacts changed: {changed:?}");
}

fn assert_overlay_publication_recovers(recreated: bool, failure_stage: PublicationFailure) {
    let home = tempdir().unwrap();
    unsafe {
        std::env::set_var("IVYGREP_HOME", home.path());
    }
    let parent = tempdir().unwrap();
    let (base_root, linked_root) = linked_workspace(&parent.path().canonicalize().unwrap());
    let base = Workspace::resolve(&base_root).unwrap();
    let workspace = Workspace::resolve(&linked_root).unwrap();
    assert!(
        workspace.is_worktree(),
        "fixture was not discovered as a linked worktree: {workspace:?}"
    );
    assert_eq!(workspace.main_worktree_root().unwrap(), base.root);
    let model = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
    index_workspace_for_watcher(&base, &model).unwrap();
    if recreated {
        index_workspace_for_watcher(&workspace, &model).unwrap();
        assert_eq!(
            literal_search(&workspace, "old_overlay_marker", &SearchOptions::default())
                .unwrap()
                .len(),
            1
        );
    }

    // The recreation must replay both divergent files, even though only lib.rs
    // changed in the linked worktree since its last successful index.
    fs::write(
        base_root.join("stable.rs"),
        "pub fn newer_base_marker() {}\n",
    )
    .unwrap();
    index_workspace_for_watcher(&base, &model).unwrap();
    fs::write(
        linked_root.join("lib.rs"),
        "pub fn recovered_overlay_marker() {}\n",
    )
    .unwrap();
    workspace.ensure_dirs().unwrap();
    let unrelated = workspace.index_dir.join("unrelated-state");
    fs::write(&unrelated, "preserve me").unwrap();
    let prior_overlay = stored_state(&workspace, true);
    let prior_base = stored_state(&base, false);
    let generation = workspace
        .read_metadata()
        .unwrap()
        .map(|metadata| metadata.index_generation)
        .unwrap_or(0);
    let tantivy_failure = matches!(failure_stage, PublicationFailure::Tantivy)
        .then(|| fail_tantivy_commits(&workspace.index_dir));
    let completion_failure = matches!(failure_stage, PublicationFailure::Completion)
        .then(|| staging::test_support::fail_publication(&workspace.metadata_path()));
    let expected_error = match failure_stage {
        PublicationFailure::Tantivy => "injected Tantivy metadata publication failure",
        PublicationFailure::Completion => "injected workspace publication failure",
    };

    for _ in 0..2 {
        let error = index_workspace_for_watcher(&workspace, &model).unwrap_err();
        assert!(format!("{error:#}").contains(expected_error), "{error:#}");
        if recreated {
            assert_state_preserved(&workspace, true, &prior_overlay);
        } else {
            assert!(!workspace.has_overlay());
            assert!(!workspace.base_ref_path().exists());
            assert!(!workspace.merkle_snapshot_path().exists());
            assert!(
                workspace
                    .read_metadata()
                    .unwrap()
                    .unwrap()
                    .last_indexed_at_unix
                    .is_none()
            );
        }
        assert_state_preserved(&base, false, &prior_base);
        assert_eq!(fs::read_to_string(&unrelated).unwrap(), "preserve me");
    }
    drop(tantivy_failure);
    drop(completion_failure);

    let recovered = index_workspace_for_watcher(&workspace, &model).unwrap();
    assert_eq!(recovered.indexed_files, 2);
    assert_eq!(recovered.deleted_files, 1);
    assert_eq!(
        workspace.read_metadata().unwrap().unwrap().index_generation,
        generation + 1
    );
    let sqlite = open_sqlite_readonly(&workspace.overlay_sqlite_path()).unwrap();
    assert_eq!(
        sqlite
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        sqlite
            .query_row(
                "SELECT COUNT(*) FROM tombstones WHERE file_path = 'removed.rs'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    drop(sqlite);
    let (index, _) = open_tantivy_index(&workspace.overlay_tantivy_dir()).unwrap();
    assert_eq!(index.reader().unwrap().searcher().num_docs(), 2);
    for marker in [
        "recovered_overlay_marker",
        "shared_overlay_marker",
        "inherited_overlay_marker",
    ] {
        assert_eq!(
            literal_search(&workspace, marker, &SearchOptions::default())
                .unwrap()
                .len(),
            1,
            "{marker}"
        );
    }
    for marker in [
        "removed_base_marker",
        "newer_base_marker",
        "old_overlay_marker",
    ] {
        assert!(
            literal_search(&workspace, marker, &SearchOptions::default())
                .unwrap()
                .is_empty(),
            "{marker}"
        );
    }
    assert_eq!(
        MerkleSnapshot::load(&workspace.merkle_snapshot_path()).unwrap(),
        MerkleSnapshot::build(&workspace.root, false).unwrap()
    );
    assert!(!workspace.worktree_overlay_is_stale().unwrap());
    assert!(workspace.index_health().is_queryable());
    assert_state_preserved(&base, false, &prior_base);
    assert_eq!(fs::read_to_string(unrelated).unwrap(), "preserve me");
    let retry = index_workspace_for_watcher(&workspace, &model).unwrap();
    assert_eq!(retry.indexed_files + retry.deleted_files, 0);
}

#[test]
#[serial]
fn fresh_overlay_retries_failed_tantivy_publication() {
    assert_overlay_publication_recovers(false, PublicationFailure::Tantivy);
}

#[test]
#[serial]
fn recreated_overlay_retries_failed_tantivy_publication() {
    assert_overlay_publication_recovers(true, PublicationFailure::Tantivy);
}

#[test]
#[serial]
fn fresh_overlay_rolls_back_failed_completion_publication() {
    assert_overlay_publication_recovers(false, PublicationFailure::Completion);
}

#[test]
#[serial]
fn recreated_overlay_rolls_back_failed_completion_publication() {
    assert_overlay_publication_recovers(true, PublicationFailure::Completion);
}
