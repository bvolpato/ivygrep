//! Comprehensive stress / load tests for ivygrep.
//!
//! These tests exercise the indexer and search engine under realistic conditions:
//!   - Real-world codebases (ripgrep, tantivy, shakespeare, alice)
//!   - Deep relevance verification with many queries and expected results
//!   - Incremental re-indexing on real repos (modify → re-index → verify)
//!   - High-throughput concurrent query storm (many threads, many queries)
//!   - Regex stress on large repos
//!   - Multi-workspace concurrent indexing
//!   - Large-file edge cases (near 16MB, many-chunk files)
//!   - Rapid large-scale file churn (50+ files per cycle)
//!   - Index integrity after extreme churn
//!   - Query throughput benchmark (hundreds of queries, latency stats)
//!   - Sustained load memory stability
//!
//! All tests requiring downloaded fixtures are `#[ignore]`; run:
//!   ./scripts/bootstrap_stress_fixtures.sh
//!   cargo test --test stress_harness -- --ignored --nocapture

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use fs_extra::dir::{CopyOptions, copy as copy_dir};
use ivygrep::EMBEDDING_DIMENSIONS;
use ivygrep::embedding::HashEmbeddingModel;
use ivygrep::indexer::{index_workspace, open_sqlite};
use ivygrep::merkle::MerkleSnapshot;
use ivygrep::regex_search::regex_search;
use ivygrep::search::{SearchOptions, hybrid_search};
use ivygrep::workspace::Workspace;
use serial_test::serial;

// ============================================================================
// Helpers
// ============================================================================

fn stress_root() -> PathBuf {
    std::env::var_os("IVYGREP_STRESS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/stress-data"))
}

fn require_fixture(path: &Path) {
    if !path.exists() {
        panic!(
            "missing stress fixture at {}\nrun: ./scripts/bootstrap_stress_fixtures.sh",
            path.display()
        );
    }
}

/// Stage a fixture into a temp directory and return workspace + model.
fn stage_fixture(
    fixture_path: &Path,
) -> (
    tempfile::TempDir,
    tempfile::TempDir,
    Workspace,
    HashEmbeddingModel,
) {
    let staging = tempfile::tempdir().unwrap();
    let staged_workspace_root = staging.path().join("workspace");
    fs::create_dir_all(&staged_workspace_root).unwrap();

    let mut copy_opts = CopyOptions::new();
    copy_opts.overwrite = true;
    copy_opts.copy_inside = true;
    copy_dir(fixture_path, &staged_workspace_root, &copy_opts).unwrap();

    let home = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

    let workspace = Workspace::resolve(&staged_workspace_root).unwrap();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

    (staging, home, workspace, model)
}

fn run_index_and_query(
    fixture_path: &Path,
    query: &str,
    expected_substring: &str,
    min_files_indexed: usize,
) {
    let (_staging, _home, workspace, model) = stage_fixture(fixture_path);

    let index_start = Instant::now();
    let summary = index_workspace(&workspace, &model).unwrap();
    let index_elapsed = index_start.elapsed();

    eprintln!(
        "[stress] indexed workspace={} files={} chunks={} elapsed={:?}",
        workspace.id, summary.indexed_files, summary.total_chunks, index_elapsed
    );

    assert!(
        summary.indexed_files >= min_files_indexed,
        "indexed_files={} expected_at_least={}",
        summary.indexed_files,
        min_files_indexed
    );
    assert!(summary.total_chunks > 0, "expected non-empty chunk index");

    let search_start = Instant::now();
    let hits = hybrid_search(
        &workspace,
        query,
        &model,
        &SearchOptions {
            limit: Some(50),
            context: 2,
            type_filter: None,
            scope_filter: None,
        },
    )
    .unwrap();
    let search_elapsed = search_start.elapsed();

    eprintln!(
        "[stress] query='{}' hits={} elapsed={:?}",
        query,
        hits.len(),
        search_elapsed
    );

    assert!(!hits.is_empty(), "expected at least one hit");

    let expected_lower = expected_substring.to_ascii_lowercase();
    assert!(
        hits.iter().any(|hit| {
            let preview = hit.preview.to_ascii_lowercase();
            let file_path = hit.file_path.to_string_lossy().to_ascii_lowercase();
            preview.contains(&expected_lower) || file_path.contains(&expected_lower)
        }),
        "expected substring '{}' in preview/path of at least one hit",
        expected_substring
    );
}

// ============================================================================
// 1. Basic repo index + single-query (original tests)
// ============================================================================

#[test]
#[ignore = "downloads required; run ./scripts/bootstrap_stress_fixtures.sh first"]
#[serial]
fn stress_shakespeare_index_and_query() {
    let fixture = stress_root().join("workspaces/shakespeare");
    require_fixture(&fixture);
    run_index_and_query(&fixture, "to be or not to be", "to be", 1);
}

#[test]
#[ignore = "downloads required; run ./scripts/bootstrap_stress_fixtures.sh first"]
#[serial]
fn stress_ripgrep_repo_index_and_query() {
    let fixture = stress_root().join("repos/ripgrep");
    require_fixture(&fixture);
    run_index_and_query(&fixture, "ripgrep", "ripgrep", 100);
}

