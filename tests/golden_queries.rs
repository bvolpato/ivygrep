use std::path::Path;

use fs_extra::dir::{CopyOptions, copy as copy_dir};
use ivygrep::EMBEDDING_DIMENSIONS;
use ivygrep::embedding::HashEmbeddingModel;
use ivygrep::indexer::index_workspace;
use ivygrep::search::{SearchOptions, hybrid_search};
use ivygrep::workspace::Workspace;
use serial_test::serial;
use tempfile::TempDir;

#[derive(Debug, serde::Serialize)]
struct SnapshotHit {
    file_path: String,
    line: usize,
    preview: String,
    sources: Vec<String>,
}

#[test]
#[serial]
fn golden_query_rust_tax() {
    let (_tmp, workspace) = stage_fixture("rust_repo");
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

    index_workspace(&workspace, &model).unwrap();
    let hits = hybrid_search(
        &workspace,
        "where is the tax calculated?",
        &model,
        &SearchOptions::default(),
    )
    .unwrap();

    assert!(!hits.is_empty());
    assert!(hits.iter().any(|hit| hit.preview.contains("calculate_tax")));

    let snap = hits
        .into_iter()
        .take(3)
        .map(|hit| SnapshotHit {
            file_path: hit.file_path.to_string_lossy().to_string(),
            line: hit.start_line,
            preview: hit.preview,
            sources: hit.sources,
        })
        .collect::<Vec<_>>();

    insta::assert_yaml_snapshot!("golden_rust_tax", snap);
}

#[test]
#[serial]
fn golden_query_python_tax() {
    let (_tmp, workspace) = stage_fixture("python_repo");
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

    index_workspace(&workspace, &model).unwrap();
    let hits = hybrid_search(
        &workspace,
        "tax function",
        &model,
        &SearchOptions::default(),
    )
    .unwrap();

    assert!(!hits.is_empty());
    assert!(hits.iter().any(|hit| hit.preview.contains("calculate_tax")));
}

#[test]
#[serial]
fn golden_query_typescript_total() {
    let (_tmp, workspace) = stage_fixture("ts_repo");
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);

    index_workspace(&workspace, &model).unwrap();
    let hits = hybrid_search(
        &workspace,
        "calculate total with tax",
        &model,
        &SearchOptions::default(),
    )
    .unwrap();

    assert!(!hits.is_empty());
    assert!(
        hits.iter()
            .any(|hit| hit.preview.contains("calculateTotal")
                || hit.preview.contains("calculateTax"))
    );
}

fn stage_fixture(name: &str) -> (TempDir, Workspace) {
    let tmp = tempfile::tempdir().unwrap();
    let fixture_root = Path::new("tests/fixtures").join(name);
    let target_root = tmp.path().join(name);

    std::fs::create_dir_all(&target_root).unwrap();

    let mut opts = CopyOptions::new();
    opts.overwrite = true;
    opts.copy_inside = true;

    copy_dir(&fixture_root, &target_root, &opts).unwrap();

    let ivygrep_home = tmp.path().join("ivygrep_home");
    unsafe { std::env::set_var("IVYGREP_HOME", &ivygrep_home) };

    let workspace = Workspace::resolve(&target_root).unwrap();
    (tmp, workspace)
}
