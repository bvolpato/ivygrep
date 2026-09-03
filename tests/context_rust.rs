use std::path::Path;

use assert_cmd::Command;
use serde_json::Value;

fn command(root: &Path, home: &Path) -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("ig"));
    command
        .current_dir(root)
        .env("IVYGREP_HOME", home)
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .env("IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT", "1");
    command
}

fn write(root: &Path, path: &str, content: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn assert_dependency(root: &Path, home: &Path, source: &str, expected: &str) {
    let output = command(root, home)
        .args([
            "context",
            &format!("Review {source}:1"),
            "--lexical-only",
            "--no-watch",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let context: Value = serde_json::from_slice(&output).unwrap();
    let prefix = format!("{source} depends on ");
    let dependencies = context["items"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|item| item["reasons"].as_array().unwrap())
        .filter_map(|reason| reason.as_str().unwrap().strip_prefix(&prefix))
        .collect::<Vec<_>>();
    assert_eq!(dependencies, [expected], "{context:#}");
}

#[test]
fn context_uses_member_crate_and_refreshes_changed_target_roots() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("workspace");
    let home = temp.path().join("home");
    write(&root, "Cargo.toml", "[package]\nname = 'outer'\n");
    write(&root, "src/auth.rs", "pub struct WrongSession;\n");
    write(
        &root,
        "crates/core/Cargo.toml",
        "[package]\nname = 'core'\n[lib]\npath = 'library/root.rs'\n",
    );
    write(
        &root,
        "crates/core/library/root.rs",
        "pub mod auth;\npub mod nested;\n",
    );
    write(
        &root,
        "crates/core/library/nested/mod.rs",
        "pub mod service;\n",
    );
    write(
        &root,
        "crates/core/library/auth.rs",
        "pub struct Session;\n",
    );
    write(
        &root,
        "crates/core/library/nested/root.rs",
        "pub mod auth;\npub mod service;\n",
    );
    write(
        &root,
        "crates/core/library/nested/auth.rs",
        "pub struct Session;\n",
    );
    let source = "crates/core/library/nested/service.rs";
    write(&root, source, "use crate::auth::Session;\n");
    command(&root, &home)
        .args(["--add", ".", "--lexical-only", "--no-watch"])
        .assert()
        .success();
    assert_dependency(&root, &home, source, "crates/core/library/auth.rs");

    // An unchanged workspace with the old graph format must rebuild its stored
    // relationships, not preserve the old cross-package edge indefinitely.
    let index = std::fs::read_dir(home.join("indexes"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let sqlite = rusqlite::Connection::open(index.join("metadata.sqlite3")).unwrap();
    sqlite
        .execute(
            "UPDATE file_edges SET target_path = 'src/auth.rs' WHERE source_path = ?1 AND kind = 1",
            [source],
        )
        .unwrap();
    drop(sqlite);
    std::fs::write(index.join("index_format_version"), "26").unwrap();
    command(&root, &home)
        .args(["--add", ".", "--lexical-only", "--no-watch"])
        .assert()
        .success();
    assert_dependency(&root, &home, source, "crates/core/library/auth.rs");
    assert_eq!(
        std::fs::read_to_string(index.join("index_format_version")).unwrap(),
        ivygrep::workspace::INDEX_FORMAT_VERSION.to_string()
    );

    write(
        &root,
        "crates/core/Cargo.toml",
        "[package]\nname = 'core'\n[lib]\npath = 'library/nested/root.rs'\n",
    );
    command(&root, &home)
        .args(["--add", ".", "--lexical-only", "--no-watch"])
        .assert()
        .success();
    assert_dependency(&root, &home, source, "crates/core/library/nested/auth.rs");
}
