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
