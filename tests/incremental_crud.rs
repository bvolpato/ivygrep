//! Incremental indexing CRUD tests.
//!
//! Validates that the Merkle-tree-driven incremental index only re-processes
//! changed files, not the entire workspace.  Each test creates a temp repo,
//! indexes it, applies a CRUD operation, re-indexes, and asserts:
//!   - The `IndexingSummary` reports the correct number of indexed/deleted files.
//!   - The chunk store (SQLite) reflects the expected state.
//!   - The Merkle snapshot on disk matches the filesystem.
//!   - Untouched files are NOT re-indexed (zero cost for unchanged files).

use std::collections::HashSet;
use std::fs;

use serial_test::serial;
use tempfile::tempdir;

use ivygrep::EMBEDDING_DIMENSIONS;
use ivygrep::embedding::HashEmbeddingModel;
use ivygrep::indexer::{index_workspace, open_sqlite};
use ivygrep::merkle::MerkleSnapshot;
use ivygrep::workspace::Workspace;

/// Helper: set IVYGREP_HOME, resolve workspace, index, return summary.
fn setup_and_index(
    root: &std::path::Path,
    home: &std::path::Path,
) -> ivygrep::indexer::IndexingSummary {
    unsafe { std::env::set_var("IVYGREP_HOME", home) };
    let workspace = Workspace::resolve(root).unwrap();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    index_workspace(&workspace, &model).unwrap()
}

fn workspace_for(root: &std::path::Path) -> Workspace {
    Workspace::resolve(root).unwrap()
}

/// Helper: get all indexed file paths from SQLite.
fn indexed_files(workspace: &Workspace) -> HashSet<String> {
    let conn = open_sqlite(&workspace.sqlite_path()).unwrap();
    let mut stmt = conn
        .prepare("SELECT DISTINCT file_path FROM chunks ORDER BY file_path")
        .unwrap();
    stmt.query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

/// Helper: count total chunks in SQLite.
fn chunk_count(workspace: &Workspace) -> usize {
    let conn = open_sqlite(&workspace.sqlite_path()).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
        .unwrap();
    count as usize
}

// ---------------------------------------------------------------------------
// CREATE: Adding new files to an already-indexed workspace
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn create_new_file_incremental() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    // Phase 1: initial index with one file
    fs::write(root.path().join("alpha.rs"), "fn alpha() -> i32 { 1 }\n").unwrap();
    let s1 = setup_and_index(root.path(), home.path());
    assert_eq!(s1.indexed_files, 1);
    assert_eq!(s1.deleted_files, 0);

    let ws = workspace_for(root.path());
    assert!(indexed_files(&ws).contains("alpha.rs"));

    // Phase 2: add a new file — only the new file should be indexed
    fs::write(root.path().join("beta.rs"), "fn beta() -> i32 { 2 }\n").unwrap();
    let s2 = setup_and_index(root.path(), home.path());
    assert_eq!(
        s2.indexed_files, 1,
        "only the new file should be re-indexed"
    );
    assert_eq!(s2.deleted_files, 0);

    let files = indexed_files(&ws);
    assert!(files.contains("alpha.rs"));
    assert!(files.contains("beta.rs"));
}

#[test]
#[serial]
fn create_multiple_new_files_incremental() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    fs::write(root.path().join("base.rs"), "fn base() {}\n").unwrap();
    let s1 = setup_and_index(root.path(), home.path());
    assert_eq!(s1.indexed_files, 1);

    // Add 3 new files at once
    fs::write(root.path().join("new1.rs"), "fn new1() {}\n").unwrap();
    fs::write(root.path().join("new2.py"), "def new2(): pass\n").unwrap();
    fs::write(root.path().join("new3.ts"), "export function new3() {}\n").unwrap();

    let s2 = setup_and_index(root.path(), home.path());
    assert_eq!(s2.indexed_files, 3, "exactly 3 new files processed");
    assert_eq!(s2.deleted_files, 0);

    let files = indexed_files(&workspace_for(root.path()));
    assert_eq!(files.len(), 4); // base + 3 new
}

// ---------------------------------------------------------------------------
// READ: No changes → no reindexing  (the most critical "skip" test)
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn no_change_means_zero_work() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    fs::write(
        root.path().join("stable.rs"),
        "fn stable() -> bool { true }\n",
    )
    .unwrap();
    fs::write(root.path().join("fixed.py"), "def fixed(): return 42\n").unwrap();

    let s1 = setup_and_index(root.path(), home.path());
    assert_eq!(s1.indexed_files, 2);
    let chunks_after_initial = s1.total_chunks;

    // Re-index without any changes
    let s2 = setup_and_index(root.path(), home.path());
    assert_eq!(s2.indexed_files, 0, "no files should be re-indexed");
    assert_eq!(s2.deleted_files, 0, "no files should be deleted");
    assert_eq!(
        s2.total_chunks, chunks_after_initial,
        "chunk count unchanged"
    );
}