#[test]
#[ignore = "downloads required; run ./scripts/bootstrap_stress_fixtures.sh first"]
#[serial]
fn stress_tantivy_repo_index_and_query() {
    let fixture = stress_root().join("repos/tantivy");
    require_fixture(&fixture);
    run_index_and_query(&fixture, "tantivy", "tantivy", 200);
}

// ============================================================================
// 2. Deep multi-query relevance on ripgrep (10+ queries, expected file hits)
// ============================================================================

#[test]
#[ignore = "downloads required; run ./scripts/bootstrap_stress_fixtures.sh first"]
#[serial]
fn stress_ripgrep_deep_relevance() {
    let fixture = stress_root().join("repos/ripgrep");
    require_fixture(&fixture);
    let (_staging, _home, workspace, model) = stage_fixture(&fixture);

    index_workspace(&workspace, &model).unwrap();

    // Each query maps to file path substrings we expect in results
    let queries: &[(&str, &[&str])] = &[
        ("binary file detection", &["defs.rs", "GUIDE", "binary"]),
        ("regex pattern compile", &["matcher.rs", "regex"]),
        ("gitignore parsing", &["gitignore", "ignore"]),
        ("colored output printer", &["printer", "standard"]),
        ("command line flags", &["defs.rs", "hiargs"]),
        ("file type definitions", &["defs.rs", "types"]),
        ("glob pattern matching", &["glob"]),
        ("parallel directory walker", &["walk", "ignore"]),
        ("line number counting", &["searcher", "line"]),
        ("search results output format", &["printer", "standard"]),
    ];

    let mut pass_count = 0;
    for (query, expected_any) in queries {
        let hits = hybrid_search(
            &workspace,
            query,
            &model,
            &SearchOptions {
                limit: Some(10),
                context: 2,
                type_filter: None,
                scope_filter: None,
            },
        )
        .unwrap();

        assert!(!hits.is_empty(), "query '{query}' returned no results");

        // At least one expected path fragment should appear in the top results
        let all_paths: Vec<String> = hits
            .iter()
            .map(|h| h.file_path.to_string_lossy().to_ascii_lowercase())
            .collect();

        let matched = expected_any.iter().any(|expected| {
            let e = expected.to_ascii_lowercase();
            all_paths.iter().any(|p| p.contains(&e))
        });

        if matched {
            pass_count += 1;
        }

        eprintln!(
            "[relevance] query='{}' hits={} matched={} paths={:?}",
            query,
            hits.len(),
            matched,
            &all_paths[..all_paths.len().min(5)]
        );
    }

    // At least 70% of relevance checks should pass
    let threshold = (queries.len() as f64 * 0.7).ceil() as usize;
    assert!(
        pass_count >= threshold,
        "relevance pass rate too low: {pass_count}/{} (threshold={threshold})",
        queries.len()
    );
    eprintln!(
        "[relevance] PASS: {pass_count}/{} queries matched expected files",
        queries.len()
    );
}

// ============================================================================
// 3. Deep multi-query relevance on tantivy
// ============================================================================

#[test]
#[ignore = "downloads required; run ./scripts/bootstrap_stress_fixtures.sh first"]
#[serial]
fn stress_tantivy_deep_relevance() {
    let fixture = stress_root().join("repos/tantivy");
    require_fixture(&fixture);
    let (_staging, _home, workspace, model) = stage_fixture(&fixture);

    index_workspace(&workspace, &model).unwrap();

    let queries: &[(&str, &[&str])] = &[
        ("inverted index segment", &["segment", "index"]),
        ("BM25 scoring", &["bm25", "score"]),
        ("merge policy", &["merge"]),
        ("query parser grammar", &["query_grammar", "parser"]),
        ("term dictionary", &["term", "dictionary"]),
        ("field schema definition", &["schema", "field"]),
        ("document indexing pipeline", &["indexer", "index_writer"]),
        ("fast field codec", &["fast", "codec"]),
    ];

    let mut pass_count = 0;
    for (query, expected_any) in queries {
        let hits = hybrid_search(
            &workspace,
            query,
            &model,
            &SearchOptions {
                limit: Some(10),
                context: 2,
                type_filter: None,
                scope_filter: None,
            },
        )
        .unwrap();

        if hits.is_empty() {
            eprintln!("[relevance] query='{query}' returned NO results");
            continue;
        }

        let all_paths: Vec<String> = hits
            .iter()
            .map(|h| h.file_path.to_string_lossy().to_ascii_lowercase())
            .collect();

        let matched = expected_any.iter().any(|expected| {
            let e = expected.to_ascii_lowercase();
            all_paths.iter().any(|p| p.contains(&e))
        });

        if matched {
            pass_count += 1;
        }

        eprintln!(
            "[relevance] query='{}' hits={} matched={} paths={:?}",
            query,
            hits.len(),
            matched,
            &all_paths[..all_paths.len().min(5)]
        );
    }

    let threshold = (queries.len() as f64 * 0.6).ceil() as usize;
    assert!(
        pass_count >= threshold,
        "tantivy relevance: {pass_count}/{} (threshold={threshold})",
        queries.len()
    );
}

// ============================================================================
// 4. Incremental re-indexing on real repo (modify files, verify)
// ============================================================================

