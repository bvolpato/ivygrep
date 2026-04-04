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
    let conn = open_sqlite(&workspace.sqlite_path()).unwrap();
    let mut stmt = conn
        .prepare("SELECT DISTINCT file_path FROM chunks ORDER BY file_path")
        .unwrap();
    stmt.query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

/// Helper: search for a query and return file paths in the results.
fn search_file_paths(workspace: &Workspace, query: &str) -> Vec<String> {
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    let hits = hybrid_search(workspace, query, Some(&model), &SearchOptions::default()).unwrap();
    hits.iter()
        .map(|h| h.file_path.to_string_lossy().to_string())
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
        &["commit", "-m", "add payments, remove core on feature branch"],
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
