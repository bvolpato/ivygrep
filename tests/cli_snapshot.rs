use std::path::Path;

use assert_cmd::Command;
use fs_extra::dir::{CopyOptions, copy as copy_dir};
use ivygrep::embedding::create_hash_model;
use ivygrep::indexer::index_workspace;
use ivygrep::workspace::{Workspace, WorkspaceMetadata};
use serial_test::serial;

fn stage_fixture_repo(name: &str) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let fixture_root = Path::new("tests/fixtures").join(name);
    let target_root = tmp.path().join("workspace");

    std::fs::create_dir_all(&target_root).unwrap();
    let mut opts = CopyOptions::new();
    opts.overwrite = true;
    opts.copy_inside = true;
    copy_dir(&fixture_root, &target_root, &opts).unwrap();

    let home = tmp.path().join("ivygrep_home");
    (tmp, target_root, home)
}

fn create_unhealthy_index_fixture(root: &Path, home: &Path, skip_gitignore: bool) -> Workspace {
    unsafe { std::env::set_var("IVYGREP_HOME", home) };

    let workspace = Workspace::resolve(root).unwrap();
    workspace.ensure_dirs().unwrap();
    workspace
        .write_metadata(&WorkspaceMetadata {
            id: workspace.id.clone(),
            root: workspace.root.clone(),
            created_at_unix: 0,
            last_indexed_at_unix: Some(1),
            watch_enabled: false,
            skip_gitignore,
            index_generation: 0,
        })
        .unwrap();
    std::fs::write(workspace.sqlite_path(), "").unwrap();
    std::fs::create_dir_all(workspace.tantivy_dir()).unwrap();
    std::fs::write(workspace.vector_path(), "").unwrap();
    workspace
}

#[test]
#[serial]
fn cli_help_snapshot() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();

    insta::assert_snapshot!("cli_help", text);
}

#[test]
#[serial]
fn cli_interactive_long_flags_are_accepted() {
    for flag in ["--interactive", "--ui"] {
        let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
        cmd.arg(flag).arg("--version").assert().success();
    }
}

#[test]
#[serial]
fn cli_short_i_is_not_interactive_alias() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    cmd.arg("-i").arg("--version").assert().failure();
}

#[test]
#[serial]
fn cli_query_json_snapshot() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "-f", "where is the tax calculated?"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let mut value: serde_json::Value = serde_json::from_slice(&output).unwrap();

    if let Some(array) = value.as_array_mut() {
        for file in array.iter_mut() {
            if let Some(total_score) = file.get_mut("total_score") {
                *total_score = serde_json::json!("<score>");
            }

            if let Some(hits) = file.get_mut("hits").and_then(|hits| hits.as_array_mut()) {
                for hit in hits {
                    if let Some(score) = hit.get_mut("score") {
                        *score = serde_json::json!("<score>");
                    }
                }
            }
        }
    }

    insta::assert_yaml_snapshot!("cli_query_json", value);
}

#[test]
#[serial]
fn cli_file_name_only_json_snapshot() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--json",
            "--hash",
            "--file-name-only",
            "-f",
            "where is the tax calculated?",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    insta::assert_yaml_snapshot!("cli_file_name_only_json", value);
}

#[test]
#[serial]
fn cli_first_line_only_text_output() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--first-line-only",
            "--hash",
            "-f",
            "where is the tax calculated?",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("pub fn calculate_tax"));
    assert!(!text.contains("amount * rate"));
}

#[test]
#[serial]
fn cli_query_with_explicit_path_json() {
    let (tmp, target_root, home) = stage_fixture_repo("rust_repo");
    let target_root_str = target_root.to_string_lossy().into_owned();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(tmp.path())
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--json",
            "--hash",
            "-f",
            "where is the tax calculated?",
            &target_root_str,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files = value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry.get("file_path").and_then(|v| v.as_str()))
        .collect::<Vec<_>>();

    assert!(!files.is_empty());
    assert!(
        files
            .iter()
            .any(|path| path.ends_with("rust_repo/src/lib.rs"))
    );
}

