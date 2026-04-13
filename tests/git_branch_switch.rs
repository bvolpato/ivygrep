//! Git branch switch integration tests.
//!
//! Validates that ivygrep's indexer correctly handles git branch switching:
//!   - Content unique to a branch is searchable when that branch is checked out.
//!   - Switching to another branch removes the old content from search results.
//!   - Switching back restores the content without breaking the index.
//!
//! These tests use real `git` commands to create repos and switch branches,
//! proving that the Merkle-tree-driven incremental indexer handles the mass
//! file changes caused by `git checkout` correctly.

use std::collections::HashSet;
use std::fs;
use std::process::Command;

use serial_test::serial;
use tempfile::tempdir;

use ivygrep::EMBEDDING_DIMENSIONS;
use ivygrep::embedding::HashEmbeddingModel;
use ivygrep::indexer::{index_workspace, open_sqlite};
use ivygrep::search::{SearchOptions, hybrid_search};
use ivygrep::workspace::Workspace;

/// Run a git command in the given directory, panicking on failure.
fn git(dir: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()
        .unwrap_or_else(|e| panic!("failed to run git {:?}: {e}", args));
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

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
    let mut files = HashSet::new();
    let use_overlay = workspace.has_overlay() || workspace.base_ref_path().exists();

    if use_overlay {
        let conn = open_sqlite(&workspace.overlay_sqlite_path()).unwrap();
        let mut tombstones = HashSet::new();
        if let Ok(mut stmt) = conn.prepare("SELECT file_path FROM tombstones")
            && let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0))
        {
            for r in rows {
                tombstones.insert(r.unwrap());
            }
        }

        let mut overlay_files = HashSet::new();
        if let Ok(mut stmt) = conn.prepare("SELECT DISTINCT file_path FROM chunks")
            && let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0))
        {
            for r in rows {
                let path = r.unwrap();
                overlay_files.insert(path.clone());
                files.insert(path);
            }
        }

        let base_dir = workspace
            .base_index_dir
            .clone()
            .unwrap_or_else(|| workspace.index_dir.clone());
        let base_sqlite = base_dir.join("metadata.sqlite3");
        if let Ok(base_conn) = open_sqlite(&base_sqlite)
            && let Ok(mut stmt) = base_conn.prepare("SELECT DISTINCT file_path FROM chunks")
            && let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0))
        {
            for r in rows {
                let path = r.unwrap();
                if !tombstones.contains(&path) && !overlay_files.contains(&path) {
                    files.insert(path);
                }
            }
        }
    } else {
        let conn = open_sqlite(&workspace.sqlite_path()).unwrap();
        let mut stmt = conn
            .prepare("SELECT DISTINCT file_path FROM chunks")
            .unwrap();
        for r in stmt.query_map([], |row| row.get::<_, String>(0)).unwrap() {
            files.insert(r.unwrap());
        }
    }
    files
}

/// Helper: search for a query and return file paths in the results.
fn search_file_paths(workspace: &Workspace, query: &str) -> Vec<String> {
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    let hits = hybrid_search(workspace, query, Some(&model), &SearchOptions::default()).unwrap();
    hits.iter()
        .map(|h| h.file_path.to_string_lossy().to_string())
        .collect()
}

/// Helper: search and return hits for a specific file, including preview content.
fn search_hits_for_file(
    workspace: &Workspace,
    query: &str,
    file_name: &str,
) -> Vec<(String, String)> {
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    let hits = hybrid_search(workspace, query, Some(&model), &SearchOptions::default()).unwrap();
    hits.iter()
        .filter(|h| h.file_path.to_string_lossy().contains(file_name))
        .map(|h| (h.file_path.to_string_lossy().to_string(), h.preview.clone()))
        .collect()
}