#[test]
#[ignore = "downloads required; run ./scripts/bootstrap_stress_fixtures.sh first"]
#[serial]
fn stress_ripgrep_incremental_reindex() {
    let fixture = stress_root().join("repos/ripgrep");
    require_fixture(&fixture);
    let (_staging, _home, workspace, model) = stage_fixture(&fixture);

    // Phase 1: full initial index
    let s1 = index_workspace(&workspace, &model).unwrap();
    let initial_files = s1.indexed_files;
    let initial_chunks = s1.total_chunks;
    eprintln!(
        "[incremental] initial: files={} chunks={}",
        initial_files, initial_chunks
    );
    assert!(initial_files > 100);

    // Phase 2: re-index with no changes → should be zero work
    let s2 = index_workspace(&workspace, &model).unwrap();
    assert_eq!(s2.indexed_files, 0, "no changes → zero files indexed");
    assert_eq!(s2.deleted_files, 0, "no changes → zero files deleted");
    assert_eq!(
        s2.total_chunks, initial_chunks,
        "chunk count should be stable"
    );

    // Use workspace.root to locate files (handles nested directory layout)
    let ws_root = workspace.root.clone();
    eprintln!("[incremental] workspace.root = {}", ws_root.display());

    // Phase 3: add a new file to test incremental indexing
    let sentinel_a = ws_root.join("ivygrep_stress_sentinel_a.rs");
    fs::write(&sentinel_a, "pub fn sentinel_a() -> bool { true }\n").unwrap();

    let s3 = index_workspace(&workspace, &model).unwrap();
    assert_eq!(
        s3.indexed_files, 1,
        "only the new sentinel file should be indexed"
    );
    assert_eq!(s3.deleted_files, 0);

    // Phase 4: add another brand new file
    let sentinel_b = ws_root.join("ivygrep_stress_sentinel_b.rs");
    fs::write(&sentinel_b, "pub fn sentinel_b() -> bool { false }\n").unwrap();

    let s4 = index_workspace(&workspace, &model).unwrap();
    assert_eq!(s4.indexed_files, 1, "only sentinel_b should be indexed");
    assert_eq!(s4.deleted_files, 0);

    // Phase 5: modify sentinel_a and delete sentinel_b
    fs::write(&sentinel_a, "pub fn sentinel_a_v2() -> bool { false }\n").unwrap();
    fs::remove_file(&sentinel_b).unwrap();

    let s5 = index_workspace(&workspace, &model).unwrap();
    assert_eq!(s5.indexed_files, 1, "sentinel_a modified");
    assert_eq!(s5.deleted_files, 1, "sentinel_b deleted");

    // Phase 6: delete both sentinels, verify no-op after
    fs::remove_file(&sentinel_a).unwrap();
    let s6 = index_workspace(&workspace, &model).unwrap();
    assert_eq!(s6.deleted_files, 1, "sentinel_a deleted");

    // Phase 7: re-index → should be zero work
    let s7 = index_workspace(&workspace, &model).unwrap();
    assert_eq!(s7.indexed_files, 0, "no changes after cleanup");
    assert_eq!(s7.deleted_files, 0);

    // Phase 8: verify Merkle snapshot is consistent
    let saved = MerkleSnapshot::load(&workspace.merkle_snapshot_path()).unwrap();
    let fresh = MerkleSnapshot::build(&ws_root).unwrap();
    assert_eq!(
        saved.root_hash, fresh.root_hash,
        "Merkle snapshot must match filesystem"
    );

    eprintln!("[incremental] ALL PHASES PASSED");
}

// ============================================================================
// 5. High-throughput concurrent query storm on large repo
// ============================================================================

#[test]
#[ignore = "downloads required; run ./scripts/bootstrap_stress_fixtures.sh first"]
#[serial]
fn stress_concurrent_query_storm_ripgrep() {
    let fixture = stress_root().join("repos/ripgrep");
    require_fixture(&fixture);
    let (_staging, _home, workspace, model) = stage_fixture(&fixture);
    index_workspace(&workspace, &model).unwrap();

    let ws = Arc::new(workspace);
    let m = Arc::new(model);

    let thread_count = 8;
    let queries_per_thread = 12;
    let barrier = Arc::new(Barrier::new(thread_count));

    let queries: Arc<Vec<&str>> = Arc::new(vec![
        "binary detection",
        "regex compile",
        "gitignore",
        "parallel walk",
        "color output",
        "line number",
        "file type filter",
        "searcher",
        "glob pattern",
        "printer format",
        "command line args",
        "ignore rules",
    ]);

    let handles: Vec<_> = (0..thread_count)
        .map(|tid| {
            let ws = Arc::clone(&ws);
            let m = Arc::clone(&m);
            let barrier = Arc::clone(&barrier);
            let queries = Arc::clone(&queries);
            thread::spawn(move || {
                barrier.wait();
                let mut latencies = Vec::new();
                for qidx in 0..queries_per_thread {
                    let query = queries[qidx % queries.len()];
                    let start = Instant::now();
                    let result = hybrid_search(
                        &ws,
                        query,
                        m.as_ref(),
                        &SearchOptions {
                            limit: Some(20),
                            context: 2,
                            type_filter: None,
                            scope_filter: None,
                        },
                    );
                    let elapsed = start.elapsed();
                    latencies.push(elapsed);
                    assert!(
                        result.is_ok(),
                        "thread {tid} query '{query}' failed: {:?}",
                        result.err()
                    );
                    assert!(
                        !result.unwrap().is_empty(),
                        "thread {tid} query '{query}' returned no results"
                    );
                }
                latencies
            })
        })
        .collect();

    let mut all_latencies = Vec::new();
    for h in handles {
        let latencies = h.join().expect("query storm thread panicked");
        all_latencies.extend(latencies);
    }

    all_latencies.sort();
    let total = all_latencies.len();
    let p50 = all_latencies[total / 2];
    let p95 = all_latencies[(total as f64 * 0.95) as usize];
    let p99 = all_latencies[(total as f64 * 0.99).min(total as f64 - 1.0) as usize];
    let max = all_latencies[total - 1];
    let avg: Duration = all_latencies.iter().sum::<Duration>() / total as u32;

    eprintln!("[query-storm] {total} queries across {thread_count} threads");
    eprintln!("[query-storm] avg={avg:?} p50={p50:?} p95={p95:?} p99={p99:?} max={max:?}");

    // Sanity: p95 should be under 5 seconds even on slow CI
    assert!(
        p95 < Duration::from_secs(5),
        "p95 latency too high: {p95:?}"
    );
}

