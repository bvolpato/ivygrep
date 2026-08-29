use std::collections::BTreeMap;

use serial_test::serial;
use tempfile::{TempDir, tempdir};

use crate::embedding::HashEmbeddingModel;
use crate::search::{SearchOptions, literal_search};

use super::*;

struct RecoveryFixture {
    _home: TempDir,
    root: TempDir,
    workspace: Workspace,
    model: HashEmbeddingModel,
}

impl RecoveryFixture {
    fn new() -> Self {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let root = tempdir().unwrap();
        for (path, text) in [
            ("stable.rs", "pub fn stable_recovery_marker() {}\n"),
            ("changed.rs", "pub fn original_recovery_marker() {}\n"),
            ("ignored.rs", "pub fn ignored_recovery_marker() {}\n"),
            ("removed.rs", "pub fn removed_recovery_marker() {}\n"),
            (".gitignore", "ignored.rs\n"),
        ] {
            fs::write(root.path().join(path), text).unwrap();
        }
        let workspace = Workspace::resolve(root.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        workspace
            .write_metadata(&WorkspaceMetadata {
                id: workspace.id.clone(),
                root: workspace.root.clone(),
                created_at_unix: 17,
                last_indexed_at_unix: None,
                watch_enabled: true,
                skip_gitignore: true,
                index_generation: 0,
            })
            .unwrap();
        let model = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &model).unwrap();
        Self {
            _home: home,
            root,
            workspace,
            model,
        }
    }

    fn edit(&self) {
        fs::write(
            self.root.path().join("changed.rs"),
            "pub fn updated_recovery_marker() {}\n",
        )
        .unwrap();
    }
}

