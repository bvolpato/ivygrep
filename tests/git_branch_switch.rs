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
use ivygrep::context::{ContextRole, build_context_bundle};
use ivygrep::embedding::{EmbeddingModel, HashEmbeddingModel, NeuralModelIdentity};
use ivygrep::indexer::{
    enhance_workspace_hash, enhance_workspace_neural, index_workspace, open_sqlite,
    reconcile_worktree_overlay,
};
use ivygrep::regex_search::regex_search_with_options;
use ivygrep::search::{SearchContext, SearchOptions, hybrid_search};
use ivygrep::workspace::Workspace;

/// Run a git command in the given directory, panicking on failure.
fn git(dir: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "commit.gpgSign=false"])
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

fn git_output(dir: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to run git {:?}: {e}", args));
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn init_git_repo(dir: &std::path::Path) {
    git(dir, &["init"]);
    git(dir, &["symbolic-ref", "HEAD", "refs/heads/main"]);
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

struct TestWorktreeNeuralModel(HashEmbeddingModel);

impl EmbeddingModel for TestWorktreeNeuralModel {
    fn dimensions(&self) -> usize {
        self.0.dimensions()
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        self.0.embed(text)
    }

    fn model_identity(&self) -> Option<&NeuralModelIdentity> {
        static IDENTITY: std::sync::OnceLock<NeuralModelIdentity> = std::sync::OnceLock::new();
        Some(IDENTITY.get_or_init(ivygrep::embedding::configured_neural_model_identity))
    }

    fn profile_info(&self) -> Option<&'static str> {
        Some("static")
    }
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

fn overlay_counts(workspace: &Workspace) -> (i64, i64) {
    let conn = open_sqlite(&workspace.overlay_sqlite_path()).unwrap();
    let files = conn
        .query_row("SELECT COUNT(DISTINCT file_path) FROM chunks", [], |row| {
            row.get(0)
        })
        .unwrap();
    let tombstones = conn
        .query_row("SELECT COUNT(*) FROM tombstones", [], |row| row.get(0))
        .unwrap();
    (files, tombstones)
}

fn assert_only_overlay_stores(workspace: &Workspace) {
    assert!(workspace.overlay_sqlite_path().exists());
    assert!(workspace.overlay_tantivy_dir().exists());
    assert!(workspace.overlay_vector_path().exists());

    for path in [
        workspace.sqlite_path(),
        workspace.tantivy_dir(),
        workspace.vector_path(),
        workspace.vector_neural_path(),
    ] {
        assert!(
            !path.exists(),
            "worktree must not materialize a full base store at {}",
            path.display()
        );
    }
}

#[test]
#[serial]
fn worktree_neural_search_includes_added_and_modified_branch_content() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();
    init_git_repo(root.path());
    fs::write(
        root.path().join("shared.rs"),
        "pub fn base_authentication_handler() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "seed base source"]);

    setup_and_index(root.path(), home.path());
    let base_workspace = workspace_for(root.path());
    let hash_model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    let neural_model = TestWorktreeNeuralModel(HashEmbeddingModel::new(EMBEDDING_DIMENSIONS));
    enhance_workspace_hash(&base_workspace, &hash_model).unwrap();
    assert!(enhance_workspace_neural(&base_workspace, &neural_model).unwrap() > 0);

    git(root.path(), &["branch", "neural-overlay", "main"]);
    let worktrees = tempdir().unwrap();
    let worktree = worktrees.path().join("neural-overlay");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            worktree.to_str().unwrap(),
            "neural-overlay",
        ],
    );
    fs::write(
        worktree.join("shared.rs"),
        "pub fn branch_authentication_handler() -> bool { true }\n",
    )
    .unwrap();
    fs::write(
        worktree.join("new.rs"),
        "pub fn branch_session_validation() -> bool { true }\n",
    )
    .unwrap();

    setup_and_index(&worktree, home.path());
    let overlay = workspace_for(&worktree);
    assert!(!overlay.vector_neural_path().exists());
    fs::write(
        worktree.join("shared.rs"),
        "pub fn branch_authentication_handler() -> bool { false }\n",
    )
    .unwrap();
    index_workspace(&overlay, &hash_model).unwrap();
    assert!(
        overlay.neural_tombstones_path().exists(),
        "overlay edits must journal removed neural keys before the first vector save"
    );
    enhance_workspace_hash(&overlay, &hash_model).unwrap();
    assert!(overlay.needs_neural_enhancement());
    assert!(enhance_workspace_neural(&overlay, &neural_model).unwrap() >= 2);
    assert!(!overlay.needs_neural_enhancement());

    for (query, file) in [
        ("branch_authentication_handler", "shared.rs"),
        ("branch_session_validation", "new.rs"),
    ] {
        let hits = hybrid_search(
            &overlay,
            query,
            Some(&neural_model),
            &SearchOptions {
                force_neural: true,
                ..SearchOptions::default()
            },
        )
        .unwrap();
        assert!(
            hits.iter().any(|hit| {
                hit.file_path.ends_with(file) && hit.sources.iter().any(|source| source == "neural")
            }),
            "branch-local file {file} was absent from neural results: {hits:#?}"
        );
    }

    let base_neural_identity = fs::read(base_workspace.neural_model_path()).unwrap();
    fs::remove_file(base_workspace.neural_model_path()).unwrap();
    let context = SearchContext::load(&overlay, Some(EMBEDDING_DIMENSIONS), true).unwrap();
    assert!(context.neural_vectors.is_some());
    assert!(
        context.base_neural_vectors.is_none(),
        "base neural vectors must not borrow the overlay model identity"
    );

    fs::write(base_workspace.neural_model_path(), base_neural_identity).unwrap();
    fs::remove_file(overlay.neural_model_path()).unwrap();
    let context = SearchContext::load(&overlay, Some(EMBEDDING_DIMENSIONS), true).unwrap();
    assert!(context.base_neural_vectors.is_some());
    assert!(
        context.neural_vectors.is_none(),
        "overlay neural vectors must not borrow the base model identity"
    );

    git(
        root.path(),
        &["worktree", "remove", worktree.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn worktree_indexed_regex_combines_base_and_overlay_without_stale_matches() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();
    init_git_repo(root.path());
    fs::write(
        root.path().join("base.rs"),
        "pub fn shared_regex_needle() {}\n",
    )
    .unwrap();
    fs::write(
        root.path().join("modified.rs"),
        "pub fn obsolete_regex_needle() {}\n",
    )
    .unwrap();
    fs::write(
        root.path().join("deleted.rs"),
        "pub fn deleted_regex_needle() {}\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "seed regex base"]);
    setup_and_index(root.path(), home.path());

    git(root.path(), &["branch", "regex-overlay", "main"]);
    let worktrees = tempdir().unwrap();
    let worktree = worktrees.path().join("regex-overlay");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            worktree.to_str().unwrap(),
            "regex-overlay",
        ],
    );
    fs::write(
        worktree.join("modified.rs"),
        "pub fn updated_regex_needle() {}\n",
    )
    .unwrap();
    fs::write(
        worktree.join("added.rs"),
        "pub fn added_regex_needle() {}\n",
    )
    .unwrap();
    fs::remove_file(worktree.join("deleted.rs")).unwrap();
    setup_and_index(&worktree, home.path());
    let workspace = workspace_for(&worktree);

    let hits =
        regex_search_with_options(&workspace, "regex_needle", &SearchOptions::default()).unwrap();
    let files = hits
        .iter()
        .map(|hit| hit.file_path.to_string_lossy().into_owned())
        .collect::<HashSet<_>>();

    assert!(files.contains("base.rs"));
    assert!(files.contains("modified.rs"));
    assert!(files.contains("added.rs"));
    assert!(!files.contains("deleted.rs"));
    assert!(hits.iter().all(|hit| !hit.preview.contains("obsolete")));

    git(
        root.path(),
        &["worktree", "remove", worktree.to_str().unwrap(), "--force"],
    );
}

fn stored_chunk_text(workspace: &Workspace, file_path: &str) -> Option<String> {
    let conn = open_sqlite(&workspace.sqlite_path()).unwrap();
    conn.query_row(
        "SELECT text FROM chunks WHERE file_path = ?1 LIMIT 1",
        [file_path],
        |row| row.get::<_, Vec<u8>>(0),
    )
    .map(ivygrep::indexer::decompress_text)
    .ok()
}

fn stored_chunk_texts_at(sqlite_path: &std::path::Path, file_path: &str) -> Vec<String> {
    let conn = open_sqlite(sqlite_path).unwrap();
    let mut stmt = conn
        .prepare("SELECT text FROM chunks WHERE file_path = ?1 ORDER BY chunk_key")
        .unwrap();
    stmt.query_map([file_path], |row| row.get::<_, Vec<u8>>(0))
        .unwrap()
        .map(|row| ivygrep::indexer::decompress_text(row.unwrap()))
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

    init_git_repo(root.path());

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
    init_git_repo(root.path());

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
    init_git_repo(root.path());

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

    init_git_repo(root.path());

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

    init_git_repo(root.path());

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
    init_git_repo(root.path());

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

    init_git_repo(root.path());
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

    init_git_repo(root.path());

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

    init_git_repo(root.path());
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

    init_git_repo(root.path());
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
// WORKTREE OVERLAY: Directory move and checkout keep overlay thin
// ===========================================================================

#[test]
#[serial]
fn worktree_moved_directory_checkout_restores_base_without_materializing_it() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    fs::write(root.path().join(".gitignore"), ".git\n").unwrap();
    fs::create_dir_all(root.path().join("src/legacy")).unwrap();
    fs::create_dir_all(root.path().join("src/stable")).unwrap();
    fs::write(
        root.path().join("src/legacy/keep.rs"),
        "pub fn legacy_keep_marker() -> bool { true }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("src/legacy/edit.rs"),
        "pub fn legacy_edit_marker() -> bool { true }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("src/legacy/remove.rs"),
        "pub fn legacy_remove_marker() -> bool { true }\n",
    )
    .unwrap();
    for i in 0..30 {
        fs::write(
            root.path().join(format!("src/stable/file_{i}.rs")),
            format!("pub fn stable_marker_{i}() -> usize {{ {i} }}\n"),
        )
        .unwrap();
    }
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base layout"]);
    setup_and_index(root.path(), home.path());

    git(root.path(), &["checkout", "-b", "wt-moved-layout"]);
    git(root.path(), &["mv", "src/legacy", "src/moved"]);
    fs::write(
        root.path().join("src/moved/edit.rs"),
        "pub fn moved_edit_marker() -> bool { true }\n",
    )
    .unwrap();
    fs::remove_file(root.path().join("src/moved/remove.rs")).unwrap();
    fs::write(
        root.path().join("src/moved/add.rs"),
        "pub fn moved_add_marker() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "-A"]);
    git(
        root.path(),
        &["commit", "-m", "move layout with mixed delta"],
    );
    git(root.path(), &["checkout", "main"]);
    git(root.path(), &["branch", "wt-base-layout", "main"]);

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_move");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            wt_path.to_str().unwrap(),
            "wt-moved-layout",
        ],
    );

    let moved = setup_and_index(&wt_path, home.path());
    let wt_ws = workspace_for(&wt_path);
    assert_eq!(
        moved.indexed_files, 3,
        "only moved/add/edited files indexed"
    );
    assert_eq!(moved.deleted_files, 3, "old directory paths tombstoned");
    assert_eq!(
        overlay_counts(&wt_ws),
        (3, 3),
        "overlay stores only divergence"
    );

    let files = indexed_files(&wt_ws);
    assert!(files.contains("src/moved/keep.rs"));
    assert!(files.contains("src/moved/edit.rs"));
    assert!(files.contains("src/moved/add.rs"));
    assert!(!files.contains("src/legacy/keep.rs"));
    assert!(!files.contains("src/legacy/remove.rs"));
    assert!(files.contains("src/stable/file_17.rs"));
    assert!(
        search_file_paths(&wt_ws, "moved_edit_marker")
            .iter()
            .any(|path| path.contains("src/moved/edit.rs"))
    );
    assert!(
        search_file_paths(&wt_ws, "legacy_remove_marker")
            .iter()
            .all(|path| !path.contains("src/legacy/remove.rs"))
    );

    git(&wt_path, &["checkout", "wt-base-layout"]);
    let restored = setup_and_index(&wt_path, home.path());
    assert_eq!(
        restored.indexed_files, 0,
        "base-equivalent checkout should reuse base instead of reindexing restored files"
    );
    assert_eq!(
        overlay_counts(&wt_ws),
        (0, 0),
        "base-equivalent checkout should leave an empty overlay"
    );

    let restored_files = indexed_files(&wt_ws);
    assert!(restored_files.contains("src/legacy/keep.rs"));
    assert!(restored_files.contains("src/legacy/remove.rs"));
    assert!(!restored_files.contains("src/moved/keep.rs"));
    assert!(
        search_file_paths(&wt_ws, "legacy_remove_marker")
            .iter()
            .any(|path| path.contains("src/legacy/remove.rs"))
    );
    assert!(
        search_file_paths(&wt_ws, "moved_edit_marker")
            .iter()
            .all(|path| !path.contains("src/moved/edit.rs")),
        "restored checkout must not serve removed overlay documents"
    );

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn worktree_incremental_overlay_keeps_edit_when_live_base_is_unindexed() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    fs::write(root.path().join(".gitignore"), ".git\n").unwrap();
    fs::write(
        root.path().join("shared.rs"),
        "pub fn indexed_base_marker() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base content"]);
    setup_and_index(root.path(), home.path());

    git(root.path(), &["branch", "wt-stale-incremental", "main"]);
    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_stale_incremental");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            wt_path.to_str().unwrap(),
            "wt-stale-incremental",
        ],
    );
    setup_and_index(&wt_path, home.path());

    fs::write(
        root.path().join("shared.rs"),
        "pub fn unindexed_live_base_marker() -> bool { true }\n",
    )
    .unwrap();
    fs::write(
        wt_path.join("shared.rs"),
        "pub fn unindexed_live_base_marker() -> bool { true }\n",
    )
    .unwrap();

    let updated = setup_and_index(&wt_path, home.path());
    let wt_ws = workspace_for(&wt_path);
    assert_eq!(
        updated.indexed_files, 1,
        "worktree edit must not delegate to stale base index"
    );
    assert_eq!(
        overlay_counts(&wt_ws),
        (1, 1),
        "worktree must shadow stale indexed base content"
    );
    let base_ws = workspace_for(root.path());
    let base_text = stored_chunk_text(&base_ws, "shared.rs").expect("base chunk must exist");
    assert!(
        base_text.contains("indexed_base_marker"),
        "worktree indexing must not implicitly rewrite an established base index"
    );
    assert!(
        !base_text.contains("unindexed_live_base_marker"),
        "base storage must remain at its indexed content"
    );
    assert!(
        search_file_paths(&wt_ws, "unindexed_live_base_marker")
            .iter()
            .any(|path| path.contains("shared.rs")),
        "worktree search must return current worktree content"
    );

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn worktree_first_overlay_refreshes_stale_base_before_inheriting_content() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    fs::write(root.path().join(".gitignore"), ".git\n").unwrap();
    fs::write(
        root.path().join("shared.rs"),
        "pub fn original_indexed_marker() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base content"]);
    setup_and_index(root.path(), home.path());

    git(root.path(), &["branch", "wt-stale-initial", "main"]);
    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_stale_initial");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            wt_path.to_str().unwrap(),
            "wt-stale-initial",
        ],
    );

    fs::write(
        root.path().join("shared.rs"),
        "pub fn refreshed_base_marker() -> bool { true }\n",
    )
    .unwrap();
    fs::write(
        wt_path.join("shared.rs"),
        "pub fn refreshed_base_marker() -> bool { true }\n",
    )
    .unwrap();

    setup_and_index(&wt_path, home.path());
    let wt_ws = workspace_for(&wt_path);
    assert_eq!(
        overlay_counts(&wt_ws),
        (0, 0),
        "matching content should be inherited after refreshing stale base"
    );
    let base_ws = workspace_for(root.path());
    assert!(
        stored_chunk_text(&base_ws, "shared.rs")
            .is_some_and(|text| text.contains("refreshed_base_marker")),
        "base storage must be refreshed before an initial overlay inherits it"
    );
    assert!(
        search_file_paths(&wt_ws, "refreshed_base_marker")
            .iter()
            .any(|path| path.contains("shared.rs")),
        "worktree search must inherit refreshed base content"
    );
    assert!(
        search_file_paths(&base_ws, "refreshed_base_marker")
            .iter()
            .any(|path| path.contains("shared.rs")),
        "first overlay indexing refreshes stale base content it will inherit"
    );

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn worktree_first_overlay_refreshes_base_after_clean_head_change() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    fs::write(
        root.path().join("shared.rs"),
        "pub fn base_v1_marker() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base v1"]);
    setup_and_index(root.path(), home.path());

    git(root.path(), &["branch", "wt-old-base", "main"]);
    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_old_base");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt-old-base"],
    );

    fs::write(
        root.path().join("shared.rs"),
        "pub fn base_v2_marker() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base v2"]);

    let summary = setup_and_index(&wt_path, home.path());
    let base_ws = workspace_for(root.path());
    let wt_ws = workspace_for(&wt_path);
    assert_eq!(summary.indexed_files, 1);
    assert_eq!(overlay_counts(&wt_ws), (1, 1));
    assert_only_overlay_stores(&wt_ws);
    assert!(
        search_file_paths(&base_ws, "base_v2_marker")
            .iter()
            .any(|path| path.contains("shared.rs")),
        "the shared base must refresh when its clean HEAD changes"
    );
    assert!(
        search_file_paths(&wt_ws, "base_v1_marker")
            .iter()
            .any(|path| path.contains("shared.rs")),
        "the older worktree version must remain searchable as a delta"
    );
    assert!(
        search_hits_for_file(&wt_ws, "base_v2_marker", "shared.rs")
            .iter()
            .all(|(_, preview)| !preview.contains("base_v2_marker")),
        "the refreshed base version must remain shadowed in the older worktree"
    );

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn worktree_does_not_reuse_base_indexed_from_dirty_checkout() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    fs::write(
        root.path().join("shared.rs"),
        "pub fn committed_marker() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "committed base"]);

    fs::write(
        root.path().join("shared.rs"),
        "pub fn dirty_marker() -> bool { true }\n",
    )
    .unwrap();
    setup_and_index(root.path(), home.path());
    let base_ws = workspace_for(root.path());
    assert!(
        !base_ws.index_dir.join("indexed_git_state").exists(),
        "a dirty base index must not be marked as reusable"
    );
    git(root.path(), &["checkout", "--", "shared.rs"]);

    git(root.path(), &["branch", "wt-clean", "main"]);
    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_clean");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt-clean"],
    );

    setup_and_index(&wt_path, home.path());
    let wt_ws = workspace_for(&wt_path);
    let base_text = stored_chunk_text(&base_ws, "shared.rs").unwrap();
    assert!(
        base_text.contains("committed_marker"),
        "a clean worktree must force reconciliation of a base indexed while dirty"
    );
    assert!(
        !base_text.contains("dirty_marker"),
        "dirty base content must not survive after the checkout is cleaned"
    );
    assert!(
        search_file_paths(&wt_ws, "committed_marker")
            .iter()
            .any(|path| path.contains("shared.rs")),
        "the worktree must inherit reconciled committed content"
    );
    assert_only_overlay_stores(&wt_ws);

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn worktree_empty_edit_hides_base_content_on_first_overlay_index() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    fs::write(root.path().join(".gitignore"), ".git\n").unwrap();
    fs::write(
        root.path().join("mutable.rs"),
        "pub fn base_content_that_must_be_hidden() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base content"]);
    setup_and_index(root.path(), home.path());

    git(root.path(), &["checkout", "-b", "wt-empty"]);
    fs::write(root.path().join("mutable.rs"), "").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "empty mutable file"]);
    git(root.path(), &["checkout", "main"]);

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_empty_edit");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt-empty"],
    );

    setup_and_index(&wt_path, home.path());
    let wt_ws = workspace_for(&wt_path);
    assert_eq!(
        overlay_counts(&wt_ws),
        (0, 1),
        "empty replacement must tombstone base without storing chunks"
    );
    assert!(
        search_file_paths(&wt_ws, "base_content_that_must_be_hidden")
            .iter()
            .all(|path| !path.contains("mutable.rs")),
        "worktree must not serve base content after file becomes empty"
    );

    let base_ws = workspace_for(root.path());
    assert!(
        search_file_paths(&base_ws, "base_content_that_must_be_hidden")
            .iter()
            .any(|path| path.contains("mutable.rs")),
        "base remains searchable"
    );

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn worktree_reconciles_base_after_sparse_checkout_change() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    fs::write(
        root.path().join("kept.rs"),
        "pub fn sparse_kept_marker() -> bool { true }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("restored.rs"),
        "pub fn sparse_restored_marker() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "sparse base"]);
    git(root.path(), &["branch", "wt-full", "main"]);
    setup_and_index(root.path(), home.path());

    git(root.path(), &["sparse-checkout", "init", "--no-cone"]);
    git(root.path(), &["sparse-checkout", "set", "kept.rs"]);
    assert!(!root.path().join("restored.rs").exists());
    assert!(
        git_output(root.path(), &["status", "--porcelain=v1"]).is_empty(),
        "sparse checkout must remain clean while changing the visible file set"
    );

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_full");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt-full"],
    );
    git(&wt_path, &["sparse-checkout", "disable"]);
    assert!(wt_path.join("restored.rs").exists());

    let summary = setup_and_index(&wt_path, home.path());
    let base_ws = workspace_for(root.path());
    let wt_ws = workspace_for(&wt_path);

    assert_eq!(summary.indexed_files, 1);
    assert_eq!(overlay_counts(&wt_ws), (1, 0));
    assert!(
        stored_chunk_text(&base_ws, "restored.rs").is_none(),
        "the reconciled sparse base must not retain hidden file chunks"
    );
    assert!(
        search_file_paths(&wt_ws, "sparse_restored_marker")
            .iter()
            .any(|path| path.contains("restored.rs")),
        "the full worktree must restore the sparse-hidden file as an overlay delta"
    );
    assert_only_overlay_stores(&wt_ws);

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn worktree_reconciles_base_after_repository_excludes_change() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    fs::write(
        root.path().join("kept.rs"),
        "pub fn repository_exclude_kept_marker() -> bool { true }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("excluded.rs"),
        "pub fn repository_exclude_removed_marker() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "repository exclude base"]);
    git(root.path(), &["branch", "wt-repository-exclude", "main"]);

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_repository_exclude");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            wt_path.to_str().unwrap(),
            "wt-repository-exclude",
        ],
    );
    setup_and_index(root.path(), home.path());

    let base_ws = workspace_for(root.path());
    assert!(stored_chunk_text(&base_ws, "excluded.rs").is_some());

    fs::write(root.path().join(".git/info/exclude"), "excluded.rs\n").unwrap();
    assert!(
        git_output(root.path(), &["status", "--porcelain=v1"]).is_empty(),
        "repository excludes must not dirty the checkout"
    );

    setup_and_index(&wt_path, home.path());
    let wt_ws = workspace_for(&wt_path);
    assert!(
        stored_chunk_text(&base_ws, "excluded.rs").is_none(),
        "base chunks must reconcile when repository excludes change"
    );
    assert!(
        search_file_paths(&wt_ws, "repository_exclude_kept_marker")
            .iter()
            .any(|path| path.contains("kept.rs")),
        "worktree must retain unaffected inherited content"
    );
    assert_only_overlay_stores(&wt_ws);

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn worktree_skip_gitignore_indexes_inherited_ignored_content_via_base() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    fs::write(root.path().join(".gitignore"), ".git\nignored*.rs\n").unwrap();
    fs::write(
        root.path().join("visible.rs"),
        "pub fn visible_marker() -> bool { true }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("ignored.rs"),
        "pub fn inherited_ignored_marker() -> bool { true }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("ignored_untracked.rs"),
        "pub fn ignored_untracked_v1() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", ".gitignore", "visible.rs"]);
    git(root.path(), &["add", "-f", "ignored.rs"]);
    git(
        root.path(),
        &["commit", "-m", "base with ignored tracked file"],
    );
    setup_and_index(root.path(), home.path());

    git(root.path(), &["branch", "wt-ignore", "main"]);
    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_ignore");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt-ignore"],
    );
    setup_and_index(&wt_path, home.path());
    let base_ws = workspace_for(root.path());
    assert!(
        !indexed_files(&base_ws).contains("ignored.rs"),
        "default base index must initially omit ignored content"
    );
    // A watcher cannot observe an indexing-mode change; promotion must force a scan.
    fs::write(base_ws.watcher_pid_path(), std::process::id().to_string()).unwrap();

    use assert_cmd::Command;
    let mut index_including_ignored = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    index_including_ignored
        .current_dir(&wt_path)
        .env("IVYGREP_HOME", home.path())
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--add", ".", "--hash", "--skip-gitignore", "--no-watch"])
        .assert()
        .success();

    let wt_ws = workspace_for(&wt_path);
    assert!(
        indexed_files(&base_ws).contains("ignored.rs"),
        "base must be upgraded to supply inherited ignored content"
    );
    assert!(
        indexed_files(&base_ws).contains("ignored_untracked.rs"),
        "base must include ignored untracked content when requested"
    );
    assert_eq!(
        overlay_counts(&wt_ws),
        (0, 1),
        "the worktree should inherit tracked ignored content and tombstone the base-only untracked file"
    );
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    let included_hits = hybrid_search(
        &wt_ws,
        "inherited_ignored_marker",
        Some(&model),
        &SearchOptions {
            skip_gitignore: true,
            ..SearchOptions::default()
        },
    )
    .unwrap();
    assert!(
        included_hits
            .iter()
            .any(|hit| hit.file_path.to_string_lossy().contains("ignored.rs")),
        "worktree search including ignored files must find inherited base content"
    );
    assert!(
        search_file_paths(&wt_ws, "inherited_ignored_marker")
            .iter()
            .all(|path| !path.contains("ignored.rs")),
        "default worktree search must still exclude ignored base content"
    );

    let base_generation = base_ws.read_metadata().unwrap().unwrap().index_generation;
    fs::write(
        root.path().join("ignored_untracked.rs"),
        "pub fn ignored_untracked_v2() -> bool { true }\n",
    )
    .unwrap();
    assert!(
        std::process::Command::new("git")
            .args(["status", "--porcelain=v1"])
            .current_dir(root.path())
            .output()
            .unwrap()
            .stdout
            .is_empty(),
        "ignored untracked changes must remain invisible to Git status"
    );

    git(root.path(), &["branch", "wt-ignore-refresh", "main"]);
    let refresh_dir = tempdir().unwrap();
    let refresh_path = refresh_dir.path().join("wt_ignore_refresh");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            refresh_path.to_str().unwrap(),
            "wt-ignore-refresh",
        ],
    );
    let mut refresh_index = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    refresh_index
        .current_dir(&refresh_path)
        .env("IVYGREP_HOME", home.path())
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--add", ".", "--hash", "--skip-gitignore", "--no-watch"])
        .assert()
        .success();

    assert!(
        stored_chunk_text(&base_ws, "ignored_untracked.rs")
            .is_some_and(|text| text.contains("ignored_untracked_v2")),
        "ignored untracked changes must trigger base reconciliation"
    );
    assert!(
        base_ws.read_metadata().unwrap().unwrap().index_generation > base_generation,
        "reconciling ignored files must advance the shared base generation"
    );

    git(
        root.path(),
        &[
            "worktree",
            "remove",
            refresh_path.to_str().unwrap(),
            "--force",
        ],
    );
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

    init_git_repo(root.path());

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
// WORKTREE OVERLAY: an incompatible base index format forces a base rebuild
// ===========================================================================