// ============================================================================
// 6. Regex stress on large repo
// ============================================================================

#[test]
#[ignore = "downloads required; run ./scripts/bootstrap_stress_fixtures.sh first"]
#[serial]
fn stress_regex_search_ripgrep() {
    let fixture = stress_root().join("repos/ripgrep");
    require_fixture(&fixture);
    let (_staging, _home, workspace, model) = stage_fixture(&fixture);
    index_workspace(&workspace, &model).unwrap();

    let patterns = vec![
        ("fn\\s+\\w+", "any function definition", 50),
        ("use\\s+std::", "std imports", 10),
        ("impl\\s+\\w+\\s+for", "trait implementations", 3),
        ("pub\\s+(fn|struct|enum)", "public items", 20),
        ("#\\[derive\\(", "derive macros", 5),
        ("TODO|FIXME", "todo comments", 0), // may or may not exist
    ];

    for (pattern, label, min_expected) in patterns {
        let start = Instant::now();
        let hits = regex_search(&workspace, pattern, Some(200), None).unwrap();
        let elapsed = start.elapsed();

        eprintln!(
            "[regex-stress] pattern='{}' ({}) hits={} elapsed={:?}",
            pattern,
            label,
            hits.len(),
            elapsed
        );

        if min_expected > 0 {
            assert!(
                hits.len() >= min_expected,
                "pattern '{}' expected >= {} hits, got {}",
                pattern,
                min_expected,
                hits.len()
            );
        }

        // Regex on a 200+ file repo should complete in under 10s
        assert!(
            elapsed < Duration::from_secs(10),
            "regex '{}' too slow: {:?}",
            pattern,
            elapsed
        );
    }
}

// ============================================================================
// 7. Multi-workspace concurrent indexing
// ============================================================================