#[test]
#[serial]
fn cli_query_word_add_is_treated_as_query() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "-f", "add"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let _value: serde_json::Value = serde_json::from_slice(&output).unwrap();
}

#[test]
#[serial]
fn cli_add_flag_indexes_workspace() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--add", "--hash", "."])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Indexed") || text.contains("indexed"));
}

#[test]
#[serial]
fn cli_verbose_json_includes_reason() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--json",
            "--hash",
            "--verbose",
            "-f",
            "where is the tax calculated?",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let mut has_reason = false;
    if let Some(files) = value.as_array() {
        for file in files {
            if let Some(hits) = file.get("hits").and_then(|hits| hits.as_array()) {
                for hit in hits {
                    if hit
                        .get("reason")
                        .and_then(|reason| reason.as_str())
                        .is_some_and(|reason| !reason.trim().is_empty())
                    {
                        has_reason = true;
                    }
                }
            }
        }
    }

    assert!(has_reason);
}

#[test]
#[serial]
fn cli_query_from_subdirectory_is_scope_restricted() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    let scoped = root.join("scoped");
    let other = root.join("other");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir_all(&scoped).unwrap();
    std::fs::create_dir_all(&other).unwrap();

    std::fs::write(
        scoped.join("match.rs"),
        "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
    )
    .unwrap();
    std::fs::write(
        other.join("match.rs"),
        "pub fn applyFilter(values: &[i32]) -> Vec<i32> { values.to_vec() }\n",
    )
    .unwrap();

    let home = tmp.path().join("ivygrep_home");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&scoped)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "-f", "applyFilter"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files = value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry.get("file_path").and_then(|v| v.as_str()))
        .collect::<Vec<_>>();

    assert!(!files.is_empty());
    assert!(files.iter().all(|path| path.starts_with("scoped/")));
}

#[test]
#[serial]
fn cli_scoped_literal_search_survives_high_scoring_parent_matches() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    let scoped = root.join("scoped");
    let other = root.join("other");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir_all(&scoped).unwrap();
    std::fs::create_dir_all(&other).unwrap();

    for i in 0..700 {
        std::fs::write(
            other.join(format!("targettoken_noise_{i:03}.rs")),
            format!(
                "pub fn noisy_{i}() {{\n    // {}\n}}\n",
                "targettoken ".repeat(80)
            ),
        )
        .unwrap();
    }

    std::fs::write(
        scoped.join("match.rs"),
        "pub fn scoped_match() -> &'static str { \"targettoken\" }\n",
    )
    .unwrap();

    let home = tmp.path().join("ivygrep_home");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&scoped)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--json",
            "--hash",
            "--literal",
            "-f",
            "-n",
            "1",
            "targettoken",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files = value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry.get("file_path").and_then(|v| v.as_str()))
        .collect::<Vec<_>>();

    assert_eq!(
        files,
        vec!["scoped/match.rs"],
        "literal search from a subdirectory should not lose scoped hits behind high-scoring parent matches"
    );
}

#[test]
#[serial]
fn cli_prevent_nested_indexing() {
    let tmp = tempfile::tempdir().unwrap();
    let parent = tmp.path().join("parent");
    let child = parent.join("child");
    std::fs::create_dir_all(&child).unwrap();

    let home = tmp.path().join("ivygrep_home");

    // Index the child repository
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    cmd.current_dir(&child)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--add", "."])
        .assert()
        .success();

    // Try to index the parent repository (should fail)
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&parent)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--add", "."])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("because it contains already indexed sub-workspaces"));
    assert!(text.contains("You must remove them first"));
    assert!(text.contains(&format!(
        "ig --rm {}",
        child.canonicalize().unwrap().display()
    )));
}

