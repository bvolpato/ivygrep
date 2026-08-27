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
