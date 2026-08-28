use serial_test::serial;

use ivygrep::embedding::create_hash_model;
use ivygrep::indexer::{
    index_workspace, index_workspace_for_watcher, index_workspace_paths_for_watcher,
};
use ivygrep::search::{SearchOptions, hybrid_search};
use ivygrep::workspace::Workspace;

#[test]
#[serial]
fn watcher_reindex_does_not_short_circuit_when_watcher_is_alive() {
    let home = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

    let repo = tempfile::tempdir().unwrap();
    std::fs::write(
        repo.path().join("lib.rs"),
        "pub fn initial_marker() -> bool { true }\n",
    )
    .unwrap();

    let workspace = Workspace::resolve(repo.path()).unwrap();
    let model = create_hash_model();
    index_workspace(&workspace, model.as_ref()).unwrap();

    let _ = ivygrep::jobs::start_job(&workspace, ivygrep::jobs::JobKind::Watcher, "idle", 1);
    std::fs::write(
        repo.path().join("lib.rs"),
        "pub fn updated_marker() -> bool { true }\n",
    )
    .unwrap();

    let summary = index_workspace_for_watcher(&workspace, model.as_ref()).unwrap();
    assert!(
        summary.indexed_files >= 1,
        "watch-triggered indexing should process the changed file"
    );

    let hits = hybrid_search(
        &workspace,
        "updated marker",
        Some(model.as_ref()),
        &SearchOptions {
            limit: Some(5),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        hits.iter()
            .any(|hit| hit.preview.contains("updated_marker")),
        "updated file contents should be searchable after watcher reindex, got {hits:#?}"
    );
}

#[test]
#[serial]
fn watcher_path_delta_updates_searchable_content() {
    let home = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

    let repo = tempfile::tempdir().unwrap();
    std::fs::write(
        repo.path().join("lib.rs"),
        "pub fn initial_marker() -> bool { true }\n",
    )
    .unwrap();

    let workspace = Workspace::resolve(repo.path()).unwrap();
    let model = create_hash_model();
    index_workspace(&workspace, model.as_ref()).unwrap();

    std::fs::write(
        repo.path().join("lib.rs"),
        "pub fn targeted_delta_marker() -> bool { true }\n",
    )
    .unwrap();
    let summary =
        index_workspace_paths_for_watcher(&workspace, model.as_ref(), &["lib.rs".into()]).unwrap();
    assert_eq!(summary.indexed_files, 1);

    let hits = hybrid_search(
        &workspace,
        "targeted delta marker",
        Some(model.as_ref()),
        &SearchOptions {
            limit: Some(5),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        hits.iter()
            .any(|hit| hit.preview.contains("targeted_delta_marker")),
        "targeted watcher delta should update searchable contents, got {hits:#?}"
    );
}

#[cfg(unix)]
#[test]
#[serial]
fn watcher_path_delta_excludes_replacement_symlinks() {
    let home = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let repo = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let source = repo.path().join("lib.rs");
    std::fs::write(&source, "pub fn original_marker() {}\n").unwrap();
    let target = outside.path().join("external.rs");
    std::fs::write(&target, "pub fn external_marker() {}\n").unwrap();
    let workspace = Workspace::resolve(repo.path()).unwrap();
    let model = create_hash_model();
    index_workspace(&workspace, model.as_ref()).unwrap();

    std::fs::remove_file(&source).unwrap();
    std::os::unix::fs::symlink(&target, &source).unwrap();
    let summary =
        index_workspace_paths_for_watcher(&workspace, model.as_ref(), &["lib.rs".into()]).unwrap();
    assert_eq!(summary.indexed_files, 0);
    assert_eq!(summary.deleted_files, 1);
    assert_eq!(summary.total_chunks, 0);
    let snapshot =
        ivygrep::merkle::MerkleSnapshot::load(&workspace.merkle_snapshot_path()).unwrap();
    assert_eq!(
        snapshot,
        ivygrep::merkle::MerkleSnapshot::build(&workspace.root, false).unwrap()
    );
}

#[cfg(unix)]
#[test]
#[serial]
fn watcher_path_delta_excludes_symlinked_parent_directories() {
    let home = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let repo = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(repo.path().join("src")).unwrap();
    std::fs::write(
        repo.path().join("src/lib.rs"),
        "pub fn original_marker() {}\n",
    )
    .unwrap();
    std::fs::write(
        outside.path().join("lib.rs"),
        "pub fn external_marker() {}\n",
    )
    .unwrap();
    let workspace = Workspace::resolve(repo.path()).unwrap();
    let model = create_hash_model();
    index_workspace(&workspace, model.as_ref()).unwrap();

    std::fs::rename(repo.path().join("src"), outside.path().join("previous")).unwrap();
    std::os::unix::fs::symlink(outside.path(), repo.path().join("src")).unwrap();
    let summary =
        index_workspace_paths_for_watcher(&workspace, model.as_ref(), &["src/lib.rs".into()])
            .unwrap();
    assert_eq!(summary.indexed_files, 0);
    assert_eq!(summary.deleted_files, 1);
    assert_eq!(summary.total_chunks, 0);
}

#[cfg(unix)]
fn assert_unreadable_directory_preserves_index(event_path: Option<&str>) {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    struct RestorePermissions(std::path::PathBuf, fs::Permissions);
    impl Drop for RestorePermissions {
        fn drop(&mut self) {
            fs::set_permissions(&self.0, self.1.clone()).unwrap();
        }
    }

    fn chunks(workspace: &Workspace) -> Vec<(String, Vec<u8>)> {
        ivygrep::indexer::open_sqlite_readonly(&workspace.sqlite_path())
            .unwrap()
            .prepare("SELECT file_path, text FROM chunks ORDER BY chunk_key")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    fn lexical_count(workspace: &Workspace) -> u64 {
        tantivy::Index::open_in_dir(workspace.tantivy_dir())
            .unwrap()
            .reader()
            .unwrap()
            .searcher()
            .num_docs()
    }

    let home = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let repo = tempfile::tempdir().unwrap();
    let locked = repo.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::write(locked.join("kept.rs"), "pub fn retained_marker() {}\n").unwrap();
    fs::write(
        repo.path().join("other.rs"),
        "pub fn original_marker() {}\n",
    )
    .unwrap();
    let workspace = Workspace::resolve(repo.path()).unwrap();
    let model = create_hash_model();
    index_workspace(&workspace, model.as_ref()).unwrap();
    let old_snapshot = fs::read(workspace.merkle_snapshot_path()).unwrap();
    let old_chunks = chunks(&workspace);
    let old_lexical_count = lexical_count(&workspace);
    let old_lexical_metadata = fs::read(workspace.tantivy_dir().join("meta.json")).unwrap();
    let old_generation = workspace.read_metadata().unwrap().unwrap().index_generation;
    assert_eq!(old_chunks.len(), 2);
    assert_eq!(old_lexical_count, 2);
    fs::write(repo.path().join("other.rs"), "pub fn updated_marker() {}\n").unwrap();

    let restore = RestorePermissions(locked.clone(), fs::metadata(&locked).unwrap().permissions());
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read_dir(&locked).is_ok() {
        return; // Privileged users can bypass Unix permission bits.
    }
    assert_eq!(
        fs::symlink_metadata(locked.join("kept.rs"))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
    let result = match event_path {
        Some(path) => index_workspace_paths_for_watcher(
            &workspace,
            model.as_ref(),
            &["other.rs".into(), path.into()],
        ),
        None => index_workspace_for_watcher(&workspace, model.as_ref()),
    };
    let error = result.unwrap_err();
    assert!(format!("{error:#}").contains("locked"), "{error:#}");
    assert_eq!(chunks(&workspace), old_chunks);
    assert_eq!(lexical_count(&workspace), old_lexical_count);
    assert_eq!(
        fs::read(workspace.tantivy_dir().join("meta.json")).unwrap(),
        old_lexical_metadata
    );
    assert_eq!(
        fs::read(workspace.merkle_snapshot_path()).unwrap(),
        old_snapshot
    );
    assert_eq!(
        workspace.read_metadata().unwrap().unwrap().index_generation,
        old_generation
    );

    drop(restore);
    let recovered = index_workspace_for_watcher(&workspace, model.as_ref()).unwrap();
    assert_eq!(recovered.indexed_files, 1);
    assert_eq!(recovered.deleted_files, 0);
    assert_eq!(chunks(&workspace).len(), 2);
    let hits = hybrid_search(
        &workspace,
        "retained_marker",
        Some(model.as_ref()),
        &SearchOptions::default(),
    )
    .unwrap();
    assert!(
        hits.iter()
            .any(|hit| hit.file_path == std::path::Path::new("locked/kept.rs"))
    );

    fs::remove_file(locked.join("kept.rs")).unwrap();
    let deleted =
        index_workspace_paths_for_watcher(&workspace, model.as_ref(), &["locked/kept.rs".into()])
            .unwrap();
    assert_eq!(deleted.deleted_files, 1);
    assert_eq!(chunks(&workspace).len(), 1);
    assert_eq!(lexical_count(&workspace), 1);
    assert_eq!(
        ivygrep::merkle::MerkleSnapshot::load(&workspace.merkle_snapshot_path()).unwrap(),
        ivygrep::merkle::MerkleSnapshot::build(&workspace.root, false).unwrap()
    );
}

#[cfg(unix)]
#[test]
#[serial]
fn unreadable_directory_full_scan_preserves_index() {
    assert_unreadable_directory_preserves_index(None);
}

#[cfg(unix)]
#[test]
#[serial]
fn unreadable_child_path_delta_preserves_index() {
    assert_unreadable_directory_preserves_index(Some("locked/kept.rs"));
}

#[cfg(unix)]
#[test]
#[serial]
fn unreadable_directory_path_delta_preserves_index() {
    assert_unreadable_directory_preserves_index(Some("locked"));
}