fn stored_texts(workspace: &Workspace) -> BTreeMap<String, Vec<u8>> {
    let sqlite = open_sqlite_readonly(&workspace.sqlite_path()).unwrap();
    let mut statement = sqlite
        .prepare("SELECT file_path, text FROM chunks ORDER BY file_path")
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn assert_visible(workspace: &Workspace, marker: &str, file: &str) {
    let hits = literal_search(
        workspace,
        marker,
        &SearchOptions {
            skip_gitignore: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        hits.iter().any(|hit| hit.file_path == Path::new(file)),
        "{marker} missing after recovery: {hits:?}"
    );
}

#[test]
#[serial]
fn corrupt_store_recovery_rebuilds_all_sources_and_preserves_settings() {
    for recovery_reason in ["hash", "tantivy", "incomplete"] {
        let fixture = RecoveryFixture::new();
        let workspace = &fixture.workspace;
        enhance_workspace_neural(workspace, &fixture.model).unwrap();
        let before = workspace.read_metadata().unwrap().unwrap();
        let mut expected_files = stored_texts(workspace).into_keys().collect::<BTreeSet<_>>();
        fixture.edit();
        fs::remove_file(fixture.root.path().join("removed.rs")).unwrap();
        expected_files.remove("removed.rs");

        let summary = if recovery_reason == "tantivy" {
            fs::write(
                workspace.tantivy_dir().join("meta.json"),
                "corrupt metadata",
            )
            .unwrap();
            // A targeted refresh must still discover the entire repository
            // once its backing stores cannot be reused.
            fs::write(
                fixture.root.path().join("unreported.rs"),
                "pub fn unreported_recovery_marker() {}\n",
            )
            .unwrap();
            expected_files.insert("unreported.rs".into());
            index_workspace_paths_for_watcher(workspace, &fixture.model, &["changed.rs".into()])
                .unwrap()
        } else if recovery_reason == "hash" {
            fs::write(workspace.vector_path(), "corrupt vector store").unwrap();
            index_workspace(workspace, &fixture.model).unwrap()
        } else {
            // Exercise fresh-store planning when a snapshot survives an
            // incomplete run, without the outer health check clearing it.
            let mut incomplete = before.clone();
            incomplete.last_indexed_at_unix = None;
            workspace.write_metadata(&incomplete).unwrap();
            index_workspace_inner(workspace, &fixture.model, false, None, true, false).unwrap()
        };

        assert_eq!(
            stored_texts(workspace).into_keys().collect::<BTreeSet<_>>(),
            expected_files,
            "corruption recovery replayed only the old incremental delta"
        );
        assert_visible(workspace, "stable_recovery_marker", "stable.rs");
        assert_visible(workspace, "updated_recovery_marker", "changed.rs");
        assert_visible(workspace, "ignored_recovery_marker", "ignored.rs");
        let (index, _) = open_tantivy_index(&workspace.tantivy_dir()).unwrap();
        assert_eq!(
            index.reader().unwrap().searcher().num_docs() as usize,
            summary.total_chunks
        );
        assert_eq!(
            summary.total_chunks,
            count_chunks(&workspace.sqlite_path()).unwrap()
        );
        assert_eq!(
            MerkleSnapshot::load(&workspace.merkle_snapshot_path()).unwrap(),
            MerkleSnapshot::build(&workspace.root, true).unwrap()
        );
        let after = workspace.read_metadata().unwrap().unwrap();
        assert_eq!(after.created_at_unix, before.created_at_unix);
        assert_eq!(after.watch_enabled, before.watch_enabled);
        assert_eq!(after.skip_gitignore, before.skip_gitignore);
        assert_eq!(after.index_generation, before.index_generation + 1);
        assert!(!workspace.vector_neural_path().exists());
        assert!(!workspace.neural_profile_path().exists());
        assert!(!workspace.neural_enhanced_generation_path().exists());
        assert!(workspace.index_health().is_queryable());

        let snapshot = fs::read(workspace.merkle_snapshot_path()).unwrap();
        let noop = index_workspace(workspace, &fixture.model).unwrap();
        assert_eq!(noop.indexed_files + noop.deleted_files, 0);
        assert_eq!(noop.total_chunks, summary.total_chunks);
        assert_eq!(
            fs::read(workspace.merkle_snapshot_path()).unwrap(),
            snapshot
        );
        assert_eq!(
            workspace.read_metadata().unwrap().unwrap().index_generation,
            after.index_generation
        );
        assert_visible(workspace, "stable_recovery_marker", "stable.rs");
    }
}

#[test]
#[serial]
fn interrupted_corrupt_store_recovery_preserves_live_generation_and_retries() {
    let fixture = RecoveryFixture::new();
    let workspace = &fixture.workspace;
    let stored = stored_texts(workspace);
    let snapshot = fs::read(workspace.merkle_snapshot_path()).unwrap();
    let metadata = fs::read(workspace.metadata_path()).unwrap();
    let tantivy_metadata = fs::read(workspace.tantivy_dir().join("meta.json")).unwrap();
    fs::write(workspace.neural_tombstones_path(), "42\n").unwrap();
    fs::write(workspace.vector_path(), "corrupt vector store").unwrap();
    fixture.edit();

    // The index-directory scope covers the staged Tantivy index. Its empty
    // initialization succeeds; the actual staged commit fails after SQLite.
    let failure = fail_tantivy_commits(&workspace.index_dir);
    for _ in 0..2 {
        let error = index_workspace(workspace, &fixture.model).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("injected Tantivy metadata publication failure"),
            "{error:#}"
        );
        assert_eq!(stored_texts(workspace), stored);
        assert_eq!(fs::read(workspace.metadata_path()).unwrap(), metadata);
        assert_eq!(
            fs::read(workspace.merkle_snapshot_path()).unwrap(),
            snapshot
        );
        assert_eq!(
            fs::read(workspace.tantivy_dir().join("meta.json")).unwrap(),
            tantivy_metadata
        );
        assert_eq!(
            fs::read_to_string(workspace.neural_tombstones_path()).unwrap(),
            "42\n"
        );
        assert_visible(workspace, "stable_recovery_marker", "stable.rs");
    }
    drop(failure);

    index_workspace(workspace, &fixture.model).unwrap();
    assert_eq!(stored_texts(workspace).len(), stored.len());
    assert_visible(workspace, "stable_recovery_marker", "stable.rs");
    assert_visible(workspace, "updated_recovery_marker", "changed.rs");
    assert!(!workspace.neural_tombstones_path().exists());
    assert!(workspace.index_health().is_queryable());
    assert_eq!(
        index_workspace(workspace, &fixture.model)
            .unwrap()
            .indexed_files,
        0
    );
}