#[test]
#[serial]
fn worktree_rejects_and_rebuilds_incompatible_base_index_formats() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    for i in 0..5 {
        fs::write(
            root.path().join(format!("base_{i}.rs")),
            format!("pub fn base_fn_{i}() -> usize {{ {i} }}\n"),
        )
        .unwrap();
    }
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base files"]);

    git(root.path(), &["checkout", "-b", "wt-fmt"]);
    fs::write(
        root.path().join("wt_only.rs"),
        "pub fn worktree_marker() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "worktree file"]);
    git(root.path(), &["checkout", "main"]);

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_fmt");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt-fmt"],
    );

    // Index base, then the worktree overlay referencing it.
    setup_and_index(root.path(), home.path());
    setup_and_index(&wt_path, home.path());

    let base_ws = workspace_for(root.path());
    let wt_ws = workspace_for(&wt_path);
    assert!(wt_ws.is_worktree(), "should detect as worktree");
    assert_eq!(
        base_ws.read_index_format_version(),
        ivygrep::workspace::INDEX_FORMAT_VERSION,
        "base should be indexed at the current format"
    );
    // Worktree finds inherited base content.
    assert!(
        search_file_paths(&wt_ws, "base_fn_2")
            .iter()
            .any(|p| p.contains("base_2.rs")),
        "worktree should find inherited base content before migration"
    );

    // The overlay's current format and generation do not make inherited
    // source text safe. Reject both pre-containment and future base formats.
    for incompatible_format in [25, ivygrep::workspace::INDEX_FORMAT_VERSION + 1] {
        fs::write(
            base_ws.index_format_version_path(),
            incompatible_format.to_string(),
        )
        .unwrap();
        assert_eq!(base_ws.read_index_format_version(), incompatible_format);
        assert_eq!(
            wt_ws.read_index_format_version(),
            ivygrep::workspace::INDEX_FORMAT_VERSION
        );
        assert!(!wt_ws.worktree_overlay_is_stale().unwrap());
        let expected = format!(
            "base index format incompatible (v{incompatible_format} != v{}); rebuild required",
            ivygrep::workspace::INDEX_FORMAT_VERSION
        );
        let error = SearchContext::load(&wt_ws, None, false)
            .err()
            .expect("a current overlay must not inherit incompatible source text");
        assert_eq!(error.to_string(), expected);
        let error =
            hybrid_search(&wt_ws, "base_fn_2", None, &SearchOptions::default()).unwrap_err();
        assert_eq!(error.to_string(), expected);
        let error = ivygrep::symbols::search_symbols(
            &wt_ws,
            "base_fn_2",
            ivygrep::symbols::SymbolSearchMode::Definitions,
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(error.to_string(), expected);

        // Worktree indexing rebuilds the base and refreshes the overlay.
        setup_and_index(&wt_path, home.path());
        assert_eq!(
            base_ws.read_index_format_version(),
            ivygrep::workspace::INDEX_FORMAT_VERSION,
            "incompatible base index should be rebuilt during worktree indexing"
        );
        assert!(
            search_file_paths(&wt_ws, "base_fn_2")
                .iter()
                .any(|path| path.contains("base_2.rs")),
            "worktree should still find inherited base content after base migration"
        );
    }

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn worktree_inherits_sources_across_line_ending_styles() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    git(root.path(), &["config", "core.autocrlf", "true"]);
    fs::write(
        root.path().join("shared.rs"),
        "pub fn shared_marker() -> bool {\n    true\n}\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "add shared source"]);

    let main_bytes = fs::read(root.path().join("shared.rs")).unwrap();
    assert!(!main_bytes.windows(2).any(|window| window == b"\r\n"));
    setup_and_index(root.path(), home.path());

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("worktree");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            "-b",
            "line-endings",
            wt_path.to_str().unwrap(),
            "main",
        ],
    );

    let worktree_bytes = fs::read(wt_path.join("shared.rs")).unwrap();
    assert!(worktree_bytes.windows(2).any(|window| window == b"\r\n"));

    let summary = setup_and_index(&wt_path, home.path());
    assert_eq!(
        summary.indexed_files, 0,
        "line-ending conversion must not materialize inherited sources"
    );
    assert_eq!(overlay_counts(&workspace_for(&wt_path)), (0, 0));
}

