use std::path::{Path, PathBuf};
use std::time::Instant;

use fs_extra::dir::{CopyOptions, copy as copy_dir};
use ivygrep::EMBEDDING_DIMENSIONS;
use ivygrep::embedding::HashEmbeddingModel;
use ivygrep::indexer::index_workspace;
use ivygrep::search::{SearchOptions, hybrid_search};
use ivygrep::workspace::Workspace;
use serial_test::serial;

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

fn run_index_and_query(
    fixture_path: &Path,
    query: &str,
    expected_substring: &str,
    min_files_indexed: usize,
) {
    let staging = tempfile::tempdir().unwrap();
    let staged_workspace_root = staging.path().join("workspace");
    std::fs::create_dir_all(&staged_workspace_root).unwrap();

    let mut copy_opts = CopyOptions::new();
    copy_opts.overwrite = true;
    copy_opts.copy_inside = true;
    copy_dir(fixture_path, &staged_workspace_root, &copy_opts).unwrap();

    let home = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

    let workspace = Workspace::resolve(&staged_workspace_root).unwrap();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

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