// ---------------------------------------------------------------------------
// THE FINAL BOSS: Git branch switch → reindex → search correctness
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn git_branch_switch_updates_index_and_search_results() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    // ── Phase 1: Create a git repo with initial content on main ──

    git(root.path(), &["init", "-b", "main"]);

    fs::write(
        root.path().join("core.rs"),
        "pub fn calculate_tax(amount: f64) -> f64 { amount * 0.2 }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("utils.rs"),
        "pub fn format_currency(val: f64) -> String { format!(\"${:.2}\", val) }\n",
    )
    .unwrap();

    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "initial commit on main"]);

    // Index on main
    let s1 = setup_and_index(root.path(), home.path());
    assert_eq!(s1.indexed_files, 2, "Phase 1: two files indexed on main");

    let ws = workspace_for(root.path());
    let files = indexed_files(&ws);
    assert!(files.contains("core.rs"), "core.rs is indexed on main");
    assert!(files.contains("utils.rs"), "utils.rs is indexed on main");

    // Search should find calculate_tax
    let results = search_file_paths(&ws, "calculate_tax");
    assert!(
        results.iter().any(|p| p.contains("core.rs")),
        "Phase 1: calculate_tax found in core.rs on main"
    );

    // ── Phase 2: Create feature branch with new content, remove main content ──

    git(root.path(), &["checkout", "-b", "feature/payments"]);

    // Add a new file only on this branch
    fs::write(
        root.path().join("payments.rs"),
        "pub fn process_payment(card: &str, amount: f64) -> bool { !card.is_empty() && amount > 0.0 }\n",
    )
    .unwrap();

    // Remove core.rs on this branch
    fs::remove_file(root.path().join("core.rs")).unwrap();

    git(root.path(), &["add", "."]);
    git(
        root.path(),
        &[
            "commit",
            "-m",
            "add payments, remove core on feature branch",
        ],
    );

    // Re-index after branch switch
    let s2 = setup_and_index(root.path(), home.path());
    assert!(
        s2.indexed_files >= 1,
        "Phase 2: at least payments.rs indexed"
    );
    assert!(s2.deleted_files >= 1, "Phase 2: core.rs deleted from index");

    let files2 = indexed_files(&ws);
    assert!(
        files2.contains("payments.rs"),
        "payments.rs is indexed on feature branch"
    );
    assert!(
        !files2.contains("core.rs"),
        "core.rs should be GONE from the index on feature branch"
    );
    assert!(
        files2.contains("utils.rs"),
        "utils.rs survives (unchanged across branches)"
    );

    // Search for payments content should succeed
    let payment_results = search_file_paths(&ws, "process_payment");
    assert!(
        payment_results.iter().any(|p| p.contains("payments.rs")),
        "Phase 2: process_payment is searchable on feature branch"
    );

    // Search for calculate_tax should NOT find core.rs anymore
    let tax_results = search_file_paths(&ws, "calculate_tax");
    assert!(
        !tax_results.iter().any(|p| p.contains("core.rs")),
        "Phase 2: calculate_tax should NOT be found after branch switch removed core.rs"
    );

    // ── Phase 3: Switch back to main — content should be restored ──

    git(root.path(), &["checkout", "main"]);

    // Re-index after switching back
    let s3 = setup_and_index(root.path(), home.path());
    assert!(
        s3.indexed_files >= 1,
        "Phase 3: core.rs re-indexed on main switch-back"
    );
    assert!(
        s3.deleted_files >= 1,
        "Phase 3: payments.rs deleted from index"
    );

    let files3 = indexed_files(&ws);
    assert!(
        files3.contains("core.rs"),
        "core.rs is BACK in the index after switching to main"
    );
    assert!(
        files3.contains("utils.rs"),
        "utils.rs still present (unchanged)"
    );
    assert!(
        !files3.contains("payments.rs"),
        "payments.rs should be GONE after switching back to main"
    );

    // Search for calculate_tax should work again!
    let tax_results_back = search_file_paths(&ws, "calculate_tax");
    assert!(
        tax_results_back.iter().any(|p| p.contains("core.rs")),
        "Phase 3: calculate_tax is searchable again after switching back to main"
    );

    // Search for process_payment should NOT find payments.rs anymore
    let payment_results_back = search_file_paths(&ws, "process_payment");
    assert!(
        !payment_results_back
            .iter()
            .any(|p| p.contains("payments.rs")),
        "Phase 3: process_payment should NOT be found after switching back to main"
    );
}