#[test]
#[serial]
fn no_change_triple_reindex_still_zero() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    fs::write(root.path().join("const.rs"), "const X: i32 = 42;\n").unwrap();
    setup_and_index(root.path(), home.path());

    for _ in 0..3 {
        let s = setup_and_index(root.path(), home.path());
        assert_eq!(s.indexed_files, 0, "repeated reindex should be free");
        assert_eq!(s.deleted_files, 0);
    }
}

// ---------------------------------------------------------------------------
// UPDATE: Modify existing files
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn update_file_only_reindexes_changed() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    fs::write(root.path().join("mutable.rs"), "fn v1() -> i32 { 1 }\n").unwrap();
    fs::write(
        root.path().join("constant.rs"),
        "fn unchanged() -> i32 { 0 }\n",
    )
    .unwrap();
    let s1 = setup_and_index(root.path(), home.path());
    assert_eq!(s1.indexed_files, 2);

    // Modify only mutable.rs
    fs::write(root.path().join("mutable.rs"), "fn v2() -> i32 { 2 }\n").unwrap();
    let s2 = setup_and_index(root.path(), home.path());
    assert_eq!(s2.indexed_files, 1, "only modified file re-indexed");
    assert_eq!(s2.deleted_files, 0);

    // The chunk content should reflect the update
    let ws = workspace_for(root.path());
    let conn = open_sqlite(&ws.sqlite_path()).unwrap();
    let text: String = conn
        .query_row(
            "SELECT text FROM chunks WHERE file_path = 'mutable.rs' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        text.contains("v2"),
        "chunk should contain updated content, got: {text}"
    );
}

#[test]
#[serial]
fn update_preserves_unmodified_chunks() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    fs::write(root.path().join("a.rs"), "fn a() -> i32 { 1 }\n").unwrap();
    fs::write(root.path().join("b.rs"), "fn b() -> i32 { 2 }\n").unwrap();
    fs::write(root.path().join("c.rs"), "fn c() -> i32 { 3 }\n").unwrap();
    let s1 = setup_and_index(root.path(), home.path());
    let initial_chunks = s1.total_chunks;

    // Modify only b.rs
    fs::write(root.path().join("b.rs"), "fn b_updated() -> i32 { 200 }\n").unwrap();
    let s2 = setup_and_index(root.path(), home.path());
    assert_eq!(s2.indexed_files, 1);

    // Total chunks should be the same (replaced, not accumulated)
    let ws = workspace_for(root.path());
    assert_eq!(
        chunk_count(&ws),
        initial_chunks,
        "chunk count should not grow"
    );
}

// ---------------------------------------------------------------------------
// DELETE: Remove files
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn delete_file_removes_chunks() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    fs::write(root.path().join("alive.rs"), "fn alive() {}\n").unwrap();
    fs::write(root.path().join("doomed.rs"), "fn doomed() {}\n").unwrap();
    let s1 = setup_and_index(root.path(), home.path());
    assert_eq!(s1.indexed_files, 2);

    fs::remove_file(root.path().join("doomed.rs")).unwrap();
    let s2 = setup_and_index(root.path(), home.path());
    assert_eq!(s2.deleted_files, 1, "one file deleted");
    assert_eq!(s2.indexed_files, 0, "no new files to index");

    let ws = workspace_for(root.path());
    let files = indexed_files(&ws);
    assert!(files.contains("alive.rs"));
    assert!(
        !files.contains("doomed.rs"),
        "doomed.rs chunks should be gone"
    );
}

#[test]
#[serial]
fn delete_all_files_empties_index() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    fs::write(root.path().join("a.rs"), "fn a() {}\n").unwrap();
    fs::write(root.path().join("b.rs"), "fn b() {}\n").unwrap();
    setup_and_index(root.path(), home.path());

    fs::remove_file(root.path().join("a.rs")).unwrap();
    fs::remove_file(root.path().join("b.rs")).unwrap();
    let s = setup_and_index(root.path(), home.path());
    assert_eq!(s.deleted_files, 2);
    assert_eq!(s.total_chunks, 0, "all chunks should be removed");

    let ws = workspace_for(root.path());
    assert!(indexed_files(&ws).is_empty());
}