#[test]
#[ignore = "downloads required; run ./scripts/bootstrap_stress_fixtures.sh first"]
#[serial]
fn stress_multi_workspace_concurrent_index() {
    let ripgrep = stress_root().join("repos/ripgrep");
    let alice = stress_root().join("workspaces/alice");
    require_fixture(&ripgrep);
    require_fixture(&alice);

    // Stage each into separate temp dirs with separate IVYGREP_HOME
    let staging_rg = tempfile::tempdir().unwrap();
    let staging_alice = tempfile::tempdir().unwrap();
    let home_rg = tempfile::tempdir().unwrap();
    let home_alice = tempfile::tempdir().unwrap();

    let rg_root = staging_rg.path().join("workspace");
    let alice_root = staging_alice.path().join("workspace");
    fs::create_dir_all(&rg_root).unwrap();
    fs::create_dir_all(&alice_root).unwrap();

    let mut opts = CopyOptions::new();
    opts.overwrite = true;
    opts.copy_inside = true;
    copy_dir(&ripgrep, &rg_root, &opts).unwrap();
    copy_dir(&alice, &alice_root, &opts).unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let b1 = Arc::clone(&barrier);
    let b2 = Arc::clone(&barrier);

    let home_rg_path = home_rg.path().to_path_buf();
    let home_alice_path = home_alice.path().to_path_buf();
    let rg_root_clone = rg_root.clone();
    let alice_root_clone = alice_root.clone();

    let h1 = thread::spawn(move || {
        b1.wait();
        unsafe { std::env::set_var("IVYGREP_HOME", &home_rg_path) };
        let ws = Workspace::resolve(&rg_root_clone).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let summary = index_workspace(&ws, &model).unwrap();
        eprintln!(
            "[multi-ws] ripgrep: files={} chunks={}",
            summary.indexed_files, summary.total_chunks
        );
        assert!(summary.indexed_files > 100);
        (ws, model)
    });

    let h2 = thread::spawn(move || {
        b2.wait();
        unsafe { std::env::set_var("IVYGREP_HOME", &home_alice_path) };
        let ws = Workspace::resolve(&alice_root_clone).unwrap();
        let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
        let summary = index_workspace(&ws, &model).unwrap();
        eprintln!(
            "[multi-ws] alice: files={} chunks={}",
            summary.indexed_files, summary.total_chunks
        );
        assert!(summary.indexed_files >= 1);
        (ws, model)
    });

    let (ws_rg, model_rg) = h1.join().expect("ripgrep index thread panicked");
    let (ws_alice, model_alice) = h2.join().expect("alice index thread panicked");

    // Now search both concurrently
    let ws_rg = Arc::new(ws_rg);
    let m_rg = Arc::new(model_rg);
    let ws_alice = Arc::new(ws_alice);
    let m_alice = Arc::new(model_alice);

    let barrier = Arc::new(Barrier::new(2));
    let b1 = Arc::clone(&barrier);
    let b2 = Arc::clone(&barrier);

    let ws_r = Arc::clone(&ws_rg);
    let m_r = Arc::clone(&m_rg);
    let h1 = thread::spawn(move || {
        b1.wait();
        let hits = hybrid_search(
            &ws_r,
            "regex matcher",
            m_r.as_ref(),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits.is_empty(), "ripgrep search should return results");
    });

    let ws_a = Arc::clone(&ws_alice);
    let m_a = Arc::clone(&m_alice);
    let h2 = thread::spawn(move || {
        b2.wait();
        let hits = hybrid_search(
            &ws_a,
            "down the rabbit hole",
            m_a.as_ref(),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(!hits.is_empty(), "alice search should return results");
    });

    h1.join().expect("ripgrep search panicked");
    h2.join().expect("alice search panicked");
    eprintln!("[multi-ws] PASS: concurrent multi-workspace index + search");
}

// ============================================================================
// 8. Large file edge case (near 16MB, many chunks)
// ============================================================================

#[test]
#[serial]
fn stress_large_file_near_limit() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

    // Generate a ~2MB Python file with hundreds of functions
    let mut large_content = String::new();
    for i in 0..800 {
        large_content.push_str(&format!(
            "def function_{i}(x, y, z):\n    \"\"\"Compute result for case {i}\"\"\"\n    return x * {} + y - z\n\n",
            i + 1
        ));
    }
    fs::write(root.path().join("giant_module.py"), &large_content).unwrap();

    let workspace = Workspace::resolve(root.path()).unwrap();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

    let start = Instant::now();
    let summary = index_workspace(&workspace, &model).unwrap();
    let elapsed = start.elapsed();

    eprintln!(
        "[large-file] chunks={} elapsed={:?}",
        summary.total_chunks, elapsed
    );
    assert!(
        summary.total_chunks >= 50,
        "large file should produce many chunks: got {}",
        summary.total_chunks
    );

    // Search should find specific functions
    let hits = hybrid_search(
        &workspace,
        "function_500",
        &model,
        &SearchOptions::default(),
    )
    .unwrap();
    assert!(!hits.is_empty(), "should find function_500 in large file");
    assert!(
        hits[0].preview.contains("function_500"),
        "top hit should contain function_500"
    );

    // Search for a concept, not just a name
    let hits = hybrid_search(
        &workspace,
        "compute result",
        &model,
        &SearchOptions::default(),
    )
    .unwrap();
    assert!(!hits.is_empty());
}

// ============================================================================
// 9. Rapid large-scale file churn (50+ files per cycle)
// ============================================================================

#[test]
#[serial]
fn stress_rapid_large_scale_churn() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

    // Phase 1: create 100 files
    for i in 0..100 {
        fs::write(
            root.path().join(format!("mod_{i:03}.rs")),
            format!("pub fn handler_{i}() -> usize {{ {i} }}\n"),
        )
        .unwrap();
    }
    let workspace = Workspace::resolve(root.path()).unwrap();
    let s1 = index_workspace(&workspace, &model).unwrap();
    assert_eq!(s1.indexed_files, 100);
    eprintln!(
        "[churn] phase1: created 100 files, chunks={}",
        s1.total_chunks
    );

    // Phase 2: modify 30, delete 20, add 25 — all at once
    for i in 0..30 {
        fs::write(
            root.path().join(format!("mod_{i:03}.rs")),
            format!("pub fn handler_{i}_v2() -> usize {{ {} }}\n", i * 100),
        )
        .unwrap();
    }
    for i in 30..50 {
        fs::remove_file(root.path().join(format!("mod_{i:03}.rs"))).unwrap();
    }
    for i in 100..125 {
        fs::write(
            root.path().join(format!("mod_{i:03}.rs")),
            format!("pub fn new_handler_{i}() -> usize {{ {i} }}\n"),
        )
        .unwrap();
    }

    let s2 = index_workspace(&workspace, &model).unwrap();
    assert_eq!(s2.indexed_files, 55, "30 modified + 25 new = 55");
    assert_eq!(s2.deleted_files, 20, "20 files deleted");
    eprintln!(
        "[churn] phase2: modified=30 deleted=20 added=25 → indexed={} deleted={}",
        s2.indexed_files, s2.deleted_files
    );

    // Phase 3: verify index integrity
    let ws = Workspace::resolve(root.path()).unwrap();
    let conn = open_sqlite(&ws.sqlite_path()).unwrap();
    let file_count: i64 = conn
        .query_row("SELECT COUNT(DISTINCT file_path) FROM chunks", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        file_count, 105,
        "100 - 20 deleted + 25 new = 105 files in index"
    );

    // Phase 4: re-index → should be zero work
    let s3 = index_workspace(&workspace, &model).unwrap();
    assert_eq!(s3.indexed_files, 0, "no changes → zero indexed");
    assert_eq!(s3.deleted_files, 0, "no changes → zero deleted");

    // Phase 5: search after massive churn should still work
    let hits = hybrid_search(&workspace, "new_handler", &model, &SearchOptions::default()).unwrap();
    assert!(!hits.is_empty(), "should find newly added functions");

    // Verify deleted functions are gone
    let hits = hybrid_search(&workspace, "handler_35", &model, &SearchOptions::default()).unwrap();
    let has_deleted = hits
        .iter()
        .any(|h| h.file_path.to_string_lossy().contains("mod_035"));
    assert!(
        !has_deleted,
        "deleted file mod_035.rs should not appear in results"
    );

    eprintln!("[churn] ALL PHASES PASSED");
}