/// Regression: `ig --literal gquota` must find the term inside a top-level
/// `const` declaration in TypeScript, not just inside functions/classes.
#[test]
#[serial]
fn cli_literal_finds_top_level_string_constant() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();

    std::fs::write(
        root.join("plugin.ts"),
        r#"import { Plugin } from "sdk";

const GEMINI_QUOTA_COMMAND = "gquota";

export function registerCommands(p: Plugin) {
    p.registerCommand(GEMINI_QUOTA_COMMAND, () => {
        console.log("checking quota...");
    });
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("README.md"),
        "# Plugin\n\nRun `/gquota` to check your quota.\n",
    )
    .unwrap();

    let home = tmp.path().join("ivygrep_home");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "--literal", "-f", "gquota"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files: Vec<&str> = value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry.get("file_path").and_then(|v| v.as_str()))
        .collect();

    assert!(
        files.iter().any(|p| p.contains("plugin.ts")),
        "literal search must find gquota in plugin.ts, got files: {:?}",
        files
    );
    assert!(
        files.iter().any(|p| p.contains("README.md")),
        "literal search must find gquota in README.md, got files: {:?}",
        files
    );
}

/// Regression: hybrid (default) mode must also surface top-level constants.
#[test]
#[serial]
fn cli_hybrid_finds_top_level_string_constant() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();

    std::fs::write(
        root.join("plugin.ts"),
        r#"import { Plugin } from "sdk";

const GEMINI_QUOTA_COMMAND = "gquota";

export function registerCommands(p: Plugin) {
    p.registerCommand(GEMINI_QUOTA_COMMAND, () => {
        console.log("checking quota...");
    });
}
"#,
    )
    .unwrap();

    let home = tmp.path().join("ivygrep_home");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "-f", "gquota"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files: Vec<&str> = value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry.get("file_path").and_then(|v| v.as_str()))
        .collect();

    assert!(
        files.iter().any(|p| p.contains("plugin.ts")),
        "hybrid search must find gquota in plugin.ts, got files: {:?}",
        files
    );
}

#[test]
#[serial]
fn cli_doctor_json_reports_unhealthy_zero_chunk_index() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("lib.rs"), "pub fn answer() -> usize { 42 }\n").unwrap();

    let home = tmp.path().join("ivygrep_home");
    let _workspace = create_unhealthy_index_fixture(&root, &home, false);

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .args(["--doctor", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["state"], "unhealthy");
    assert_eq!(value["healthy"], false);
    assert_eq!(value["chunk_count"], 0);
    assert!(
        value["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|finding| finding.as_str())
            .any(|finding| finding.contains("zero chunks")),
        "doctor findings should mention the zero-chunk failure mode: {value:#}"
    );
}

#[test]
#[serial]
fn cli_doctor_fix_repairs_unhealthy_index() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("lib.rs"), "pub fn answer() -> usize { 42 }\n").unwrap();

    let home = tmp.path().join("ivygrep_home");
    let _workspace = create_unhealthy_index_fixture(&root, &home, false);

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .args(["--doctor", "--fix", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["state"], "healthy");
    assert_eq!(value["healthy"], true);
    assert_eq!(value["repaired"], true);
    assert!(
        value["chunk_count"].as_u64().unwrap_or_default() >= 1,
        "doctor --fix should rebuild the index: {value:#}"
    );
}

#[test]
#[serial]
fn cli_query_auto_repairs_unhealthy_index() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("lib.rs"), "pub fn answer() -> usize { 42 }\n").unwrap();

    let home = tmp.path().join("ivygrep_home");
    let _workspace = create_unhealthy_index_fixture(&root, &home, false);

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = cmd
        .current_dir(&root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "-f", "answer"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let files = value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry.get("file_path").and_then(|v| v.as_str()))
        .collect::<Vec<_>>();

    assert!(
        files.iter().any(|path| path.ends_with("lib.rs")),
        "search should recover from an unhealthy index and return lib.rs: {:?}",
        files
    );
}

#[test]
#[serial]
fn cli_query_cleans_stale_legacy_watcher_pid() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");
    unsafe { std::env::set_var("IVYGREP_HOME", &home) };

    let workspace = Workspace::resolve(&target_root).unwrap();
    let model = create_hash_model();
    let _ = index_workspace(&workspace, model.as_ref()).unwrap();
    std::fs::write(workspace.watcher_pid_path(), "999999").unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    cmd.current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--json", "--hash", "-f", "where is the tax calculated?"])
        .assert()
        .success();

    assert!(
        !workspace.watcher_pid_path().exists(),
        "query should remove stale legacy watcher pid files"
    );
}