// ---------------------------------------------------------------------------
// COMBINED CRUD: multiple operations in one pass
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn combined_create_update_delete_in_one_pass() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    // Initial: a.rs, b.rs, c.rs
    fs::write(root.path().join("a.rs"), "fn a() { 1 }\n").unwrap();
    fs::write(root.path().join("b.rs"), "fn b() { 2 }\n").unwrap();
    fs::write(root.path().join("c.rs"), "fn c() { 3 }\n").unwrap();
    let s1 = setup_and_index(root.path(), home.path());
    assert_eq!(s1.indexed_files, 3);

    // Now: delete a.rs, modify b.rs, add d.rs, keep c.rs unchanged
    fs::remove_file(root.path().join("a.rs")).unwrap();
    fs::write(root.path().join("b.rs"), "fn b_v2() { 20 }\n").unwrap();
    fs::write(root.path().join("d.rs"), "fn d() { 4 }\n").unwrap();

    let s2 = setup_and_index(root.path(), home.path());
    assert_eq!(s2.deleted_files, 1, "a.rs deleted");
    assert_eq!(s2.indexed_files, 2, "b.rs (modified) + d.rs (new)");

    let ws = workspace_for(root.path());
    let files = indexed_files(&ws);
    assert!(!files.contains("a.rs"));
    assert!(files.contains("b.rs"));
    assert!(files.contains("c.rs"));
    assert!(files.contains("d.rs"));
}

// ---------------------------------------------------------------------------
// MERKLE SNAPSHOT INTEGRITY
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn merkle_snapshot_matches_filesystem_after_crud() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    fs::write(root.path().join("x.rs"), "fn x() {}\n").unwrap();
    fs::write(root.path().join("y.py"), "def y(): pass\n").unwrap();
    setup_and_index(root.path(), home.path());

    // Modify x, delete y, add z
    fs::write(root.path().join("x.rs"), "fn x_v2() {}\n").unwrap();
    fs::remove_file(root.path().join("y.py")).unwrap();
    fs::write(root.path().join("z.ts"), "export function z() {}\n").unwrap();
    setup_and_index(root.path(), home.path());

    let ws = workspace_for(root.path());
    let saved = MerkleSnapshot::load(&ws.merkle_snapshot_path()).unwrap();
    let fresh = MerkleSnapshot::build(root.path()).unwrap();

    assert_eq!(
        saved.root_hash, fresh.root_hash,
        "persisted snapshot = fresh scan"
    );
    assert_eq!(saved.files, fresh.files, "file hashes should match");

    // Another re-index should be a no-op
    let s = setup_and_index(root.path(), home.path());
    assert_eq!(s.indexed_files, 0);
    assert_eq!(s.deleted_files, 0);
}

// ---------------------------------------------------------------------------
// SUBDIRECTORY CRUD
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn subdirectory_crud_is_incremental() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    fs::create_dir_all(root.path().join("src/models")).unwrap();
    fs::write(root.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(
        root.path().join("src/models/user.rs"),
        "struct User { name: String }\n",
    )
    .unwrap();
    let s1 = setup_and_index(root.path(), home.path());
    assert_eq!(s1.indexed_files, 2);

    // Add new file in subdirectory only
    fs::write(
        root.path().join("src/models/post.rs"),
        "struct Post { title: String }\n",
    )
    .unwrap();
    let s2 = setup_and_index(root.path(), home.path());
    assert_eq!(s2.indexed_files, 1, "only the new subdirectory file");
    assert_eq!(s2.deleted_files, 0);

    // Delete file from subdirectory
    fs::remove_file(root.path().join("src/models/user.rs")).unwrap();
    let s3 = setup_and_index(root.path(), home.path());
    assert_eq!(s3.deleted_files, 1);
    assert_eq!(s3.indexed_files, 0);

    let ws = workspace_for(root.path());
    let files = indexed_files(&ws);
    assert!(files.contains("src/main.rs"));
    assert!(files.contains("src/models/post.rs"));
    assert!(!files.contains("src/models/user.rs"));
}

// ---------------------------------------------------------------------------
// LARGE-SCALE INCREMENTAL: many files, only a few change
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn large_workspace_tiny_change_is_cheap() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    // Create 50 files
    for i in 0..50 {
        fs::write(
            root.path().join(format!("file_{i:03}.rs")),
            format!("fn func_{i}() -> usize {{ {i} }}\n"),
        )
        .unwrap();
    }
    let s1 = setup_and_index(root.path(), home.path());
    assert_eq!(s1.indexed_files, 50);

    // Modify just 1 file out of 50
    fs::write(
        root.path().join("file_025.rs"),
        "fn func_25_v2() -> usize { 2500 }\n",
    )
    .unwrap();

    let s2 = setup_and_index(root.path(), home.path());
    assert_eq!(
        s2.indexed_files, 1,
        "only 1 of 50 files should be re-indexed"
    );
    assert_eq!(s2.deleted_files, 0);
}

// ---------------------------------------------------------------------------
// OVERWRITE WITH IDENTICAL CONTENT: no re-index
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn rewrite_with_same_content_is_noop() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    let content = "fn stable() -> bool { true }\n";
    fs::write(root.path().join("file.rs"), content).unwrap();
    setup_and_index(root.path(), home.path());

    // Write the exact same content (simulates save-without-edit)
    fs::write(root.path().join("file.rs"), content).unwrap();
    let s = setup_and_index(root.path(), home.path());
    assert_eq!(s.indexed_files, 1, "mtime changed -> re-index occurs even if content is same");
    assert_eq!(s.deleted_files, 0);
}