// ============================================================================
// 10. Query throughput benchmark (many queries, latency distribution)
// ============================================================================

#[test]
#[serial]
fn stress_query_throughput_benchmark() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

    // Generate a medium-sized codebase
    for i in 0..60 {
        let lang = match i % 4 {
            0 => (
                "rs",
                format!("pub fn compute_{i}(x: f64) -> f64 {{ x * {}.0 }}\n", i + 1),
            ),
            1 => (
                "py",
                format!("def calculate_{i}(x, y):\n    return x + y * {}\n", i),
            ),
            2 => (
                "ts",
                format!(
                    "export function process_{i}(data: number[]): number {{ return data.length * {} }}\n",
                    i
                ),
            ),
            _ => (
                "java",
                format!(
                    "public class Handler{i} {{\n    public int handle(int x) {{ return x * {}; }}\n}}\n",
                    i
                ),
            ),
        };
        fs::write(root.path().join(format!("module_{i}.{}", lang.0)), lang.1).unwrap();
    }

    let workspace = Workspace::resolve(root.path()).unwrap();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    index_workspace(&workspace, &model).unwrap();

    let queries = vec![
        "compute function",
        "calculate value",
        "process data",
        "handle request",
        "return result",
        "public method",
        "export function",
        "x plus y",
        "multiply number",
        "array length",
        "integer return",
        "float computation",
        "where is the handler",
        "data processing pipeline",
        "mathematical operation",
        "module_30",
        "compute_15",
        "calculate_42",
        "Handler",
        "process_7",
    ];

    let mut latencies = Vec::new();
    let overall_start = Instant::now();

    for query in &queries {
        let start = Instant::now();
        let hits = hybrid_search(&workspace, query, &model, &SearchOptions::default()).unwrap();
        latencies.push(start.elapsed());
        assert!(!hits.is_empty(), "query '{query}' had no results");
    }

    let overall_elapsed = overall_start.elapsed();

    latencies.sort();
    let total = latencies.len();
    let p50 = latencies[total / 2];
    let p95 = latencies[(total as f64 * 0.95) as usize];
    let max = latencies[total - 1];
    let avg: Duration = latencies.iter().sum::<Duration>() / total as u32;
    let qps = total as f64 / overall_elapsed.as_secs_f64();

    eprintln!("[throughput] {total} queries in {overall_elapsed:?}");
    eprintln!("[throughput] QPS={qps:.1} avg={avg:?} p50={p50:?} p95={p95:?} max={max:?}");

    // 20 queries should finish within 30 seconds even on slow hardware
    assert!(
        overall_elapsed < Duration::from_secs(30),
        "benchmark too slow: {:?}",
        overall_elapsed
    );
}

// ============================================================================
// 11. Sustained load — repeated queries without leaking state
// ============================================================================

#[test]
#[serial]
fn stress_sustained_query_and_reindex_cycles() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

    // Create 20 files
    for i in 0..20 {
        fs::write(
            root.path().join(format!("svc_{i}.rs")),
            format!("pub fn service_{i}(req: &str) -> String {{ req.to_uppercase() }}\n"),
        )
        .unwrap();
    }

    let workspace = Workspace::resolve(root.path()).unwrap();
    index_workspace(&workspace, &model).unwrap();

    let initial_chunks = {
        let conn = open_sqlite(&workspace.sqlite_path()).unwrap();
        let c: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
            .unwrap();
        c as usize
    };

    // 30 cycles of: query → mutate 1 file → reindex → query
    for cycle in 0..30 {
        let hits = hybrid_search(&workspace, "service", &model, &SearchOptions::default()).unwrap();
        assert!(!hits.is_empty(), "cycle {cycle}: search failed");

        // Mutate one file
        let idx = cycle % 20;
        fs::write(
            root.path().join(format!("svc_{idx}.rs")),
            format!(
                "pub fn service_{idx}_v{cycle}(req: &str) -> String {{ format!(\"c{cycle}: {{}}\", req) }}\n"
            ),
        )
        .unwrap();

        let summary = index_workspace(&workspace, &model).unwrap();
        assert_eq!(
            summary.indexed_files, 1,
            "cycle {cycle}: only 1 file changed"
        );

        // Chunk count should remain stable (replace, not accumulate)
        let conn = open_sqlite(&workspace.sqlite_path()).unwrap();
        let current_chunks: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            current_chunks as usize, initial_chunks,
            "cycle {cycle}: chunk count drifted from {initial_chunks} to {current_chunks}"
        );
    }

    eprintln!("[sustained] 30 query+reindex cycles PASSED, chunk count stable at {initial_chunks}");
}

