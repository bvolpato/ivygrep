//! Concurrency tests for ivygrep.
//!
//! Validates that:
//!  1. Concurrent searches do not corrupt results or panic.
//!  2. Indexing while searches are running does not cause crashes.
//!  3. Multiple sequential index+search cycles are safe (simulates non-daemon use).
//!  4. Rapid CRUD + search interleaving is safe (simulates daemon-like behavior).
//!  5. Parallel index calls serialize via file lock (no corruption).
//!  6. Concurrent CLI binary invocations are safe.

use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;

use serial_test::serial;
use tempfile::tempdir;

use ivygrep::EMBEDDING_DIMENSIONS;
use ivygrep::embedding::HashEmbeddingModel;
use ivygrep::indexer::{index_workspace, open_sqlite};
use ivygrep::search::{SearchOptions, hybrid_search};
use ivygrep::workspace::Workspace;

/// Helper: create a workspace with source files and index it.
fn make_indexed_workspace(
    file_count: usize,
) -> (
    tempfile::TempDir,
    tempfile::TempDir,
    Workspace,
    HashEmbeddingModel,
) {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    for i in 0..file_count {
        fs::write(
            root.path().join(format!("module_{i}.rs")),
            format!(
                "/// Module {i} documentation\npub fn compute_{i}(x: f64) -> f64 {{ x * {}.0 }}\n",
                i + 1
            ),
        )
        .unwrap();
    }

    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let workspace = Workspace::resolve(root.path()).unwrap();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    index_workspace(&workspace, &model).unwrap();

    (root, home, workspace, model)
}

// ---------------------------------------------------------------------------
// 1. Concurrent reads (parallel searches) — no daemon
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn concurrent_searches_do_not_panic() {
    let (_root, _home, workspace, model) = make_indexed_workspace(10);

    let ws = Arc::new(workspace);
    let m = Arc::new(model);
    let barrier = Arc::new(Barrier::new(8));

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let ws = Arc::clone(&ws);
            let m = Arc::clone(&m);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait(); // all threads start simultaneously
                let queries = ["compute", "module documentation", "function", "f64"];
                let query = queries[i % queries.len()];
                let opts = SearchOptions::default();
                let result = hybrid_search(&ws, query, Some(m.as_ref()), &opts);
                assert!(
                    result.is_ok(),
                    "search '{query}' on thread {i} failed: {result:?}"
                );
                result.unwrap()
            })
        })
        .collect();

    for h in handles {
        let hits = h.join().expect("thread panicked");
        assert!(!hits.is_empty(), "search should return results");
    }
}

// ---------------------------------------------------------------------------
// 2. Read-while-write: search while re-indexing (non-daemon CLI pattern)
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn search_during_reindex_does_not_crash() {
    let (root, _home, workspace, model) = make_indexed_workspace(15);

    let ws = Arc::new(workspace);
    let m = Arc::new(model);
    let barrier = Arc::new(Barrier::new(2));

    // Thread A: re-index (simulating file changes + index_workspace)
    let ws_a = Arc::clone(&ws);
    let m_a = Arc::clone(&m);
    let root_path = root.path().to_path_buf();
    let barrier_a = Arc::clone(&barrier);
    let indexer_handle = thread::spawn(move || {
        barrier_a.wait();
        // Mutate some files
        for i in 0..5 {
            fs::write(
                root_path.join(format!("module_{i}.rs")),
                format!(
                    "pub fn compute_{i}_v2(x: f64) -> f64 {{ x * {}.0 }}\n",
                    i * 10
                ),
            )
            .unwrap();
        }
        let result = index_workspace(&ws_a, m_a.as_ref());
        // The indexer should succeed
        assert!(result.is_ok(), "indexing failed: {result:?}");
    });

    // Thread B: search concurrently
    let ws_b = Arc::clone(&ws);
    let m_b = Arc::clone(&m);
    let barrier_b = Arc::clone(&barrier);
    let search_handle = thread::spawn(move || {
        barrier_b.wait();
        // Run multiple searches while indexer is running
        for _ in 0..5 {
            let opts = SearchOptions::default();
            let result = hybrid_search(&ws_b, "compute function", Some(m_b.as_ref()), &opts);
            // The search might see old or new data, but must NOT crash
            match result {
                Ok(_hits) => {} // success: might be old or new data
                Err(e) => {
                    // Read errors during concurrent write are acceptable
                    // as long as they don't panic.
                    eprintln!("Acceptable search error during concurrent write: {e}");
                }
            }
            thread::sleep(std::time::Duration::from_millis(10));
        }
    });

    indexer_handle.join().expect("indexer panicked");
    search_handle.join().expect("searcher panicked");
}