#[test]
#[serial]
fn git_branch_switch_rapid_toggle_is_stable() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    // Create repo with two branches, each with distinct content
    git(root.path(), &["init", "-b", "main"]);

    fs::write(
        root.path().join("main_only.rs"),
        "pub fn main_feature() -> &'static str { \"main\" }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "main branch"]);

    git(root.path(), &["checkout", "-b", "experiment"]);
    fs::remove_file(root.path().join("main_only.rs")).unwrap();
    fs::write(
        root.path().join("experiment_only.rs"),
        "pub fn experiment_feature() -> &'static str { \"experiment\" }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "experiment branch"]);

    git(root.path(), &["checkout", "main"]);

    // Rapid toggle: main → experiment → main → experiment → main
    // Each time, re-index and verify correctness.
    for cycle in 0..3 {
        // On main
        setup_and_index(root.path(), home.path());
        let ws = workspace_for(root.path());
        let files = indexed_files(&ws);
        assert!(
            files.contains("main_only.rs"),
            "cycle {cycle}: main_only.rs present on main"
        );
        assert!(
            !files.contains("experiment_only.rs"),
            "cycle {cycle}: experiment_only.rs absent on main"
        );

        // Switch to experiment
        git(root.path(), &["checkout", "experiment"]);
        setup_and_index(root.path(), home.path());
        let files = indexed_files(&ws);
        assert!(
            !files.contains("main_only.rs"),
            "cycle {cycle}: main_only.rs absent on experiment"
        );
        assert!(
            files.contains("experiment_only.rs"),
            "cycle {cycle}: experiment_only.rs present on experiment"
        );

        // Switch back to main
        git(root.path(), &["checkout", "main"]);
    }
}

#[test]
#[serial]
fn git_branch_with_modified_content_same_filename() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    // Create repo where the same file has DIFFERENT content on different branches
    git(root.path(), &["init", "-b", "main"]);

    fs::write(
        root.path().join("config.rs"),
        "pub fn get_mode() -> &'static str { \"production_environment\" }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "production config"]);

    git(root.path(), &["checkout", "-b", "staging"]);
    fs::write(
        root.path().join("config.rs"),
        "pub fn get_mode() -> &'static str { \"staging_environment\" }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "staging config"]);

    // Index on staging branch
    setup_and_index(root.path(), home.path());
    let ws = workspace_for(root.path());

    // Should find "staging_environment", not "production_environment"
    let staging_results = search_file_paths(&ws, "staging_environment");
    assert!(
        staging_results.iter().any(|p| p.contains("config.rs")),
        "staging content should be searchable on staging branch"
    );

    // Switch to main
    git(root.path(), &["checkout", "main"]);
    setup_and_index(root.path(), home.path());

    // Now "production_environment" should be findable
    let prod_results = search_file_paths(&ws, "production_environment");
    assert!(
        prod_results.iter().any(|p| p.contains("config.rs")),
        "production content should be searchable on main branch"
    );

    // Verify the actual indexed content reflects main, not staging
    let conn = open_sqlite(&ws.sqlite_path()).unwrap();
    let raw: Vec<u8> = conn
        .query_row(
            "SELECT text FROM chunks WHERE file_path = 'config.rs' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let text = ivygrep::indexer::decompress_text(raw);
    assert!(
        text.contains("production_environment"),
        "indexed chunk should contain 'production_environment' on main, got: {text}"
    );
    assert!(
        !text.contains("staging_environment"),
        "indexed chunk should NOT contain 'staging_environment' on main, got: {text}"
    );
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
// EDGE CASE: File rename (git mv) across branches
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn git_branch_renames_file_old_path_gone_new_path_indexed() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    git(root.path(), &["init", "-b", "main"]);

    fs::write(
        root.path().join("old_name.rs"),
        "pub fn important_logic() -> i32 { 42 }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "original file"]);

    // Index on main with the original filename
    setup_and_index(root.path(), home.path());
    let ws = workspace_for(root.path());
    assert!(
        indexed_files(&ws).contains("old_name.rs"),
        "old_name.rs indexed before rename"
    );

    // Create a branch that renames the file
    git(root.path(), &["checkout", "-b", "refactor"]);
    git(root.path(), &["mv", "old_name.rs", "new_name.rs"]);
    git(root.path(), &["commit", "-m", "rename file"]);

    // Re-index after rename
    let s = setup_and_index(root.path(), home.path());
    assert!(s.indexed_files >= 1, "new_name.rs should be indexed");
    assert!(s.deleted_files >= 1, "old_name.rs should be deleted");

    let files = indexed_files(&ws);
    assert!(
        files.contains("new_name.rs"),
        "new_name.rs is indexed after rename"
    );
    assert!(
        !files.contains("old_name.rs"),
        "old_name.rs is GONE after rename"
    );

    // Search should find content via new path
    let results = search_file_paths(&ws, "important_logic");
    assert!(
        results.iter().any(|p| p.contains("new_name.rs")),
        "important_logic findable under new_name.rs"
    );
    assert!(
        !results.iter().any(|p| p.contains("old_name.rs")),
        "important_logic NOT under old_name.rs"
    );

    // Switch back to main — original name should be restored
    git(root.path(), &["checkout", "main"]);
    setup_and_index(root.path(), home.path());

    let files_main = indexed_files(&ws);
    assert!(
        files_main.contains("old_name.rs"),
        "old_name.rs restored on main"
    );
    assert!(
        !files_main.contains("new_name.rs"),
        "new_name.rs gone on main"
    );
}

// ---------------------------------------------------------------------------
// EDGE CASE: Entire subdirectory appears/disappears on branch switch
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn git_branch_adds_entire_subdirectory() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    git(root.path(), &["init", "-b", "main"]);

    fs::write(
        root.path().join("main.rs"),
        "fn main() { println!(\"hello\"); }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "just main.rs"]);

    // Index on main — only 1 file
    setup_and_index(root.path(), home.path());
    let ws = workspace_for(root.path());
    let initial_chunks = chunk_count(&ws);
    assert_eq!(indexed_files(&ws).len(), 1, "only main.rs on main");

    // Create branch with an entire new subdirectory (5 files)
    git(root.path(), &["checkout", "-b", "feature/api"]);
    fs::create_dir_all(root.path().join("api/handlers")).unwrap();
    for i in 0..5 {
        let content = format!(
            "pub fn handle_request_{}(req: &str) -> String {{ format!(\"response_{}: {{}}\", req) }}\n",
            i, i
        );
        fs::write(
            root.path().join(format!("api/handlers/handler_{i}.rs")),
            content,
        )
        .unwrap();
    }
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "add api handlers"]);

    // Re-index — 5 new files should appear
    let s = setup_and_index(root.path(), home.path());
    assert_eq!(s.indexed_files, 5, "5 new handler files indexed");
    assert_eq!(s.deleted_files, 0, "main.rs not deleted");

    let files = indexed_files(&ws);
    assert_eq!(files.len(), 6, "main.rs + 5 handlers");
    assert!(files.contains("api/handlers/handler_0.rs"));
    assert!(files.contains("api/handlers/handler_4.rs"));
    assert!(
        chunk_count(&ws) > initial_chunks,
        "more chunks after adding files"
    );

    // Search for handler content
    let results = search_file_paths(&ws, "handle_request_3");
    assert!(
        results.iter().any(|p| p.contains("handler_3.rs")),
        "handler_3.rs searchable on feature branch"
    );

    // Switch back to main — entire api/ directory disappears
    git(root.path(), &["checkout", "main"]);
    let s_back = setup_and_index(root.path(), home.path());
    assert_eq!(s_back.deleted_files, 5, "5 handler files removed");

    let files_main = indexed_files(&ws);
    assert_eq!(files_main.len(), 1, "back to just main.rs");
    assert!(files_main.contains("main.rs"));
    assert!(!files_main.contains("api/handlers/handler_0.rs"));

    // Chunks should be back to initial count
    assert_eq!(
        chunk_count(&ws),
        initial_chunks,
        "chunk count restored after switching back"
    );

    // Handler search should return nothing
    let results_back = search_file_paths(&ws, "handle_request_3");
    assert!(
        !results_back.iter().any(|p| p.contains("handler_3.rs")),
        "handler_3.rs NOT searchable after switching back to main"
    );
}

// ---------------------------------------------------------------------------
// WORKTREE: Seed-from-base indexing
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn git_worktree_seeds_from_base_and_applies_delta() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    // Create a repo with 50 files to make the seed benefit obvious
    git(root.path(), &["init", "-b", "main"]);

    for i in 0..50 {
        fs::write(
            root.path().join(format!("module_{i:03}.rs")),
            format!("pub fn func_{i}() -> usize {{ {i} }}\n"),
        )
        .unwrap();
    }
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "initial 50 files"]);

    // Index the main workspace
    let s1 = setup_and_index(root.path(), home.path());
    assert_eq!(s1.indexed_files, 50, "all 50 files indexed on main");

    let ws = workspace_for(root.path());
    let main_chunks = chunk_count(&ws);
    assert!(main_chunks > 0, "main has chunks");

    // Create a branch with 2 modified files
    git(root.path(), &["checkout", "-b", "feature/tweak"]);
    fs::write(
        root.path().join("module_010.rs"),
        "pub fn func_10_modified() -> usize { 1000 }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("module_020.rs"),
        "pub fn func_20_modified() -> usize { 2000 }\n",
    )
    .unwrap();
    // Add a new file only on feature branch
    fs::write(
        root.path().join("feature_only.rs"),
        "pub fn feature_exclusive() -> &'static str { \"only_on_feature\" }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "modify 2 files, add 1"]);

    // Go back to main
    git(root.path(), &["checkout", "main"]);

    // Create a worktree for the feature branch
    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("worktree");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            wt_path.to_str().unwrap(),
            "feature/tweak",
        ],
    );

    // Verify the worktree has the .git file (not directory)
    assert!(
        wt_path.join(".git").is_file(),
        "worktree should have .git file"
    );

    // Index the worktree — it should seed from the base
    let s2 = setup_and_index(&wt_path, home.path());

    let wt_ws = workspace_for(&wt_path);

    // Verify worktree detection
    assert!(
        wt_ws.is_worktree(),
        "worktree workspace should report is_worktree=true"
    );
    assert!(
        wt_ws.base_index_dir.is_some(),
        "worktree should have base_index_dir"
    );

    // The worktree should have seeded from base:
    // - It should have processed far fewer files than a full re-index (not 50)
    // - It should have the modified + added files indexed
    assert!(
        s2.indexed_files < 50,
        "worktree should seed from base, not re-index all 50 files. Got: {}",
        s2.indexed_files,
    );

    // Verify the worktree's index has the correct content
    let wt_files = indexed_files(&wt_ws);

    // All 50 original + 1 new file
    assert!(
        wt_files.contains("module_000.rs"),
        "inherited file from base"
    );
    assert!(
        wt_files.contains("module_049.rs"),
        "inherited file from base"
    );
    assert!(
        wt_files.contains("feature_only.rs"),
        "new file on feature branch"
    );

    // Search should find the modified content
    let modified_results = search_file_paths(&wt_ws, "func_10_modified");
    assert!(
        modified_results.iter().any(|p| p.contains("module_010.rs")),
        "modified func_10 should be searchable in worktree"
    );

    // Search should find the feature-only content
    let feature_results = search_file_paths(&wt_ws, "feature_exclusive");
    assert!(
        feature_results
            .iter()
            .any(|p| p.contains("feature_only.rs")),
        "feature_exclusive should be searchable in worktree"
    );

    // Search for inherited content should still work
    let inherited_results = search_file_paths(&wt_ws, "func_0");
    assert!(
        !inherited_results.is_empty(),
        "inherited content from base should be searchable"
    );

    // base_ref.json should exist
    assert!(
        wt_ws.index_dir.join("base_ref.json").exists(),
        "base_ref.json should be written"
    );

    // Clean up worktree
    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn git_worktree_repo_id_matches_main() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    git(root.path(), &["init", "-b", "main"]);
    fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "initial"]);

    // Index main
    setup_and_index(root.path(), home.path());
    let main_ws = workspace_for(root.path());

    // Create worktree
    git(root.path(), &["checkout", "-b", "wt-branch"]);
    git(root.path(), &["checkout", "main"]);

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt-branch"],
    );

    let wt_ws = workspace_for(&wt_path);

    // repo_id should be the same for both
    assert!(main_ws.repo_id.is_some(), "main should have repo_id");
    assert!(wt_ws.repo_id.is_some(), "worktree should have repo_id");
    assert_eq!(
        main_ws.repo_id, wt_ws.repo_id,
        "main and worktree should share the same repo_id"
    );

    // workspace IDs should be DIFFERENT (different paths)
    assert_ne!(
        main_ws.id, wt_ws.id,
        "main and worktree should have different workspace IDs"
    );

    // worktree should detect base
    assert!(wt_ws.is_worktree(), "wt should be a worktree");
    assert!(!main_ws.is_worktree(), "main should NOT be a worktree");

    // Clean up
    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