// ===========================================================================
// WORKTREE OVERLAY: an outdated empty overlay format forces a thin rebuild
// ===========================================================================

#[test]
#[serial]
fn worktree_rebuilds_outdated_zero_delta_overlay_format() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    fs::write(root.path().join(".gitignore"), ".git\n").unwrap();
    fs::write(root.path().join("base.rs"), "pub fn base_marker() {}\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base"]);

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_empty");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            "-b",
            "wt-empty",
            wt_path.to_str().unwrap(),
            "main",
        ],
    );

    setup_and_index(root.path(), home.path());
    setup_and_index(&wt_path, home.path());

    let wt_ws = workspace_for(&wt_path);
    let overlay_chunk_count = {
        let conn = open_sqlite(&wt_ws.overlay_sqlite_path()).unwrap();
        conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap()
    };
    assert_eq!(overlay_chunk_count, 0, "test requires a zero-delta overlay");

    fs::write(
        wt_ws.index_format_version_path(),
        (ivygrep::workspace::INDEX_FORMAT_VERSION - 1).to_string(),
    )
    .unwrap();
    assert!(
        wt_ws.quick_index_health().needs_rebuild(),
        "old-format zero-delta overlay must be rebuilt, issues={:?}",
        wt_ws.quick_index_health().issues
    );

    let summary = setup_and_index(&wt_path, home.path());
    assert_eq!(
        summary.indexed_files, 0,
        "rebuilding an unchanged overlay must not materialize base files"
    );
    let rebuilt_chunk_count = {
        let conn = open_sqlite(&wt_ws.overlay_sqlite_path()).unwrap();
        conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap()
    };
    assert_eq!(
        rebuilt_chunk_count, 0,
        "upgraded zero-delta overlay must remain thin"
    );
    assert_eq!(
        wt_ws.read_index_format_version(),
        ivygrep::workspace::INDEX_FORMAT_VERSION,
        "rebuilt overlay must carry current format"
    );
    assert!(
        search_file_paths(&wt_ws, "base_marker")
            .iter()
            .any(|path| path.contains("base.rs")),
        "rebuilt overlay must still query inherited base content"
    );

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

