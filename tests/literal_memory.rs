#![cfg(target_os = "linux")]

use std::os::unix::process::CommandExt;
use std::process::Command;

#[test]
fn one_literal_hit_does_not_allocate_every_matching_preview() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("repo");
    let home = fixture.path().join("home");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(
        root.join("events.txt"),
        "needle repeated event\n".repeat(200_000),
    )
    .unwrap();
    let command = || {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
        command
            .env("IVYGREP_HOME", &home)
            .env("IVYGREP_NO_AUTOSPAWN", "1")
            .env("RAYON_NUM_THREADS", "2")
            .env("TOKIO_WORKER_THREADS", "2")
            .env_remove("RUST_LOG");
        command
    };
    let indexed = command()
        .args(["--add", "--no-watch", "--hash"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(indexed.status.success(), "{indexed:?}");

    let mut query = command();
    query
        .args([
            "--literal",
            "needle",
            "--limit",
            "1",
            "-C",
            "100",
            "--hash",
            "--json",
        ])
        .arg(&root);
    // Limit this child's heap, not the test runner or file-backed index maps.
    // The old path needs over 800 MiB of previews for a one-hit response.
    unsafe {
        query.pre_exec(|| {
            let heap = libc::rlimit {
                rlim_cur: 512 * 1024 * 1024,
                rlim_max: 512 * 1024 * 1024,
            };
            let core = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            if libc::setrlimit(libc::RLIMIT_DATA, &heap) != 0
                || libc::setrlimit(libc::RLIMIT_CORE, &core) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let output = query.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let files: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(files.as_array().unwrap().len(), 1);
    assert_eq!(files[0]["hits"].as_array().unwrap().len(), 1);
    assert_eq!(files[0]["hits"][0]["start_line"], 1);
    assert_eq!(files[0]["hits"][0]["end_line"], 101);
}