// ===========================================================================
// WORKTREE OVERLAY: Tombstone accuracy — delete in worktree must be invisible
// to search, but base must still have it.
// ===========================================================================

#[test]
#[serial]
fn worktree_tombstone_hides_deleted_file_from_search() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    git(root.path(), &["init", "-b", "main"]);

    fs::write(
        root.path().join("keep.rs"),
        "pub fn keep_me() -> &'static str { \"always here\" }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("remove_me.rs"),
        "pub fn secret_base_function() -> i32 { 999 }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "two files on main"]);

    // Index the base
    setup_and_index(root.path(), home.path());
    let base_ws = workspace_for(root.path());

    // Confirm both files searchable in base
    let base_results = search_file_paths(&base_ws, "secret_base_function");
    assert!(
        base_results.iter().any(|p| p.contains("remove_me.rs")),
        "base should find secret_base_function"
    );

    // Create branch that deletes remove_me.rs
    git(root.path(), &["checkout", "-b", "wt-delete"]);
    fs::remove_file(root.path().join("remove_me.rs")).unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "delete remove_me.rs"]);
    git(root.path(), &["checkout", "main"]);

    // Create worktree
    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_delete");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt-delete"],
    );

    // Index the worktree
    setup_and_index(&wt_path, home.path());
    let wt_ws = workspace_for(&wt_path);

    // Worktree: remove_me.rs must be invisible
    let wt_results = search_file_paths(&wt_ws, "secret_base_function");
    assert!(
        !wt_results.iter().any(|p| p.contains("remove_me.rs")),
        "worktree must NOT find secret_base_function — tombstone should hide it"
    );

    // Worktree: keep.rs must still be found (inherited from base)
    let wt_keep = search_file_paths(&wt_ws, "keep_me");
    assert!(
        wt_keep.iter().any(|p| p.contains("keep.rs")),
        "worktree should still find keep_me via base inheritance"
    );

    // Worktree: indexed_files should not contain remove_me.rs
    let wt_files = indexed_files(&wt_ws);
    assert!(
        !wt_files.contains("remove_me.rs"),
        "worktree indexed_files must not contain tombstoned file"
    );
    assert!(
        wt_files.contains("keep.rs"),
        "worktree indexed_files must contain inherited file"
    );

    // Base: must still find remove_me.rs (unaffected by worktree)
    let base_results_after = search_file_paths(&base_ws, "secret_base_function");
    assert!(
        base_results_after
            .iter()
            .any(|p| p.contains("remove_me.rs")),
        "base must still find secret_base_function — worktree must not contaminate base"
    );

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

