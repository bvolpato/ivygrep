use std::path::Path;

use assert_cmd::Command;
use fs_extra::dir::{CopyOptions, copy as copy_dir};
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

#[test]
#[serial]
fn cli_help_snapshot() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ivygrep"));
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
fn cli_query_json_snapshot() {
    let (_tmp, target_root, home) = stage_fixture_repo("rust_repo");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ivygrep"));
    let output = cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .args(["--json", "-f", "where is the tax calculated?"])
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

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ivygrep"));
    let output = cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .args([
            "--json",
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

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ivygrep"));
    let output = cmd
        .current_dir(&target_root)
        .env("IVYGREP_HOME", &home)
        .args(["--first-line-only", "-f", "where is the tax calculated?"])
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

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ivygrep"));
    let output = cmd
        .current_dir(tmp.path())
        .env("IVYGREP_HOME", &home)
        .args([
            "--json",
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
