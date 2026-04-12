use serial_test::serial;

use ivygrep::embedding::create_hash_model;
use ivygrep::indexer::{index_workspace, index_workspace_for_watcher};
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