// ===========================================================================
// WORKTREE OVERLAY: Content isolation — worktree-only files must not leak
// into base search.
// ===========================================================================

#[test]
#[serial]
fn worktree_new_file_invisible_to_base_search() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    git(root.path(), &["init", "-b", "main"]);
    fs::write(
        root.path().join("base.rs"),
        "pub fn base_func() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base file"]);

    setup_and_index(root.path(), home.path());

    // Branch with a new exclusive file
    git(root.path(), &["checkout", "-b", "wt-add"]);
    fs::write(
        root.path().join("worktree_exclusive.rs"),
        "pub fn only_in_worktree() -> &'static str { \"exclusive_content_xyz\" }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "add exclusive file"]);
    git(root.path(), &["checkout", "main"]);

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_add");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt-add"],
    );

    setup_and_index(&wt_path, home.path());
    let wt_ws = workspace_for(&wt_path);

    // Worktree: should find the exclusive content
    let wt_results = search_file_paths(&wt_ws, "exclusive_content_xyz");
    assert!(
        wt_results
            .iter()
            .any(|p| p.contains("worktree_exclusive.rs")),
        "worktree must find exclusive_content_xyz"
    );

    // Worktree: should also find inherited base content
    let wt_base = search_file_paths(&wt_ws, "base_func");
    assert!(
        wt_base.iter().any(|p| p.contains("base.rs")),
        "worktree must find inherited base_func"
    );

    // Base: must NOT find worktree-exclusive content
    let base_ws = workspace_for(root.path());
    let base_results = search_file_paths(&base_ws, "exclusive_content_xyz");
    assert!(
        !base_results
            .iter()
            .any(|p| p.contains("worktree_exclusive.rs")),
        "base must NOT find worktree-exclusive content"
    );

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

// ===========================================================================
// WORKTREE OVERLAY: Modified content divergence — same file, different content
// ===========================================================================

