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

pub(super) fn linked_workspace(parent: &Path) -> (PathBuf, PathBuf) {
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

pub(super) fn stored_state(workspace: &Workspace, overlay: bool) -> BTreeMap<PathBuf, Vec<u8>> {
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
fn incremental_overlay_recovers_when_ignore_edit_is_reverted_after_partial_commit() {
    // Cover both a stale deletion marker and a missing deletion marker.
    for initially_visible in [true, false] {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let parent = tempdir().unwrap();
        let (base_root, linked_root) = linked_workspace(&parent.path().canonicalize().unwrap());
        let workspace = Workspace::resolve(&linked_root).unwrap();
        let model = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        index_workspace_for_watcher(&workspace, &model).unwrap();
        let exclude = base_root.join(".git/info/exclude");
        let initial_rule = if initially_visible { "" } else { "stable.rs\n" };
        if !initially_visible {
            fs::write(&exclude, initial_rule).unwrap();
            index_workspace_for_watcher(&workspace, &model).unwrap();
        }
        let snapshot = fs::read(workspace.merkle_snapshot_path()).unwrap();
        let attempted_rule = if initially_visible { "stable.rs\n" } else { "" };
        fs::write(&exclude, attempted_rule).unwrap();

        let failure = fail_tantivy_commits(&workspace.index_dir);
        let error = index_workspace_for_watcher(&workspace, &model).unwrap_err();
        assert!(format!("{error:#}").contains("injected Tantivy metadata publication failure"));
        assert!(workspace.quick_index_health().needs_rebuild());
        assert_eq!(
            fs::read(workspace.merkle_snapshot_path()).unwrap(),
            snapshot
        );
        // SQLite has committed, but Tantivy and the source snapshot have not.
        let sqlite = open_sqlite_readonly(&workspace.overlay_sqlite_path()).unwrap();
        assert_eq!(
            sqlite
                .query_row(
                    "SELECT COUNT(*) FROM tombstones WHERE file_path = 'stable.rs'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            if initially_visible { 1 } else { 0 }
        );
        drop(sqlite);
        drop(failure);

        // The input now matches the old snapshot. A normal Merkle diff is empty,
        // but the partially committed deletion state still needs to be repaired.
        fs::write(&exclude, initial_rule).unwrap();
        index_workspace_for_watcher(&workspace, &model).unwrap();
        assert!(!workspace.indexing_incomplete_path().exists());
        assert_eq!(
            literal_search(
                &workspace,
                "shared_overlay_marker",
                &SearchOptions::default()
            )
            .unwrap()
            .len(),
            if initially_visible { 1 } else { 0 }
        );
        assert_eq!(
            MerkleSnapshot::load(&workspace.merkle_snapshot_path()).unwrap(),
            MerkleSnapshot::build(&workspace.root, false).unwrap()
        );
    }
}

#[test]
#[serial]
fn fresh_overlay_rolls_back_failed_completion_publication() {
    assert_overlay_publication_recovers(false, PublicationFailure::Completion);
}

#[test]
fn initial_overlay_diff_uses_persisted_base_coverage() {
    let parent = tempdir().unwrap();
    let base = parent.path().join("base");
    let linked = parent.path().join("linked");
    fs::create_dir_all(&base).unwrap();
    fs::create_dir_all(&linked).unwrap();
    for root in [&base, &linked] {
        fs::write(root.join("shared.rs"), "pub fn shared() {}\n").unwrap();
    }
    let live_base = MerkleSnapshot::build_content_based(&base, false).unwrap();
    let worktree = MerkleSnapshot::build_content_based(&linked, false).unwrap();
    assert_eq!(live_base, worktree);
    // The live checkouts agree, but the persisted base did not index this file.
    let diff = initial_overlay_diff(&base, &MerkleSnapshot::empty(), &live_base, &worktree);
    assert_eq!(
        diff.added_or_modified,
        vec![(PathBuf::from("shared.rs"), false)]
    );

    let indexed_base = MerkleSnapshot::build(&base, false).unwrap();
    for root in [&base, &linked] {
        fs::remove_file(root.join("shared.rs")).unwrap();
    }
    let live_base = MerkleSnapshot::build_content_based(&base, false).unwrap();
    let worktree = MerkleSnapshot::build_content_based(&linked, false).unwrap();
    let diff = initial_overlay_diff(&base, &indexed_base, &live_base, &worktree);
    assert_eq!(diff.deleted, vec![PathBuf::from("shared.rs")]);
}

#[test]
fn initial_overlay_diff_does_not_inherit_changed_base_metadata() {
    let parent = tempdir().unwrap();
    let base = parent.path().join("base");
    let linked = parent.path().join("linked");
    fs::create_dir_all(&base).unwrap();
    fs::create_dir_all(&linked).unwrap();
    fs::write(base.join("shared.rs"), "pub fn before() {}\n").unwrap();
    let indexed_base = MerkleSnapshot::build(&base, false).unwrap();
    for root in [&base, &linked] {
        fs::write(root.join("shared.rs"), "pub fn after() {}\n").unwrap();
    }
    let live_base = MerkleSnapshot::build_content_based(&base, false).unwrap();
    let worktree = MerkleSnapshot::build_content_based(&linked, false).unwrap();
    assert_eq!(live_base, worktree);
    let diff = initial_overlay_diff(&base, &indexed_base, &live_base, &worktree);
    assert_eq!(
        diff.added_or_modified,
        vec![(PathBuf::from("shared.rs"), false)]
    );
}

#[test]
#[serial]
fn base_overlay_state_waits_for_coherent_publication() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let root = tempdir().unwrap();
    fs::write(root.path().join("before.rs"), "pub fn before() {}\n").unwrap();
    let workspace = Workspace::resolve(root.path()).unwrap();
    workspace.ensure_dirs().unwrap();
    workspace
        .write_metadata(&WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: 1,
            last_indexed_at_unix: Some(1),
            watch_enabled: false,
            skip_gitignore: false,
            index_generation: 1,
        })
        .unwrap();
    fs::write(workspace.index_incarnation_path(), "before").unwrap();
    MerkleSnapshot::build(root.path(), false)
        .unwrap()
        .save(&workspace.merkle_snapshot_path())
        .unwrap();

    let publication_lock = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(workspace.lock_path())
        .unwrap();
    fs2::FileExt::lock_exclusive(&publication_lock).unwrap();

    let (sender, receiver) = std::sync::mpsc::channel();
    let capture_workspace = workspace.clone();
    let handle = std::thread::spawn(move || {
        sender
            .send(capture_base_overlay_state(&capture_workspace))
            .unwrap();
    });
    assert!(
        receiver
            .recv_timeout(std::time::Duration::from_millis(100))
            .is_err(),
        "base state capture must wait for publication"
    );

    fs::write(root.path().join("after.rs"), "pub fn after() {}\n").unwrap();
    let expected_snapshot = MerkleSnapshot::build(root.path(), false).unwrap();
    workspace
        .write_metadata(&WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: 1,
            last_indexed_at_unix: Some(2),
            watch_enabled: false,
            skip_gitignore: false,
            index_generation: 2,
        })
        .unwrap();
    fs::write(workspace.index_incarnation_path(), "after").unwrap();
    expected_snapshot
        .save(&workspace.merkle_snapshot_path())
        .unwrap();
    fs2::FileExt::unlock(&publication_lock).unwrap();
    let (generation, incarnation, snapshot) = receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap()
        .unwrap();
    handle.join().unwrap();
    assert_eq!(generation, 2);
    assert_eq!(incarnation, "after");
    assert_eq!(snapshot, expected_snapshot);
}

#[test]
fn overlay_snapshot_rejects_edits_between_metadata_and_content_capture() {
    let root = tempdir().unwrap();
    let path = root.path().join("shared.rs");
    fs::write(&path, "pub fn before() {}\n").unwrap();
    let metadata = MerkleSnapshot::build(root.path(), false).unwrap();
    let original = MerkleSnapshot::build_content_based(root.path(), false).unwrap();
    validate_overlay_snapshot(root.path(), &metadata, &original).unwrap();

    fs::write(&path, "pub fn changed_content() {}\n").unwrap();
    let changed = MerkleSnapshot::build_content_based(root.path(), false).unwrap();
    assert!(validate_overlay_snapshot(root.path(), &metadata, &changed).is_err());

    fs::remove_file(path).unwrap();
    let missing = MerkleSnapshot::build_content_based(root.path(), false).unwrap();
    assert!(validate_overlay_snapshot(root.path(), &metadata, &missing).is_err());
}

#[test]
#[serial]
fn recreated_overlay_rolls_back_failed_completion_publication() {
    assert_overlay_publication_recovers(true, PublicationFailure::Completion);
}
