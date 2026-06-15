use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::Command as AssertCommand;
use ivygrep::EMBEDDING_DIMENSIONS;
use ivygrep::embedding::HashEmbeddingModel;
use ivygrep::indexer::index_workspace;
use ivygrep::symbols::{SymbolSearchMode, search_symbols};
use ivygrep::workspace::Workspace;
use serial_test::serial;

fn index(root: &Path, home: &Path) -> Workspace {
    unsafe { std::env::set_var("IVYGREP_HOME", home) };
    let workspace = Workspace::resolve(root).unwrap();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    index_workspace(&workspace, &model).unwrap();
    workspace
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "commit.gpgSign=false"])
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[serial]
fn multilingual_symbols_references_and_incremental_deletion() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".git")).unwrap();
    fs::write(
        root.path().join("payments.rs"),
        "pub fn charge_card() {}\npub fn run_checkout() { charge_card(); }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("payments.py"),
        "def authorize_payment(card):\n    return card.is_valid()\n",
    )
    .unwrap();
    fs::write(
        root.path().join("trace.go"),
        "package trace\n\
         func shouldSampleTrace(priority int) bool { return priority > 0 }\n\
         type Sampler struct{}\n\
         func (sampler *Sampler) KeepTrace(priority int) bool { return shouldSampleTrace(priority) }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("flags.ts"),
        "export function isFeatureEnabled(name: string): boolean { return true; }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("CacheManager.java"),
        "class CacheManager {\n    public void invalidateCache(String key) {\n        remove(key);\n    }\n}\n",
    )
    .unwrap();

    let workspace = index(root.path(), home.path());
    for (symbol, expected_file) in [
        ("charge_card", "payments.rs"),
        ("authorize_payment", "payments.py"),
        ("shouldSampleTrace", "trace.go"),
        ("KeepTrace", "trace.go"),
        ("isFeatureEnabled", "flags.ts"),
        ("invalidateCache", "CacheManager.java"),
    ] {
        let hits = search_symbols(
            &workspace,
            symbol,
            SymbolSearchMode::Definitions,
            Some(10),
            None,
        )
        .unwrap();
        assert!(
            hits.iter()
                .any(|hit| hit.file_path == Path::new(expected_file)),
            "missing {symbol} definition in {expected_file}: {hits:?}"
        );
    }

    let references = search_symbols(
        &workspace,
        "charge_card",
        SymbolSearchMode::References,
        Some(10),
        None,
    )
    .unwrap();
    assert!(
        references
            .iter()
            .any(|hit| hit.file_path == Path::new("payments.rs"))
    );
    assert!(
        references
            .iter()
            .all(|hit| hit.preview.contains("run_checkout")),
        "the definition itself must not be reported as a reference: {references:?}"
    );
    let callers = search_symbols(
        &workspace,
        "charge_card",
        SymbolSearchMode::Callers,
        Some(10),
        None,
    )
    .unwrap();
    assert!(
        callers
            .iter()
            .any(|hit| hit.preview.contains("run_checkout"))
    );

    fs::write(
        root.path().join("payments.rs"),
        "pub fn replacement_gateway() {}\n",
    )
    .unwrap();
    let workspace = index(root.path(), home.path());
    assert!(
        search_symbols(
            &workspace,
            "charge_card",
            SymbolSearchMode::Definitions,
            Some(10),
            None,
        )
        .unwrap()
        .is_empty()
    );
    assert!(
        search_symbols(
            &workspace,
            "charge_card",
            SymbolSearchMode::References,
            Some(10),
            None,
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
#[serial]
fn symbol_cli_supports_json_and_human_output() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".git")).unwrap();
    fs::write(
        root.path().join("lib.rs"),
        "pub fn calculate_invoice_total() -> u64 { 42 }\n",
    )
    .unwrap();
    index(root.path(), home.path());

    let mut json = AssertCommand::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = json
        .current_dir(root.path())
        .env("IVYGREP_HOME", home.path())
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--symbol", "--json", "calculate_invoice_total"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value[0]["hits"][0]["sources"][0], "symbol");

    let mut human = AssertCommand::new(assert_cmd::cargo::cargo_bin!("ig"));
    human
        .current_dir(root.path())
        .env("IVYGREP_HOME", home.path())
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--symbol", "calculate_invoice_total"])
        .assert()
        .success()
        .stdout(predicates::str::contains("calculate_invoice_total"));
}