#[test]
#[serial]
fn worktree_modified_file_shows_overlay_content_not_base() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    git(root.path(), &["init", "-b", "main"]);
    fs::write(
        root.path().join("divergent.rs"),
        "pub fn production_cardinal_zebra() -> i32 { 42 }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base version"]);

    setup_and_index(root.path(), home.path());

    // Branch that modifies the same file
    git(root.path(), &["checkout", "-b", "wt-mod"]);
    fs::write(
        root.path().join("divergent.rs"),
        "pub fn staging_flamingo_penguin() -> i32 { 99 }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "worktree version"]);
    git(root.path(), &["checkout", "main"]);

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_mod");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt-mod"],
    );

    setup_and_index(&wt_path, home.path());
    let wt_ws = workspace_for(&wt_path);

    // Worktree: must find worktree-specific content
    let wt_results = search_file_paths(&wt_ws, "staging_flamingo_penguin");
    assert!(
        wt_results.iter().any(|p| p.contains("divergent.rs")),
        "worktree must find staging_flamingo_penguin in divergent.rs"
    );

    // Worktree: any result for divergent.rs must serve overlay content, never base content.
    // (In a tiny index, hash embedding similarity can return low-relevance hits for the
    //  same file. What matters is the content served is exclusively from the overlay.)
    let wt_base_hits = search_hits_for_file(&wt_ws, "production_cardinal_zebra", "divergent.rs");
    for (_path, preview) in &wt_base_hits {
        assert!(
            !preview.contains("production_cardinal_zebra"),
            "worktree must NOT serve base content — got preview: {preview}"
        );
    }

    // Base: must find base-specific content
    let base_ws = workspace_for(root.path());
    let base_results = search_file_paths(&base_ws, "production_cardinal_zebra");
    assert!(
        base_results.iter().any(|p| p.contains("divergent.rs")),
        "base must still find production_cardinal_zebra"
    );

    // Base: must NOT find worktree-specific content
    // Base: any result for divergent.rs must serve base content, not overlay content.
    let base_leak_hits = search_hits_for_file(&base_ws, "staging_flamingo_penguin", "divergent.rs");
    for (_path, preview) in &base_leak_hits {
        assert!(
            !preview.contains("staging_flamingo_penguin"),
            "base must NOT serve worktree content — got preview: {preview}"
        );
    }

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

// ===========================================================================
// WORKTREE OVERLAY: Auto-index base when indexing worktree first
// ===========================================================================

#[test]
#[serial]
fn worktree_auto_indexes_base_when_missing() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    git(root.path(), &["init", "-b", "main"]);

    for i in 0..10 {
        fs::write(
            root.path().join(format!("src_{i}.rs")),
            format!("pub fn base_func_{i}() -> usize {{ {i} }}\n"),
        )
        .unwrap();
    }
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "initial 10 files"]);

    git(root.path(), &["checkout", "-b", "wt-auto"]);
    fs::write(
        root.path().join("auto_only.rs"),
        "pub fn auto_exclusive_marker() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "worktree exclusive"]);
    git(root.path(), &["checkout", "main"]);

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_auto");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt-auto"],
    );

    // Index the worktree WITHOUT indexing the base first
    let s = setup_and_index(&wt_path, home.path());
    let wt_ws = workspace_for(&wt_path);

    assert!(wt_ws.is_worktree(), "should detect as worktree");

    // The worktree should have fewer indexed files than a full re-index
    assert!(
        s.indexed_files < 11,
        "worktree delta should be small, not all 11 files. Got: {}",
        s.indexed_files
    );

    // Worktree search should find both inherited and exclusive content
    let inherited = search_file_paths(&wt_ws, "base_func_5");
    assert!(
        !inherited.is_empty(),
        "worktree should find inherited base_func_5 after auto-indexing base"
    );

    let exclusive = search_file_paths(&wt_ws, "auto_exclusive_marker");
    assert!(
        exclusive.iter().any(|p| p.contains("auto_only.rs")),
        "worktree should find auto_exclusive_marker"
    );

    // Verify the base was actually indexed (base metadata should exist)
    let base_ws = workspace_for(root.path());
    let base_files = indexed_files(&base_ws);
    assert_eq!(
        base_files.len(),
        10,
        "base should have all 10 files after auto-indexing cascade"
    );

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

// ===========================================================================
// WORKTREE OVERLAY: Incremental update — further changes to overlay
// ===========================================================================

#[test]
#[serial]
fn worktree_incremental_overlay_update() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    git(root.path(), &["init", "-b", "main"]);
    fs::write(
        root.path().join("stable.rs"),
        "pub fn stable_func() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "initial"]);

    setup_and_index(root.path(), home.path());

    git(root.path(), &["checkout", "-b", "wt-incr"]);
    fs::write(
        root.path().join("phase1.rs"),
        "pub fn phase1_marker() -> &'static str { \"phase1_content\" }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "phase 1"]);
    git(root.path(), &["checkout", "main"]);

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_incr");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt-incr"],
    );

    // Phase 1: initial overlay
    setup_and_index(&wt_path, home.path());
    let wt_ws = workspace_for(&wt_path);
    let phase1 = search_file_paths(&wt_ws, "phase1_marker");
    assert!(
        phase1.iter().any(|p| p.contains("phase1.rs")),
        "phase 1: should find phase1_marker"
    );

    // Phase 2: make an uncommitted change directly in worktree
    fs::write(
        wt_path.join("phase2.rs"),
        "pub fn phase2_new_marker() -> &'static str { \"phase2_content\" }\n",
    )
    .unwrap();

    // Re-index the worktree incrementally
    let s2 = setup_and_index(&wt_path, home.path());
    assert!(
        s2.indexed_files >= 1,
        "phase 2: at least phase2.rs should be indexed"
    );

    // Both phase1 and phase2 content should be searchable
    let phase1_still = search_file_paths(&wt_ws, "phase1_marker");
    assert!(
        phase1_still.iter().any(|p| p.contains("phase1.rs")),
        "phase 2: phase1_marker should still be found"
    );

    let phase2 = search_file_paths(&wt_ws, "phase2_new_marker");
    assert!(
        phase2.iter().any(|p| p.contains("phase2.rs")),
        "phase 2: phase2_new_marker should be found after incremental update"
    );

    // Stable base content should still be inherited
    let stable = search_file_paths(&wt_ws, "stable_func");
    assert!(
        stable.iter().any(|p| p.contains("stable.rs")),
        "inherited stable_func should still be searchable"
    );

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

