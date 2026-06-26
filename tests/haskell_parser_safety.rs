use std::fs;
use std::process::Command;

use tempfile::tempdir;

#[test]
fn optimized_haskell_indexing_does_not_corrupt_the_heap() {
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();
    let source = "{-aaaaaaaaaaaaaa aaaa}\n    {-aaa (aaaaaaaaaa [aaaaaaaaaaaaa aaa\n";

    // Multiple files make the stale-pointer corruption deterministic on
    // optimized GCC builds for Linux ARM64.
    for index in 0..64 {
        fs::write(root.path().join(format!("Crash{index}.hs")), source).unwrap();
    }

    let output = Command::new(env!("CARGO_BIN_EXE_ig"))
        .args([
            "--add",
            root.path().to_str().unwrap(),
            "--no-watch",
            "--hash",
            "--json",
            "--force",
        ])
        .env("IVYGREP_HOME", home.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Haskell indexing failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
