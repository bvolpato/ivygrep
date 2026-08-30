use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::assert::OutputAssertExt;
use ivygrep::embedding::HashEmbeddingModel;
use ivygrep::indexer::{enhance_workspace_hash, enhance_workspace_neural, index_workspace};
use ivygrep::workspace::Workspace;
use serial_test::serial;

fn command(root: &Path, home: &Path) -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    command
        .current_dir(root)
        .env("IVYGREP_HOME", home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .env("HF_HUB_OFFLINE", "1");
    command
}

#[test]
#[serial]
fn optional_neural_metadata_does_not_trigger_primary_repair_on_cli_miss() {
    let home = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    fs::write(
        root.path().join("source.rs"),
        "pub fn intact_primary() {}\n",
    )
    .unwrap();
    let workspace = Workspace::resolve(root.path()).unwrap();
    let model = HashEmbeddingModel::new(ivygrep::EMBEDDING_DIMENSIONS);
    index_workspace(&workspace, &model).unwrap();
    enhance_workspace_hash(&workspace, &model).unwrap();
    enhance_workspace_neural(&workspace, &model).unwrap();
    fs::write(workspace.neural_model_path(), "{\n").unwrap();
    let primary = [
        workspace.sqlite_path(),
        workspace.tantivy_dir().join("meta.json"),
        workspace.vector_path(),
        workspace.merkle_snapshot_path(),
        workspace.metadata_path(),
    ]
    .map(|path| {
        let bytes = fs::read(&path).unwrap();
        (path, bytes)
    });
    for mode in ["--literal", "--lexical-only"] {
        let output = command(root.path(), home.path())
            .args([mode, "--json", "--", "zzquixoticabsenttokenzz"])
            .arg(root.path())
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output).unwrap(),
            serde_json::json!([])
        );
        for (path, bytes) in &primary {
            assert_eq!(
                fs::read(path).unwrap(),
                *bytes,
                "{mode}: {}",
                path.display()
            );
        }
    }
    assert!(workspace.index_health().is_queryable());
    let report = ivygrep::doctor::inspect_workspace(&workspace);
    assert!(!report.healthy);
    assert!(
        report
            .findings
            .iter()
            .any(|value| value.contains("neural model metadata"))
    );
}