#[test]
#[serial]
fn symbol_cli_respects_type_and_path_filters() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".git")).unwrap();
    fs::create_dir_all(root.path().join("src")).unwrap();
    fs::create_dir_all(root.path().join("tools")).unwrap();
    fs::write(
        root.path().join("src/service.rs"),
        "pub fn shared_symbol() {}\n",
    )
    .unwrap();
    fs::write(
        root.path().join("tools/service.py"),
        "def shared_symbol():\n    return True\n",
    )
    .unwrap();
    fs::write(
        root.path().join("tools/ignored.py"),
        "def shared_symbol():\n    return False\n",
    )
    .unwrap();
    fs::write(root.path().join(".gitignore"), "tools/ignored.py\n").unwrap();

    let mut add = AssertCommand::new(assert_cmd::cargo::cargo_bin!("ig"));
    add.current_dir(root.path())
        .env("IVYGREP_HOME", home.path())
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--add", "--no-watch", "--skip-gitignore", "."])
        .assert()
        .success();

    let mut command = AssertCommand::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = command
        .current_dir(root.path())
        .env("IVYGREP_HOME", home.path())
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--symbol",
            "--json",
            "--type",
            "py",
            "--include",
            "tools/**",
            "--exclude",
            "**/ignored.py",
            "shared_symbol",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let paths = value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["file_path"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 1);
    assert!(paths[0].ends_with("tools/service.py"), "{paths:?}");

    let mut include_ignored = AssertCommand::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = include_ignored
        .current_dir(root.path())
        .env("IVYGREP_HOME", home.path())
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args([
            "--symbol",
            "--json",
            "--type",
            "py",
            "--include",
            "tools/**",
            "--skip-gitignore",
            "shared_symbol",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let paths = value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["file_path"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 2, "{paths:?}");
    assert!(
        paths.iter().any(|path| path.ends_with("tools/ignored.py")),
        "{paths:?}"
    );
}

#[test]
#[serial]
fn all_indices_symbol_search_uses_absolute_indexed_paths() {
    let parent = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let first = parent.path().join("first");
    let second = parent.path().join("second");
    let unindexed = parent.path().join("unindexed");
    for root in [&first, &second, &unindexed] {
        fs::create_dir_all(root).unwrap();
    }
    for root in [&first, &second] {
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join("service.rs"), "pub fn shared_symbol() {}\n").unwrap();
        index(root, home.path());
    }

    for cwd in [&first, &unindexed] {
        let mut command = AssertCommand::new(assert_cmd::cargo::cargo_bin!("ig"));
        let output = command
            .current_dir(cwd)
            .env("IVYGREP_HOME", home.path())
            .env("IVYGREP_NO_AUTOSPAWN", "1")
            .args(["--all-indices", "--symbol", "--json", "shared_symbol"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
        let paths = value
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|entry| entry["file_path"].as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 2, "{paths:?}");
        assert!(paths.iter().all(|path| Path::new(path).is_absolute()));
        assert!(
            paths
                .iter()
                .any(|path| path.starts_with(first.to_str().unwrap()))
        );
        assert!(
            paths
                .iter()
                .any(|path| path.starts_with(second.to_str().unwrap()))
        );
    }
}

#[test]
#[serial]
fn worktree_overlay_hides_shadowed_base_symbols() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    git(root.path(), &["init"]);
    git(root.path(), &["symbolic-ref", "HEAD", "refs/heads/main"]);
    fs::write(
        root.path().join("service.rs"),
        "pub fn shadowed_base_symbol() {}\n",
    )
    .unwrap();
    fs::write(
        root.path().join("shared.rs"),
        "pub fn inherited_symbol() {}\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base"]);
    git(root.path(), &["checkout", "-b", "feature"]);
    fs::write(
        root.path().join("service.rs"),
        "pub fn replacement_symbol() {}\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "replace symbol"]);
    git(root.path(), &["checkout", "main"]);

    let worktree_parent = tempfile::tempdir().unwrap();
    let worktree = worktree_parent.path().join("feature");
    git(
        root.path(),
        &["worktree", "add", worktree.to_str().unwrap(), "feature"],
    );

    index(root.path(), home.path());
    let overlay = index(&worktree, home.path());
    assert!(
        search_symbols(
            &overlay,
            "shadowed_base_symbol",
            SymbolSearchMode::Definitions,
            Some(10),
            None,
        )
        .unwrap()
        .is_empty()
    );
    for symbol in ["replacement_symbol", "inherited_symbol"] {
        assert!(
            !search_symbols(
                &overlay,
                symbol,
                SymbolSearchMode::Definitions,
                Some(10),
                None,
            )
            .unwrap()
            .is_empty(),
            "worktree should find {symbol}"
        );
    }

    git(
        root.path(),
        &["worktree", "remove", worktree.to_str().unwrap(), "--force"],
    );
}