// ===========================================================================
// WORKTREE OVERLAY: Multiple worktrees are independent
// ===========================================================================

#[test]
#[serial]
fn multiple_worktrees_are_independent() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    git(root.path(), &["init", "-b", "main"]);
    fs::write(
        root.path().join("shared.rs"),
        "pub fn shared_func() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "shared base"]);

    setup_and_index(root.path(), home.path());

    // Branch A: adds file_a.rs
    git(root.path(), &["checkout", "-b", "wt-a"]);
    fs::write(
        root.path().join("file_a.rs"),
        "pub fn only_in_branch_a() -> &'static str { \"branch_a_unique_marker\" }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "branch A file"]);
    git(root.path(), &["checkout", "main"]);

    // Branch B: adds file_b.rs, deletes shared.rs
    git(root.path(), &["checkout", "-b", "wt-b"]);
    fs::write(
        root.path().join("file_b.rs"),
        "pub fn only_in_branch_b() -> &'static str { \"branch_b_unique_marker\" }\n",
    )
    .unwrap();
    fs::remove_file(root.path().join("shared.rs")).unwrap();
    git(root.path(), &["add", "."]);
    git(
        root.path(),
        &["commit", "-m", "branch B file, delete shared"],
    );
    git(root.path(), &["checkout", "main"]);

    // Create both worktrees
    let wt_a_dir = tempdir().unwrap();
    let wt_a_path = wt_a_dir.path().join("wt_a");
    git(
        root.path(),
        &["worktree", "add", wt_a_path.to_str().unwrap(), "wt-a"],
    );

    let wt_b_dir = tempdir().unwrap();
    let wt_b_path = wt_b_dir.path().join("wt_b");
    git(
        root.path(),
        &["worktree", "add", wt_b_path.to_str().unwrap(), "wt-b"],
    );

    // Index both worktrees
    setup_and_index(&wt_a_path, home.path());
    setup_and_index(&wt_b_path, home.path());

    let ws_a = workspace_for(&wt_a_path);
    let ws_b = workspace_for(&wt_b_path);

    // Worktree A: should find branch_a_unique_marker and shared_func
    let a_own = search_file_paths(&ws_a, "branch_a_unique_marker");
    assert!(
        a_own.iter().any(|p| p.contains("file_a.rs")),
        "wt-a must find its own branch_a_unique_marker"
    );
    let a_shared = search_file_paths(&ws_a, "shared_func");
    assert!(
        a_shared.iter().any(|p| p.contains("shared.rs")),
        "wt-a must find inherited shared_func"
    );
    let a_leak = search_file_paths(&ws_a, "branch_b_unique_marker");
    assert!(
        !a_leak.iter().any(|p| p.contains("file_b.rs")),
        "wt-a must NOT find branch_b_unique_marker"
    );

    // Worktree B: should find branch_b_unique_marker but NOT shared_func
    let b_own = search_file_paths(&ws_b, "branch_b_unique_marker");
    assert!(
        b_own.iter().any(|p| p.contains("file_b.rs")),
        "wt-b must find its own branch_b_unique_marker"
    );
    let b_shared = search_file_paths(&ws_b, "shared_func");
    assert!(
        !b_shared.iter().any(|p| p.contains("shared.rs")),
        "wt-b must NOT find shared_func — it was deleted in this branch"
    );
    let b_leak = search_file_paths(&ws_b, "branch_a_unique_marker");
    assert!(
        !b_leak.iter().any(|p| p.contains("file_a.rs")),
        "wt-b must NOT find branch_a_unique_marker"
    );

    // Base: must be unaffected by both worktrees
    let base_ws = workspace_for(root.path());
    let base_shared = search_file_paths(&base_ws, "shared_func");
    assert!(
        base_shared.iter().any(|p| p.contains("shared.rs")),
        "base must still find shared_func"
    );
    let base_a = search_file_paths(&base_ws, "branch_a_unique_marker");
    assert!(
        !base_a.iter().any(|p| p.contains("file_a.rs")),
        "base must NOT find branch_a content"
    );
    let base_b = search_file_paths(&base_ws, "branch_b_unique_marker");
    assert!(
        !base_b.iter().any(|p| p.contains("file_b.rs")),
        "base must NOT find branch_b content"
    );

    git(
        root.path(),
        &["worktree", "remove", wt_a_path.to_str().unwrap(), "--force"],
    );
    git(
        root.path(),
        &["worktree", "remove", wt_b_path.to_str().unwrap(), "--force"],
    );
}

// ===========================================================================
// WORKTREE OVERLAY: Delete then re-add file with different content
// ===========================================================================

#[test]
#[serial]
fn worktree_delete_then_readd_shows_new_content() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    git(root.path(), &["init", "-b", "main"]);
    fs::write(
        root.path().join("mutable.rs"),
        "pub fn mercury_astronaut_launch() -> i32 { 7 }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "original"]);

    setup_and_index(root.path(), home.path());

    // Branch: delete the file, then recreate it with completely different content
    git(root.path(), &["checkout", "-b", "wt-readd"]);
    fs::remove_file(root.path().join("mutable.rs")).unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "delete mutable.rs"]);

    fs::write(
        root.path().join("mutable.rs"),
        "pub fn neptune_submarine_ocean() -> i32 { 88 }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "recreate mutable.rs"]);
    git(root.path(), &["checkout", "main"]);

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_readd");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt-readd"],
    );

    setup_and_index(&wt_path, home.path());
    let wt_ws = workspace_for(&wt_path);

    // Worktree: must find the NEW content
    let new_results = search_file_paths(&wt_ws, "neptune_submarine_ocean");
    assert!(
        new_results.iter().any(|p| p.contains("mutable.rs")),
        "worktree must find neptune_submarine_ocean in re-added mutable.rs"
    );

    // Worktree: any result for mutable.rs must serve overlay content, never base content.
    let old_hits = search_hits_for_file(&wt_ws, "mercury_astronaut_launch", "mutable.rs");
    for (_path, preview) in &old_hits {
        assert!(
            !preview.contains("mercury_astronaut_launch"),
            "worktree must NOT serve base content — got preview: {preview}"
        );
    }

    // Base: must still have the original content
    let base_ws = workspace_for(root.path());
    let base_results = search_file_paths(&base_ws, "mercury_astronaut_launch");
    assert!(
        base_results.iter().any(|p| p.contains("mutable.rs")),
        "base must still find mercury_astronaut_launch"
    );

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

