use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use assert_cmd::assert::OutputAssertExt;
use serde_json::Value;

fn command(root: &Path, home: &Path, mode: &str) -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    command
        .current_dir(root)
        .env("IVYGREP_HOME", home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .env("IVYGREP_RERANKER", mode)
        .env("HF_HUB_OFFLINE", "1")
        .env_remove("IVYGREP_RERANK_LIMIT")
        .env_remove("IVYGREP_RERANKER_CAPTURE")
        .env_remove("RUST_LOG");
    command
}

fn query(root: &Path, home: &Path, mode: &str, context: Option<usize>) -> Value {
    let mut command = command(root, home, mode);
    command.args(["--lexical-only", "--json", "--verbose", "--limit", "10"]);
    if let Some(context) = context {
        command.args(["-C", &context.to_string()]);
    }
    let output = command
        .args(["--", "alpha \"beta gamma\""])
        .arg(root)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn without_display_fields(mut files: Value) -> Value {
    for file in files.as_array_mut().unwrap() {
        for hit in file["hits"].as_array_mut().unwrap() {
            let hit = hit.as_object_mut().unwrap();
            for field in ["start_line", "end_line", "preview"] {
                hit.remove(field);
            }
        }
    }
    files
}

fn stage_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    let home = temp.path().join("home");
    std::fs::create_dir(&root).unwrap();
    let statements = [
        "    alpha = \"first\"\n",
        "    beta = \"second\"\n",
        "    gamma = \"third\"\n",
        "    padding_a = 1\n",
        "    padding_b = 2\n",
        "    padding_c = 3\n",
        "    padding_d = 4\n",
        "    padding_e = 5\n",
    ];
    for (name, order) in [
        ('a', [0, 1, 2, 3, 4, 5, 6, 7]),
        ('b', [0, 3, 4, 5, 1, 6, 7, 2]),
        ('c', [0, 1, 3, 4, 5, 6, 7, 2]),
        ('d', [3, 4, 5, 6, 7, 0, 1, 2]),
        ('e', [0, 3, 4, 5, 6, 1, 2, 7]),
        ('f', [3, 4, 0, 1, 2, 5, 6, 7]),
    ] {
        let mut content = format!("def candidate_{name}():\n");
        for index in order {
            content.push_str(statements[index]);
        }
        content.push_str("    return 0\n");
        std::fs::write(root.join(format!("{name}.py")), content).unwrap();
    }
    command(&root, &home, "learned")
        .arg("--add")
        .arg(&root)
        .args(["--no-watch", "--hash"])
        .assert()
        .success();
    (temp, root, home)
}

#[test]
fn learned_ranking_is_independent_of_display_context() {
    let (_temp, root, home) = stage_fixture();
    let default = query(&root, &home, "learned", None);
    assert_eq!(default, query(&root, &home, "learned", Some(2)));
    let paths = default
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["file_path"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(paths, ["e.py", "d.py", "f.py", "a.py", "b.py"]);

    let deterministic = query(&root, &home, "deterministic", Some(2));
    assert_ne!(
        without_display_fields(default.clone()),
        without_display_fields(deterministic.clone()),
        "the fixture must exercise an active learned rerank"
    );
    for (mode, baseline) in [("learned", default), ("deterministic", deterministic)] {
        for context in [0, 20] {
            let displayed = query(&root, &home, mode, Some(context));
            assert_eq!(
                without_display_fields(displayed.clone()),
                without_display_fields(baseline.clone()),
                "{mode} ranking changed with -C {context}"
            );
            for file in displayed.as_array().unwrap() {
                let path = file["file_path"].as_str().unwrap();
                let content = std::fs::read_to_string(root.join(path)).unwrap();
                let hit = &file["hits"][0];
                if context == 0 {
                    let focus = content
                        .lines()
                        .position(|line| line.contains("alpha"))
                        .unwrap()
                        + 1;
                    assert_eq!(hit["start_line"], focus);
                    assert_eq!(hit["end_line"], focus);
                    assert_eq!(hit["preview"], "    alpha = \"first\"");
                } else {
                    assert_eq!(hit["start_line"], 1);
                    assert_eq!(hit["end_line"], content.lines().count());
                    assert_eq!(hit["preview"], content.trim_end_matches('\n'));
                }
            }
        }
    }
}

fn captured_query(
    root: &Path,
    home: &Path,
    mode: &str,
    text: &str,
    context: usize,
    limit: usize,
) -> (Value, Value) {
    let child = command(root, home, mode)
        .env("IVYGREP_RERANKER_CAPTURE", "1")
        .args(["--lexical-only", "--json", "--verbose", "-C"])
        .arg(context.to_string())
        .arg("--limit")
        .arg(limit.to_string())
        .args(["--", text])
        .arg(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    let records = stderr
        .lines()
        .filter_map(|line| line.strip_prefix("IVYGREP_RERANKER_CAPTURE\t"))
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1, "{stderr}");
    let mut record: Value = serde_json::from_str(records[0]).unwrap();
    assert_eq!(record["process_id"], pid);
    assert_eq!(record["query"], text);
    assert_eq!(record["schema_version"], 1);
    assert_eq!(record["stage"], "pre-learned-accepted-files");
    assert_eq!(record["ranking_context_lines"], 2);
    record.as_object_mut().unwrap().remove("process_id");
    (serde_json::from_slice(&output.stdout).unwrap(), record)
}

#[test]
fn capture_reports_actual_canonical_prelearned_features_and_skipped_gates() {
    let (_temp, root, home) = stage_fixture();
    let text = "alpha \"beta gamma\"";
    let deterministic = query(&root, &home, "deterministic", Some(2));
    let (_, canonical) = captured_query(&root, &home, "learned", text, 2, 10);
    assert_eq!(canonical["status"], "applied");
    assert!(canonical["reason"].is_null());
    assert!(
        canonical["model_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
    );
    for (rank, candidate) in canonical["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let file = &deterministic[rank];
        assert_eq!(candidate["file_path"], file["file_path"]);
        assert_eq!(candidate["total_score"], file["total_score"]);
        assert_eq!(candidate["hit_count"], file["hit_count"]);
        assert_eq!(candidate["canonical_preview"], file["hits"][0]["preview"]);
        assert_eq!(candidate["sources"], file["hits"][0]["sources"]);
        assert_eq!(candidate["baseline_rank"], rank);
        let features = candidate["native_features"].as_array().unwrap();
        assert_eq!(
            features.len(),
            canonical["feature_schema"].as_array().unwrap().len()
        );
        let score = candidate["total_score"].as_f64().unwrap() as f32;
        assert_eq!(
            features[0].as_f64().unwrap() as f32,
            score.ln_1p().min(4.0) / 4.0
        );
        assert!(
            features
                .iter()
                .all(|value| value.as_f64().unwrap().is_finite())
        );
    }
    assert_eq!(canonical["candidates"].as_array().unwrap().len(), 5);
    for context in [0, 2, 20] {
        let (displayed, record) = captured_query(&root, &home, "learned", text, context, 10);
        assert_eq!(displayed, query(&root, &home, "learned", Some(context)));
        assert_eq!(record, canonical);
    }
    for (mode, text, limit, reason) in [
        ("deterministic", text, 10, "deterministic-mode"),
        ("learned", text, 4, "fewer-than-five-files"),
        ("learned", "candidate_a", 10, "route-not-learned"),
    ] {
        let (_, record) = captured_query(&root, &home, mode, text, 2, limit);
        assert_eq!(record["status"], "skipped");
        assert_eq!(record["reason"], reason);
        assert!(record["candidates"].as_array().unwrap().is_empty());
    }
}