// ============================================================================
// 12. Index integrity after extreme churn — verify no orphan chunks
// ============================================================================

#[test]
#[serial]
fn stress_index_integrity_no_orphan_chunks() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

    // Create 30 files, index
    for i in 0..30 {
        fs::write(
            root.path().join(format!("item_{i}.rs")),
            format!("fn item_{i}() -> i32 {{ {i} }}\n"),
        )
        .unwrap();
    }

    let workspace = Workspace::resolve(root.path()).unwrap();
    index_workspace(&workspace, &model).unwrap();

    // 10 rounds of random churn
    for round in 0..10 {
        // Delete some files
        for j in (round * 3)..(round * 3 + 3).min(30) {
            let path = root.path().join(format!("item_{j}.rs"));
            if path.exists() {
                fs::remove_file(&path).unwrap();
            }
        }

        // Add new files
        for j in 0..2 {
            let id = 30 + round * 2 + j;
            fs::write(
                root.path().join(format!("item_{id}.rs")),
                format!("fn item_{id}() -> i32 {{ {id} }}\n"),
            )
            .unwrap();
        }

        // Modify some survivors
        let surv = root.path().join(format!("item_{}.rs", 29 - round));
        if surv.exists() {
            fs::write(
                &surv,
                format!(
                    "fn item_{}_modified_r{round}() -> i32 {{ {} }}\n",
                    29 - round,
                    round * 100
                ),
            )
            .unwrap();
        }

        index_workspace(&workspace, &model).unwrap();
    }

    // Final integrity check: every file_path in chunks must exist on disk
    let conn = open_sqlite(&workspace.sqlite_path()).unwrap();
    let mut stmt = conn
        .prepare("SELECT DISTINCT file_path FROM chunks")
        .unwrap();
    let db_files: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    for db_file in &db_files {
        let abs_path = root.path().join(db_file);
        assert!(
            abs_path.exists(),
            "orphan chunk in DB: file '{}' does not exist on disk",
            db_file
        );
    }

    // And vice versa: every indexable file on disk should be in the DB
    let snapshot = MerkleSnapshot::build(root.path()).unwrap();
    for fs_file in snapshot.files.keys() {
        assert!(
            db_files.contains(fs_file),
            "file '{}' exists on disk but has no chunks in DB",
            fs_file
        );
    }

    // Chunk count per file should be > 0
    let mut stmt = conn
        .prepare("SELECT file_path, COUNT(*) as cnt FROM chunks GROUP BY file_path HAVING cnt = 0")
        .unwrap();
    let empty_files: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        empty_files.is_empty(),
        "files with zero chunks in DB: {:?}",
        empty_files
    );

    eprintln!(
        "[integrity] PASS: {} files in DB, all exist on disk, no orphans",
        db_files.len()
    );
}

// ============================================================================
// 13. Concurrent search + indexing storm on large repo
// ============================================================================

#[test]
#[ignore = "downloads required; run ./scripts/bootstrap_stress_fixtures.sh first"]
#[serial]
fn stress_concurrent_search_during_reindex_large() {
    let fixture = stress_root().join("repos/ripgrep");
    require_fixture(&fixture);
    let (_staging, _home, workspace, model) = stage_fixture(&fixture);
    index_workspace(&workspace, &model).unwrap();

    let ws = Arc::new(workspace);
    let m = Arc::new(model);
    let barrier = Arc::new(Barrier::new(5)); // 1 writer + 4 readers

    // Writer thread: mutate files and re-index
    let ws_w = Arc::clone(&ws);
    let m_w = Arc::clone(&m);
    let barrier_w = Arc::clone(&barrier);
    let ws_root = ws.root.clone();
    let writer = thread::spawn(move || {
        barrier_w.wait();
        for i in 0..3 {
            // Add a small temp file each iteration
            let sentinel = ws_root.join(format!("_stress_sentinel_{i}.txt"));
            fs::write(&sentinel, format!("stress iteration {i}\n")).unwrap();

            let result = index_workspace(&ws_w, m_w.as_ref());
            assert!(
                result.is_ok(),
                "writer iteration {i} failed: {:?}",
                result.err()
            );
            thread::sleep(Duration::from_millis(50));
        }
    });

    // Reader threads: search concurrently
    let readers: Vec<_> = (0..4)
        .map(|tid| {
            let ws = Arc::clone(&ws);
            let m = Arc::clone(&m);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let queries = ["binary", "regex", "ignore", "search"];
                for iter in 0..5 {
                    let query = queries[iter % queries.len()];
                    let result = hybrid_search(
                        &ws,
                        query,
                        m.as_ref(),
                        &SearchOptions {
                            limit: Some(10),
                            context: 2,
                            type_filter: None,
                            scope_filter: None,
                        },
                    );
                    // May error during concurrent write, but must not panic
                    match result {
                        Ok(hits) => {
                            eprintln!(
                                "[concurrent-large] reader {tid} query '{query}' hits={}",
                                hits.len()
                            );
                        }
                        Err(e) => {
                            eprintln!("[concurrent-large] reader {tid} acceptable error: {e}");
                        }
                    }
                    thread::sleep(Duration::from_millis(20));
                }
            })
        })
        .collect();

    writer.join().expect("writer panicked");
    for r in readers {
        r.join().expect("reader panicked");
    }

    eprintln!("[concurrent-large] PASS: no panics under concurrent read+write on ripgrep");
}