// ===========================================================================
// WORKTREE OVERLAY: Staleness invalidation upon base index update
// ===========================================================================

#[test]
#[serial]
fn worktree_overlay_staleness_invalidation() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    git(root.path(), &["init", "-b", "main"]);
    fs::write(root.path().join("base.rs"), "pub fn base_v1() {}\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base v1"]);

    setup_and_index(root.path(), home.path());

    git(root.path(), &["checkout", "-b", "wt-branch"]);
    fs::write(root.path().join("wt.rs"), "pub fn wt_only() {}\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "wt branch"]);
    git(root.path(), &["checkout", "main"]);

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_stale");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt-branch"],
    );

    setup_and_index(&wt_path, home.path());

    let wt_ws = workspace_for(&wt_path);

    let r1 = search_file_paths(&wt_ws, "base_v1");
    assert!(r1.iter().any(|p| p.contains("base.rs")));

    // 4. Update base!
    fs::write(
        root.path().join("base.rs"),
        "pub fn base_v1() {}\npub fn base_v2() {}\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base v2"]);

    // Index base (bumps generation)
    setup_and_index(root.path(), home.path());
    let base_ws = workspace_for(root.path());
    let r_base = search_file_paths(&base_ws, "base_v2");
    assert!(r_base.iter().any(|p| p.contains("base.rs")));

    // 5. Re-index worktree! Should detect staleness and REBUILD overlay.
    setup_and_index(&wt_path, home.path());

    // It should now find base_v2 inherited from base!
    let r2 = search_file_paths(&wt_ws, "base_v2");
    assert!(
        r2.iter().any(|p| p.contains("base.rs")),
        "Worktree overlay must find new base content after base updates and worktree re-indexes"
    );

    // And make sure wt.rs is still there
    let r3 = search_file_paths(&wt_ws, "wt_only");
    assert!(
        r3.iter().any(|p| p.contains("wt.rs")),
        "Worktree overlay must still find its own files after invalidation rebuild"
    );

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn worktree_overlay_auto_reindex_via_cli_e2e() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    git(root.path(), &["init", "-b", "main"]);
    fs::write(root.path().join("base.rs"), "pub fn base_v1() {}\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base v1"]);

    setup_and_index(root.path(), home.path());

    git(root.path(), &["checkout", "-b", "wt-e2e-branch"]);
    fs::write(root.path().join("wt.rs"), "pub fn wt_only_e2e() {}\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "wt e2e branch"]);
    git(root.path(), &["checkout", "main"]);

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_stale_e2e");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt-e2e-branch"],
    );

    // Initial explicit index of the worktree
    setup_and_index(&wt_path, home.path());

    // 4. Update base!
    fs::write(
        root.path().join("base.rs"),
        "pub fn base_v1() {}\npub fn base_v2_e2e() {}\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base v2"]);

    // Index base (bumps generation)
    setup_and_index(root.path(), home.path());
    
    // Now NO explicit index in worktree!
    // Just run a search using the IG CLI, which should detect staleness.
    use assert_cmd::Command;
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    cmd.current_dir(&wt_path)
        .env("IVYGREP_HOME", home.path())
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .arg("base_v2_e2e")
        .assert()
        .success()
        .stdout(predicates::str::contains("base.rs"));
}
