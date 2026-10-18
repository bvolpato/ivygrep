use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

fn temporary_directory() -> tempfile::TempDir {
    #[cfg(unix)]
    {
        tempfile::tempdir_in("/tmp").unwrap()
    }
    #[cfg(not(unix))]
    {
        tempfile::tempdir().unwrap()
    }
}

fn command(home: &Path, root: &Path) -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    command
        .current_dir(root)
        .env("IVYGREP_HOME", home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .env("RAYON_NUM_THREADS", "2")
        .env("TOKIO_WORKER_THREADS", "2");
    command
}

fn run(home: &Path, root: &Path, args: &[&str]) -> Value {
    let output = command(home, root).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn result_paths(value: &Value) -> Vec<PathBuf> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|file| serde_json::from_value(file["file_path"].clone()).unwrap())
        .collect()
}

struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_daemon(home: &Path, root: &Path) -> Daemon {
    let child = command(home, root)
        .arg("--daemon")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut daemon = Daemon(child);
    let endpoint = home.join(if cfg!(windows) {
        "daemon.port"
    } else {
        "daemon.sock"
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    while !endpoint.exists() {
        assert!(
            daemon.0.try_wait().unwrap().is_none(),
            "daemon exited early"
        );
        assert!(Instant::now() < deadline, "daemon did not become ready");
        std::thread::sleep(Duration::from_millis(25));
    }
    daemon
}

#[test]
fn literal_unicode_case_folding_does_not_lose_indexed_files() {
    let temporary = temporary_directory();
    let home = temporary.path().join("home");
    let root = temporary.path().join("repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("unicode.txt"), "Kelvin\nKit\naxİ\n").unwrap();
    let indexed = command(&home, &root)
        .args(["--add", ".", "--hash", "--no-watch"])
        .output()
        .unwrap();
    assert!(indexed.status.success(), "{indexed:?}");

    for query in ["kelvin", "KELVIN", "Kelvin", "kit", "axi"] {
        let matches = run(
            &home,
            &root,
            &["--literal", "--no-watch", "--json", "-n", "2", query],
        );
        assert_eq!(
            result_paths(&matches),
            [PathBuf::from("unicode.txt")],
            "{query}"
        );
    }
}

#[test]
fn literal_prefilter_handles_unicode_characters_that_lowercase_to_ascii() {
    let temporary = temporary_directory();
    let home = temporary.path().join("home");
    let root = temporary.path().join("repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    let variants = (0x80..=0x10ffff)
        .filter_map(char::from_u32)
        .filter_map(|character| {
            let lower = character.to_lowercase().collect::<String>();
            lower
                .chars()
                .any(|part| part.is_ascii_alphanumeric())
                .then_some((character, lower))
        })
        .collect::<Vec<_>>();
    assert!(!variants.is_empty());
    for (character, _) in &variants {
        std::fs::write(
            root.join(format!("unicode-{:x}.txt", u32::from(*character))),
            format!("xyz{character}pqr\nab{character}\n"),
        )
        .unwrap();
    }
    let indexed = command(&home, &root)
        .args(["--add", ".", "--hash", "--no-watch"])
        .output()
        .unwrap();
    assert!(indexed.status.success(), "{indexed:?}");

    for (character, lower) in variants {
        let expected = PathBuf::from(format!("unicode-{:x}.txt", u32::from(character)));
        let mut queries = vec![format!("xyz{lower}pqr")];
        if let Some(first) = lower.chars().next().filter(char::is_ascii_alphanumeric) {
            queries.push(format!("ab{first}"));
        }
        for query in queries {
            let matches = run(
                &home,
                &root,
                &["--literal", "--no-watch", "--json", "-n", "10", &query],
            );
            assert!(
                result_paths(&matches).contains(&expected),
                "{character:?} lowercases to {lower:?}; query {query:?}: {matches}"
            );
        }
    }
}

#[test]
fn literal_file_limit_ranking_and_unicode_match_locally_and_through_daemon() {
    let temporary = temporary_directory();
    let home = temporary.path().join("home");
    let root = temporary.path().join("repo");
    let second = temporary.path().join("second");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir_all(second.join(".git")).unwrap();
    std::fs::write(root.join("a.txt"), "needle\n".repeat(3)).unwrap();
    std::fs::write(root.join("b.txt"), "needle\n".repeat(4)).unwrap();
    std::fs::write(root.join("c.txt"), "needle\n").unwrap();
    std::fs::write(root.join("unicode.txt"), "Kelvin\nKit\naxİ\n").unwrap();
    std::fs::write(second.join("z.txt"), "needle\n".repeat(5)).unwrap();

    for repo in [&root, &second] {
        let output = command(&home, repo)
            .args(["--add", ".", "--hash", "--no-watch"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }

    let check = || {
        let files = run(
            &home,
            &root,
            &[
                "--literal",
                "--no-watch",
                "--json",
                "-n",
                "2",
                "-C",
                "0",
                "needle",
            ],
        );
        assert_eq!(
            result_paths(&files),
            [PathBuf::from("b.txt"), PathBuf::from("a.txt")]
        );
        assert_eq!(files[0]["hit_count"], 4);
        assert_eq!(files[0]["hits"].as_array().unwrap().len(), 3);
        assert_eq!(files[0]["more_hits_in_file"], 1);
        assert_eq!(files[1]["hits"].as_array().unwrap().len(), 3);

        let text = command(&home, &root)
            .args(["--literal", "--no-watch", "-n", "2", "-C", "0", "needle"])
            .output()
            .unwrap();
        assert!(text.status.success(), "{text:?}");
        assert!(
            String::from_utf8_lossy(&text.stdout)
                .contains("1 more match omitted. Use --no-limit to show them.")
        );

        let unlimited = run(
            &home,
            &root,
            &[
                "--literal",
                "--no-watch",
                "--json",
                "--no-limit",
                "-C",
                "0",
                "needle",
            ],
        );
        assert_eq!(
            result_paths(&unlimited),
            [
                PathBuf::from("b.txt"),
                PathBuf::from("a.txt"),
                PathBuf::from("c.txt"),
            ]
        );
        assert_eq!(unlimited[0]["hits"].as_array().unwrap().len(), 4);
        assert!(unlimited[0].get("more_hits_in_file").is_none());

        let names = run(
            &home,
            &root,
            &[
                "--literal",
                "--no-watch",
                "--file-name-only",
                "--json",
                "-n",
                "2",
                "-C",
                "100",
                "needle",
            ],
        );
        let names: Vec<PathBuf> = serde_json::from_value(names).unwrap();
        assert_eq!(names, result_paths(&files));

        for limit in [vec![], vec!["--no-limit"]] {
            let mut args = vec!["--literal", "--no-watch", "--file-name-only", "--json"];
            args.extend(limit);
            args.push("needle");
            let names: Vec<PathBuf> = serde_json::from_value(run(&home, &root, &args)).unwrap();
            assert_eq!(
                names,
                [
                    PathBuf::from("b.txt"),
                    PathBuf::from("a.txt"),
                    PathBuf::from("c.txt"),
                ]
            );
        }

        let all = run(
            &home,
            &root,
            &[
                "--literal",
                "--no-watch",
                "--all-indices",
                "--json",
                "-n",
                "2",
                "-C",
                "0",
                "needle",
            ],
        );
        assert_eq!(
            result_paths(&all),
            [
                second.canonicalize().unwrap().join("z.txt"),
                root.canonicalize().unwrap().join("b.txt"),
            ]
        );
        let all_names: Vec<PathBuf> = serde_json::from_value(run(
            &home,
            &root,
            &[
                "--literal",
                "--no-watch",
                "--all-indices",
                "--file-name-only",
                "--json",
                "-n",
                "2",
                "needle",
            ],
        ))
        .unwrap();
        assert_eq!(all_names, result_paths(&all));

        for query in ["kelvin", "KELVIN", "Kelvin", "kit", "axi"] {
            let matches = run(
                &home,
                &root,
                &["--literal", "--no-watch", "--json", "-n", "2", query],
            );
            assert_eq!(
                result_paths(&matches),
                [PathBuf::from("unicode.txt")],
                "{query}"
            );
        }
    };

    check();
    let _daemon = start_daemon(&home, &root);
    check();
}

#[test]
fn literal_all_indices_deduplicates_overlapping_workspaces_before_limiting_files() {
    let temporary = temporary_directory();
    let home = temporary.path().join("home");
    let root = temporary.path().join("repo");
    let nested = root.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("a.txt"), "needle\n".repeat(3)).unwrap();
    std::fs::write(root.join("b.txt"), "needle\n".repeat(2)).unwrap();
    for workspace in [&root, &nested] {
        let output = command(&home, workspace)
            .args(["--add", ".", "--hash", "--no-watch"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }
    let expected = [
        nested.canonicalize().unwrap().join("a.txt"),
        root.canonicalize().unwrap().join("b.txt"),
    ];
    let check = || {
        let files = run(
            &home,
            &root,
            &[
                "--literal",
                "--no-watch",
                "--all-indices",
                "--json",
                "-n",
                "2",
                "needle",
            ],
        );
        assert_eq!(result_paths(&files), expected);
        assert_eq!(files[0]["hit_count"], 3);
        let names: Vec<PathBuf> = serde_json::from_value(run(
            &home,
            &root,
            &[
                "--literal",
                "--no-watch",
                "--all-indices",
                "--file-name-only",
                "--json",
                "-n",
                "2",
                "needle",
            ],
        ))
        .unwrap();
        assert_eq!(names, expected);
    };
    check();
    let _daemon = start_daemon(&home, &root);
    check();
}