// ===========================================================================
// WORKTREE OVERLAY: healthy after indexing; no-change reindex is a true no-op
// ===========================================================================

#[test]
#[serial]
fn worktree_overlay_is_healthy_and_noop_on_no_change() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    fs::write(root.path().join("base.rs"), "pub fn base() {}\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base"]);

    git(root.path(), &["checkout", "-b", "wt"]);
    fs::write(
        root.path().join("wt_only.rs"),
        "pub fn worktree_only_fn() {}\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "wt file"]);
    git(root.path(), &["checkout", "main"]);

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt");
    git(
        root.path(),
        &["worktree", "add", wt_path.to_str().unwrap(), "wt"],
    );

    setup_and_index(root.path(), home.path()); // base
    setup_and_index(&wt_path, home.path()); // overlay

    // A freshly-indexed overlay must be healthy (not flagged for rebuild).
    let wt_ws = workspace_for(&wt_path);
    assert!(
        !wt_ws.quick_index_health().needs_rebuild(),
        "freshly indexed worktree overlay should be healthy, issues={:?}",
        wt_ws.quick_index_health().issues
    );

    // And a no-change reindex must be a true no-op: stores untouched.
    let vec_path = wt_ws.overlay_vector_path();
    assert!(vec_path.exists(), "overlay vector store should exist");
    let mtime_before = fs::metadata(&vec_path).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));

    let s = setup_and_index(&wt_path, home.path());
    assert_eq!(
        s.indexed_files, 0,
        "no-change overlay reindex indexes nothing"
    );
    let mtime_after = fs::metadata(&vec_path).unwrap().modified().unwrap();
    assert_eq!(
        mtime_before, mtime_after,
        "a no-change overlay reindex must not rewrite the vector store"
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

    init_git_repo(root.path());
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

#[test]
#[serial]
fn worktree_doc_include_change_reindexes_base_owned_rust_module() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::create_dir_all(root.path().join("docs")).unwrap();
    fs::write(
        root.path().join("src/lib.rs"),
        "#![doc = include_str!(\"../docs/router.md\")]\n",
    )
    .unwrap();
    fs::write(
        root.path().join("docs/router.md"),
        "baseonlyzephyr router layering explanation\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base doc include"]);
    setup_and_index(root.path(), home.path());

    git(root.path(), &["checkout", "-b", "wt-doc-include"]);
    git(root.path(), &["checkout", "main"]);
    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_doc_include");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            wt_path.to_str().unwrap(),
            "wt-doc-include",
        ],
    );
    setup_and_index(&wt_path, home.path());

    fs::write(
        wt_path.join("docs/router.md"),
        "worktreeonlynebula router layering replacement\n",
    )
    .unwrap();
    let refresh = setup_and_index(&wt_path, home.path());
    assert!(
        refresh.indexed_files >= 2,
        "the changed include and inherited owner should enter the overlay"
    );
    let workspace = workspace_for(&wt_path);
    assert!(
        search_file_paths(&workspace, "worktreeonlynebula")
            .iter()
            .any(|path| path == "src/lib.rs"),
        "worktree include content should be searchable through its Rust owner"
    );
    let overlay_owner_text =
        stored_chunk_texts_at(&workspace.overlay_sqlite_path(), "src/lib.rs").join("\n");
    assert!(
        overlay_owner_text.contains("worktreeonlynebula")
            && !overlay_owner_text.contains("baseonlyzephyr"),
        "overlay owner must contain only current dependency text: {overlay_owner_text}"
    );
    let conn = open_sqlite(&workspace.overlay_sqlite_path()).unwrap();
    let owner_tombstoned = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tombstones WHERE file_path = 'src/lib.rs')",
            [],
            |row| row.get::<_, bool>(0),
        )
        .unwrap();
    assert!(owner_tombstoned, "overlay owner must hide the base owner");
    drop(conn);

    fs::write(
        wt_path.join("docs/router.md"),
        "baseonlyzephyr router layering explanation\n",
    )
    .unwrap();
    setup_and_index(&wt_path, home.path());
    assert!(
        search_file_paths(&workspace, "baseonlyzephyr")
            .iter()
            .any(|path| path == "src/lib.rs"),
        "returning the dependency to base content should refresh the owner"
    );
    let restored_owner_text =
        stored_chunk_texts_at(&workspace.overlay_sqlite_path(), "src/lib.rs").join("\n");
    assert!(
        restored_owner_text.contains("baseonlyzephyr")
            && !restored_owner_text.contains("worktreeonlynebula"),
        "restored owner must replace stale overlay dependency text: {restored_owner_text}"
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

    init_git_repo(root.path());
    fs::write(
        root.path().join("shared.rs"),
        "pub fn shared_func() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "shared base"]);

    setup_and_index(root.path(), home.path());
    let base_ws = workspace_for(root.path());
    let base_generation = base_ws.read_metadata().unwrap().unwrap().index_generation;

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
    let summary_a = setup_and_index(&wt_a_path, home.path());
    let summary_b = setup_and_index(&wt_b_path, home.path());

    let ws_a = workspace_for(&wt_a_path);
    let ws_b = workspace_for(&wt_b_path);

    assert_eq!(summary_a.indexed_files, 1);
    assert_eq!(summary_a.deleted_files, 0);
    assert_eq!(summary_b.indexed_files, 1);
    assert_eq!(summary_b.deleted_files, 1);
    assert_eq!(overlay_counts(&ws_a), (1, 0));
    assert_eq!(overlay_counts(&ws_b), (1, 1));
    assert_eq!(ws_a.base_index_dir.as_ref(), Some(&base_ws.index_dir));
    assert_eq!(ws_b.base_index_dir.as_ref(), Some(&base_ws.index_dir));
    assert_only_overlay_stores(&ws_a);
    assert_only_overlay_stores(&ws_b);
    assert_eq!(
        base_ws.read_metadata().unwrap().unwrap().index_generation,
        base_generation,
        "indexing unchanged worktrees must not rewrite the shared base index"
    );

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

#[test]
#[serial]
fn concurrent_worktrees_auto_index_one_shared_base_and_store_only_deltas() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    for i in 0..40 {
        fs::write(
            root.path().join(format!("base_{i}.rs")),
            format!("pub fn shared_base_{i}() -> usize {{ {i} }}\n"),
        )
        .unwrap();
    }
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "shared base"]);

    git(root.path(), &["checkout", "-b", "concurrent-a"]);
    fs::write(
        root.path().join("delta_a.rs"),
        "pub fn concurrent_delta_a() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "delta a"]);
    git(root.path(), &["checkout", "main"]);

    git(root.path(), &["checkout", "-b", "concurrent-b"]);
    fs::write(
        root.path().join("delta_b.rs"),
        "pub fn concurrent_delta_b() -> bool { true }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "delta b"]);
    git(root.path(), &["checkout", "main"]);

    let wt_a_dir = tempdir().unwrap();
    let wt_a_path = wt_a_dir.path().join("wt_a");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            wt_a_path.to_str().unwrap(),
            "concurrent-a",
        ],
    );
    let wt_b_dir = tempdir().unwrap();
    let wt_b_path = wt_b_dir.path().join("wt_b");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            wt_b_path.to_str().unwrap(),
            "concurrent-b",
        ],
    );

    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let index_worktree = |path: std::path::PathBuf, barrier: std::sync::Arc<std::sync::Barrier>| {
        std::thread::spawn(move || {
            let workspace = Workspace::resolve(&path).unwrap();
            let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
            barrier.wait();
            index_workspace(&workspace, &model).unwrap()
        })
    };
    let handle_a = index_worktree(wt_a_path.clone(), barrier.clone());
    let handle_b = index_worktree(wt_b_path.clone(), barrier.clone());
    barrier.wait();
    let summary_a = handle_a.join().unwrap();
    let summary_b = handle_b.join().unwrap();

    let base_ws = workspace_for(root.path());
    let ws_a = workspace_for(&wt_a_path);
    let ws_b = workspace_for(&wt_b_path);
    assert_eq!(indexed_files(&base_ws).len(), 40);
    assert_eq!(
        base_ws.read_metadata().unwrap().unwrap().index_generation,
        1,
        "concurrent worktree startup must build the shared base only once"
    );
    assert_eq!(summary_a.indexed_files, 1);
    assert_eq!(summary_b.indexed_files, 1);
    assert_eq!(overlay_counts(&ws_a), (1, 0));
    assert_eq!(overlay_counts(&ws_b), (1, 0));
    assert_eq!(ws_a.base_index_dir.as_ref(), Some(&base_ws.index_dir));
    assert_eq!(ws_b.base_index_dir.as_ref(), Some(&base_ws.index_dir));
    assert_only_overlay_stores(&ws_a);
    assert_only_overlay_stores(&ws_b);

    assert!(
        search_file_paths(&ws_a, "shared_base_17")
            .iter()
            .any(|path| path.contains("base_17.rs"))
    );
    assert!(
        search_file_paths(&ws_b, "shared_base_17")
            .iter()
            .any(|path| path.contains("base_17.rs"))
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

    init_git_repo(root.path());
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

    init_git_repo(root.path());
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
fn worktree_search_reconciles_base_only_additions_before_loading_context() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    init_git_repo(root.path());
    fs::write(root.path().join("shared.rs"), "pub fn shared_api() {}\n").unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base"]);
    setup_and_index(root.path(), home.path());

    git(root.path(), &["checkout", "-b", "worktree-branch"]);
    fs::write(
        root.path().join("worktree.rs"),
        "pub fn worktree_only_api() {}\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "worktree"]);
    git(root.path(), &["checkout", "main"]);

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_search_reconcile");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            wt_path.to_str().unwrap(),
            "worktree-branch",
        ],
    );
    setup_and_index(&wt_path, home.path());
    let wt_workspace = workspace_for(&wt_path);

    fs::write(
        root.path().join("main_only.rs"),
        "pub fn main_branch_generation_marker() {}\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "main-only addition"]);
    setup_and_index(root.path(), home.path());

    assert!(!wt_path.join("main_only.rs").exists());
    assert!(wt_workspace.worktree_overlay_is_stale().unwrap());

    let results = search_file_paths(&wt_workspace, "main_branch_generation_marker");
    assert!(
        results.iter().all(|path| !path.contains("main_only.rs")),
        "search must not return a base-only file absent from the worktree: {results:?}"
    );
    assert!(
        !wt_workspace.worktree_overlay_is_stale().unwrap(),
        "search should reconcile the overlay before loading its context"
    );

    let worktree_results = search_file_paths(&wt_workspace, "worktree_only_api");
    assert!(
        worktree_results
            .iter()
            .any(|path| path.contains("worktree.rs")),
        "reconciliation must preserve worktree-only content"
    );

    fs::write(wt_workspace.base_ref_path(), b"{malformed").unwrap();
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(wt_workspace.lock_path())
        .unwrap();
    fs2::FileExt::lock_exclusive(&lock_file).unwrap();

    let reconcile_workspace = wt_workspace.clone();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let reconcile = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        reconcile_worktree_overlay(&reconcile_workspace, &model)
    });
    started_rx.recv().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));

    assert!(
        wt_workspace.overlay_sqlite_path().exists(),
        "reconciliation must not clear overlay storage before acquiring index.lock"
    );
    assert!(
        wt_workspace.base_ref_path().exists(),
        "reconciliation must not clear the base reference before acquiring index.lock"
    );

    fs2::FileExt::unlock(&lock_file).unwrap();
    assert!(reconcile.join().unwrap().unwrap());
    assert!(!wt_workspace.worktree_overlay_is_stale().unwrap());

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn worktree_context_graph_prefers_overlay_dependencies() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();
    init_git_repo(root.path());
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"worktree-graph\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        root.path().join("src/auth.rs"),
        "use crate::old_clock::now;\npub fn rotate_refresh_token() { now(); }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("src/old_clock.rs"),
        "pub fn now() -> u64 { 1 }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("src/new_clock.rs"),
        "pub fn now() -> u64 { 2 }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("src/session.rs"),
        "use crate::policy::validate_policy;\npub fn refresh_session_orchestration() { validate_policy(); }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("src/policy.rs"),
        "pub fn validate_policy() -> &'static str { \"base-policy\" }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base graph"]);
    setup_and_index(root.path(), home.path());

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_context_graph");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            "-b",
            "context-graph-worktree",
            wt_path.to_str().unwrap(),
        ],
    );
    fs::write(
        wt_path.join("src/auth.rs"),
        "use crate::new_clock::now;\npub fn rotate_refresh_token() { now(); }\n",
    )
    .unwrap();
    fs::write(
        wt_path.join("src/policy.rs"),
        "pub fn validate_policy() -> &'static str { \"worktree-policy\" }\n",
    )
    .unwrap();
    setup_and_index(&wt_path, home.path());

    let workspace = workspace_for(&wt_path);
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    let bundle = build_context_bundle(
        &workspace,
        "rotate refresh token expiration",
        Some(&model),
        &SearchOptions::default(),
        4_000,
    )
    .unwrap();
    let dependency_paths = bundle
        .items
        .iter()
        .filter(|item| item.roles.contains(&ContextRole::Dependency))
        .map(|item| item.file_path.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(
        dependency_paths
            .iter()
            .any(|path| path == "src/new_clock.rs"),
        "{bundle:#?}"
    );
    assert!(
        dependency_paths
            .iter()
            .all(|path| path != "src/old_clock.rs"),
        "{bundle:#?}"
    );

    let bundle = build_context_bundle(
        &workspace,
        "refresh session orchestration",
        Some(&model),
        &SearchOptions::default(),
        4_000,
    )
    .unwrap();
    let policy = bundle.items.iter().find(|item| {
        item.file_path == std::path::Path::new("src/policy.rs")
            && item.roles.contains(&ContextRole::Dependency)
    });
    assert!(
        policy.is_some_and(|item| {
            item.preview.contains("worktree-policy") && !item.preview.contains("base-policy")
        }),
        "{bundle:#?}"
    );

    git(
        root.path(),
        &["worktree", "remove", wt_path.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn worktree_restored_dependency_wakes_overlay_importer() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();
    init_git_repo(root.path());
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::write(
        root.path().join("src/lib.rs"),
        "use crate::helper::work;\npub fn restore_overlay_dependency_wakeup() { work(); }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("src/helper.rs"),
        "pub fn work() -> &'static str { \"base-target\" }\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base dependency"]);
    setup_and_index(root.path(), home.path());

    let wt_dir = tempdir().unwrap();
    let wt_path = wt_dir.path().join("wt_restored_dependency");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            "-b",
            "restored-dependency-worktree",
            wt_path.to_str().unwrap(),
        ],
    );
    fs::remove_file(wt_path.join("src/helper.rs")).unwrap();
    fs::write(
        wt_path.join("src/lib.rs"),
        "use crate::helper::work;\n// overlay importer\npub fn restore_overlay_dependency_wakeup() { work(); }\n",
    )
    .unwrap();
    setup_and_index(&wt_path, home.path());

    let workspace = workspace_for(&wt_path);
    let conn = open_sqlite(&workspace.overlay_sqlite_path()).unwrap();
    let unresolved = conn
        .query_row(
            "SELECT COUNT(DISTINCT spec) FROM unresolved_file_dependencies
             WHERE source_path = 'src/lib.rs' AND spec = 'crate::helper::work'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(unresolved, 1);
    drop(conn);

    fs::write(
        wt_path.join("src/helper.rs"),
        "pub fn work() -> &'static str { \"base-target\" }\n",
    )
    .unwrap();
    let summary = setup_and_index(&wt_path, home.path());
    assert_eq!(summary.indexed_files, 1);

    let conn = open_sqlite(&workspace.overlay_sqlite_path()).unwrap();
    let edge = conn
        .query_row(
            "SELECT COUNT(*) FROM file_edges
             WHERE source_path = 'src/lib.rs'
               AND target_path = 'src/helper.rs'
               AND kind = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(edge, 1);
    let unresolved = conn
        .query_row(
            "SELECT COUNT(*) FROM unresolved_file_dependencies
             WHERE source_path = 'src/lib.rs' AND spec = 'crate::helper::work'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(unresolved, 0);
    let overlay_target_chunks = conn
        .query_row(
            "SELECT COUNT(*) FROM chunks WHERE file_path = 'src/helper.rs'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(overlay_target_chunks, 0);
    drop(conn);

    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    let bundle = build_context_bundle(
        &workspace,
        "restore overlay dependency wakeup",
        Some(&model),
        &SearchOptions::default(),
        4_000,
    )
    .unwrap();
    assert!(
        bundle.items.iter().any(|item| {
            item.file_path == std::path::Path::new("src/helper.rs")
                && item.roles.contains(&ContextRole::Dependency)
                && item.preview.contains("base-target")
        }),
        "{bundle:#?}"
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

    init_git_repo(root.path());
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
        &[
            "worktree",
            "add",
            wt_path.to_str().unwrap(),
            "wt-e2e-branch",
        ],
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