// ---------------------------------------------------------------------------
// 3. Sequential index-search-modify-search cycles (typical non-daemon use)
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn sequential_crud_search_cycles() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

    // Cycle 1: create and search
    fs::write(
        root.path().join("app.rs"),
        "pub fn calculate_tax(amount: f64) -> f64 { amount * 0.15 }\n",
    )
    .unwrap();
    let ws = Workspace::resolve(root.path()).unwrap();
    index_workspace(&ws, &model).unwrap();

    let hits = hybrid_search(&ws, "tax calculation", Some(&model), &SearchOptions::default()).unwrap();
    assert!(!hits.is_empty(), "cycle 1: should find tax");

    // Cycle 2: modify and search
    fs::write(
        root.path().join("app.rs"),
        "pub fn calculate_discount(price: f64) -> f64 { price * 0.10 }\n",
    )
    .unwrap();
    index_workspace(&ws, &model).unwrap();

    let hits = hybrid_search(
        &ws,
        "discount calculation",
        Some(&model),
        &SearchOptions::default(),
    )
    .unwrap();
    assert!(!hits.is_empty(), "cycle 2: should find discount");

    // The old content should not be in any chunk
    let conn = open_sqlite(&ws.sqlite_path()).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM chunks WHERE text LIKE '%calculate_tax%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 0,
        "old function should be completely gone from chunks"
    );

    // Cycle 3: delete and search
    fs::remove_file(root.path().join("app.rs")).unwrap();
    index_workspace(&ws, &model).unwrap();

    let conn = open_sqlite(&ws.sqlite_path()).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        count, 0,
        "cycle 3: all chunks should be gone after file delete"
    );

    // Cycle 4: re-create and search
    fs::write(
        root.path().join("app.rs"),
        "pub fn process_payment(amount: f64) -> bool { amount > 0.0 }\n",
    )
    .unwrap();
    index_workspace(&ws, &model).unwrap();

    let hits = hybrid_search(&ws, "process payment", Some(&model), &SearchOptions::default()).unwrap();
    assert!(
        !hits.is_empty(),
        "cycle 4: should find newly created content"
    );
}