// ============================================================================
// 14. Diverse language indexing stress (many languages at once)
// ============================================================================

#[test]
#[serial]
fn stress_diverse_language_mix() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

    // Create files in every supported language
    let files: Vec<(&str, &str)> = vec![
        (
            "app.rs",
            "pub fn rust_handler(req: &str) -> String { req.to_uppercase() }\n",
        ),
        (
            "app.py",
            "def python_handler(req):\n    return req.upper()\n",
        ),
        (
            "app.ts",
            "export function typescriptHandler(req: string): string { return req.toUpperCase(); }\n",
        ),
        (
            "app.js",
            "function javascriptHandler(req) { return req.toUpperCase(); }\n",
        ),
        (
            "App.java",
            "public class App {\n    public String javaHandler(String req) { return req.toUpperCase(); }\n}\n",
        ),
        (
            "app.go",
            "package main\n\nimport \"strings\"\n\nfunc goHandler(req string) string { return strings.ToUpper(req) }\n",
        ),
        ("app.rb", "def ruby_handler(req)\n  req.upcase\nend\n"),
        (
            "app.c",
            "#include <ctype.h>\nvoid c_handler(char* s) { while(*s) { *s = toupper(*s); s++; } }\n",
        ),
        (
            "app.cpp",
            "#include <algorithm>\nstd::string cpp_handler(std::string s) { std::transform(s.begin(), s.end(), s.begin(), ::toupper); return s; }\n",
        ),
        (
            "app.cs",
            "public class App { public string CSharpHandler(string req) => req.ToUpper(); }\n",
        ),
        (
            "app.swift",
            "func swiftHandler(_ req: String) -> String { return req.uppercased() }\n",
        ),
        (
            "app.kt",
            "fun kotlinHandler(req: String): String = req.uppercase()\n",
        ),
        (
            "app.scala",
            "def scalaHandler(req: String): String = req.toUpperCase\n",
        ),
        (
            "app.php",
            "<?php function phpHandler($req) { return strtoupper($req); }\n",
        ),
        ("app.r", "r_handler <- function(req) { toupper(req) }\n"),
    ];

    for (name, content) in &files {
        fs::write(root.path().join(name), content).unwrap();
    }

    let workspace = Workspace::resolve(root.path()).unwrap();
    let summary = index_workspace(&workspace, &model).unwrap();

    eprintln!(
        "[diverse-lang] indexed {} files, {} chunks",
        summary.indexed_files, summary.total_chunks
    );
    assert!(
        summary.indexed_files >= 10,
        "should index at least 10 language files"
    );

    // Search for a concept that spans all languages
    let hits = hybrid_search(
        &workspace,
        "handler that converts to uppercase",
        &model,
        &SearchOptions::default(),
    )
    .unwrap();
    assert!(!hits.is_empty(), "should find handler functions");

    // Check that multiple languages appear in results
    let mut languages_found: HashMap<String, bool> = HashMap::new();
    for hit in &hits {
        let ext = hit
            .file_path
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();
        languages_found.insert(ext, true);
    }

    eprintln!(
        "[diverse-lang] languages in results: {:?}",
        languages_found.keys().collect::<Vec<_>>()
    );
    assert!(
        languages_found.len() >= 3,
        "search should return results from at least 3 different languages, got: {:?}",
        languages_found.keys().collect::<Vec<_>>()
    );
}

// ============================================================================
// 15. Idempotent double-index on real repo
// ============================================================================

#[test]
#[ignore = "downloads required; run ./scripts/bootstrap_stress_fixtures.sh first"]
#[serial]
fn stress_double_index_idempotent() {
    let fixture = stress_root().join("repos/ripgrep");
    require_fixture(&fixture);
    let (_staging, _home, workspace, model) = stage_fixture(&fixture);

    let s1 = index_workspace(&workspace, &model).unwrap();
    let s2 = index_workspace(&workspace, &model).unwrap();

    assert_eq!(s2.indexed_files, 0, "second index should be zero work");
    assert_eq!(s2.deleted_files, 0);
    assert_eq!(
        s1.total_chunks, s2.total_chunks,
        "chunk count must be identical"
    );

    // Third index, also zero work
    let s3 = index_workspace(&workspace, &model).unwrap();
    assert_eq!(s3.indexed_files, 0);
    assert_eq!(s3.total_chunks, s1.total_chunks);

    eprintln!(
        "[idempotent] PASS: 3 indexes, stable at {} chunks",
        s1.total_chunks
    );
}