// ---------------------------------------------------------------------------
// 4. Rapid interleaved CRUD + search (simulates daemon-like behavior)
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn rapid_interleaved_crud_and_search() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

    // Create initial files
    for i in 0..5 {
        fs::write(
            root.path().join(format!("file_{i}.rs")),
            format!("fn original_{i}() -> usize {{ {i} }}\n"),
        )
        .unwrap();
    }

    let ws = Workspace::resolve(root.path()).unwrap();
    index_workspace(&ws, &model).unwrap();

    // 20 rapid cycles of: mutate → index → search
    for cycle in 0..20 {
        let file_idx = cycle % 5;
        let filename = format!("file_{file_idx}.rs");

        // Alternate between modify, delete+recreate, and add-new
        match cycle % 3 {
            0 => {
                // Modify existing
                fs::write(
                    root.path().join(&filename),
                    format!("fn modified_{file_idx}_c{cycle}() -> usize {{ {cycle} }}\n"),
                )
                .unwrap();
            }
            1 => {
                // Delete and recreate
                let _ = fs::remove_file(root.path().join(&filename));
                fs::write(
                    root.path().join(&filename),
                    format!("fn recreated_{file_idx}_c{cycle}() -> usize {{ {cycle} }}\n"),
                )
                .unwrap();
            }
            2 => {
                // Add temp file, will be removed next cycle
                fs::write(
                    root.path().join(format!("temp_{cycle}.rs")),
                    format!("fn temporary_{cycle}() {{}}\n"),
                )
                .unwrap();
                // Clean up old temp files
                if cycle >= 3 {
                    let _ = fs::remove_file(root.path().join(format!("temp_{}.rs", cycle - 3)));
                }
            }
            _ => unreachable!(),
        }

        // Re-index
        let summary = index_workspace(&ws, &model).unwrap();
        assert!(
            summary.total_chunks > 0,
            "cycle {cycle}: should have chunks after indexing"
        );

        // Search
        let result = hybrid_search(&ws, "function", Some(&model), &SearchOptions::default());
        assert!(
            result.is_ok(),
            "cycle {cycle}: search failed: {:?}",
            result.err()
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Parallel index calls serialize via file lock (no corruption)
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn parallel_index_calls_serialize_via_lock() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    for i in 0..10 {
        fs::write(
            root.path().join(format!("src_{i}.rs")),
            format!("fn func_{i}() {{ }}\n"),
        )
        .unwrap();
    }

    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let workspace = Workspace::resolve(root.path()).unwrap();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

    // First index to establish baseline
    index_workspace(&workspace, &model).unwrap();

    // Now run 4 parallel index_workspace calls (no mutations — all should be no-ops).
    // The file lock ensures they serialize, so all should succeed.
    let ws = Arc::new(workspace);
    let m = Arc::new(model);
    let barrier = Arc::new(Barrier::new(4));

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let ws = Arc::clone(&ws);
            let m = Arc::clone(&m);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                // With file locking, all calls should succeed (serialized)
                let result = index_workspace(&ws, m.as_ref());
                assert!(result.is_ok(), "index_workspace failed: {:?}", result.err());
                let summary = result.unwrap();
                assert_eq!(summary.indexed_files, 0, "no-op indexing");
                assert_eq!(summary.deleted_files, 0, "no-op indexing");
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked during parallel indexing");
    }

    // After all parallel calls, the index should be consistent
    let conn = open_sqlite(&ws.sqlite_path()).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(DISTINCT file_path) FROM chunks", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 10, "all 10 files should be in the index");
}

// ---------------------------------------------------------------------------
// 6. CLI binary concurrent invocations (end-to-end)
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn concurrent_cli_searches_via_binary() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();

    for i in 0..5 {
        fs::write(
            root.path().join(format!("mod_{i}.rs")),
            format!(
                "/// Search target {i}\npub fn compute_{i}(value: f64) -> f64 {{ value * {}.0 }}\n",
                i + 1
            ),
        )
        .unwrap();
    }

    // Index first (single process, no contention)
    let ivygrep_bin = assert_cmd::cargo::cargo_bin!("ig");
    let output = std::process::Command::new(ivygrep_bin)
        .arg("--add")
        .arg(root.path())
        .env("IVYGREP_HOME", home.path())
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "index failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Launch 6 concurrent searches.
    // Use -f to also test that re-indexing serializes properly via the lock.
    let barrier = Arc::new(Barrier::new(6));
    let handles: Vec<_> = (0..6)
        .map(|i| {
            let root_path = root.path().to_path_buf();
            let home_path = home.path().to_path_buf();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let queries = [
                    "compute",
                    "search target",
                    "function",
                    "value",
                    "pub fn",
                    "f64",
                ];
                let query = queries[i % queries.len()];
                let bin = assert_cmd::cargo::cargo_bin!("ig");
                let output = std::process::Command::new(bin)
                    .args(["-f", "--json", query])
                    .arg(&root_path)
                    .env("IVYGREP_HOME", &home_path)
                    .env("IVYGREP_NO_AUTOSPAWN", "1")
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "search '{query}' process failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let stdout = String::from_utf8_lossy(&output.stdout);
                let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
                    panic!("invalid JSON from search '{query}': {e}\nstdout: {stdout}");
                });
                assert!(parsed.is_array(), "expected JSON array");
                (query.to_string(), parsed)
            })
        })
        .collect();

    for h in handles {
        let (query, _json) = h.join().expect("cli search thread panicked");
        eprintln!("concurrent CLI search '{query}': OK");
    }
}
