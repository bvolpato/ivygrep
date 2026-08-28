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
        "pub struct PaymentRouter { routes: usize }\n\
         pub enum PaymentStatus { Pending, Complete }\n\
         pub fn charge_card() {}\n\
         pub fn run_checkout() { charge_card(); }\n",
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
        ("PaymentRouter", "payments.rs"),
        ("PaymentStatus", "payments.rs"),
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
    let first = first.canonicalize().unwrap();
    let second = second.canonicalize().unwrap();
    let unindexed = unindexed.canonicalize().unwrap();
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
        assert!(paths.iter().any(|path| Path::new(path).starts_with(&first)));
        assert!(
            paths
                .iter()
                .any(|path| Path::new(path).starts_with(&second))
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
        "pub fn shadowed_base_symbol() {}\npub fn shared_name() {}\n\
         pub struct Expected;\nimpl Expected { pub fn run() {} }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("a_deleted.rs"),
        "pub fn deleted_name() {}\n",
    )
    .unwrap();
    fs::write(
        root.path().join("shared.rs"),
        "pub fn inherited_symbol() {}\npub fn shared_name() {}\npub fn deleted_name() {}\n\
         pub struct Unrelated;\nimpl Unrelated { pub fn run() {} }\n",
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
    fs::remove_file(root.path().join("a_deleted.rs")).unwrap();
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
    for symbol in ["shared_name", "deleted_name", "Expected::run"] {
        for limit in [Some(0), Some(1), Some(2), None] {
            let hits = search_symbols(&overlay, symbol, SymbolSearchMode::Definitions, limit, None)
                .unwrap();
            let paths = hits
                .iter()
                .map(|hit| hit.file_path.as_path())
                .collect::<Vec<_>>();
            let expected = if limit == Some(0) {
                vec![]
            } else {
                vec![Path::new("shared.rs")]
            };
            assert_eq!(paths, expected, "{symbol}, limit {limit:?}");
        }
    }

    git(
        root.path(),
        &["worktree", "remove", worktree.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn worktree_symbol_limits_preserve_global_owner_and_name_order() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    git(root.path(), &["init"]);
    git(root.path(), &["symbolic-ref", "HEAD", "refs/heads/main"]);
    fs::write(root.path().join("a.rs"), "pub fn same_name() {}\n").unwrap();
    fs::write(
        root.path().join("z.rs"),
        "pub struct Expected;\nimpl Expected {\n    pub fn run() {}\n    pub fn folded_run() {}\n}\n\
         pub struct Flow;\npub fn shared_name() {}\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base"]);
    index(root.path(), home.path());

    let worktree_parent = tempfile::tempdir().unwrap();
    let worktree = worktree_parent.path().join("feature");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            "-b",
            "feature",
            worktree.to_str().unwrap(),
        ],
    );
    fs::write(
        worktree.join("new.rs"),
        "pub struct Unrelated;\nimpl Unrelated { pub fn run() {} }\n\
         pub fn flow() {}\npub fn shared_name() {}\npub fn same_name() {}\n",
    )
    .unwrap();
    fs::write(
        worktree.join("folded.rs"),
        "pub struct expected;\nimpl expected { pub fn folded_run() {} }\n",
    )
    .unwrap();
    let overlay = index(&worktree, home.path());

    for (symbol, expected) in [
        ("Expected::run", vec!["z.rs"]),
        ("Expected::folded_run", vec!["z.rs"]),
        ("expected::folded_run", vec!["folded.rs"]),
        ("EXPECTED::folded_run", vec!["folded.rs", "z.rs"]),
        ("Missing::run", vec!["new.rs", "z.rs"]),
        ("Flow", vec!["z.rs", "new.rs"]),
        ("flow", vec!["new.rs", "z.rs"]),
        ("same_name", vec!["a.rs", "new.rs"]),
        ("shared_name", vec!["new.rs", "z.rs"]),
    ] {
        for limit in [Some(0), Some(1), Some(2), None] {
            let hits = search_symbols(&overlay, symbol, SymbolSearchMode::Definitions, limit, None)
                .unwrap();
            let paths = hits
                .iter()
                .map(|hit| hit.file_path.as_path())
                .collect::<Vec<_>>();
            let expected = expected
                .iter()
                .take(limit.unwrap_or(usize::MAX))
                .map(Path::new)
                .collect::<Vec<_>>();
            assert_eq!(paths, expected, "{symbol}, limit {limit:?}");
        }
    }
    git(
        root.path(),
        &["worktree", "remove", worktree.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn annotated_definitions_register_their_declared_names() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".git")).unwrap();
    fs::write(
        root.path().join("Worker.java"),
        "public class Worker implements Runnable {\n\
         \x20   @Override public void run() {\n\
         \x20       if (ready()) {\n\
         \x20           dispatch();\n\
         \x20       }\n\
         \x20   }\n\
         }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("profile.py"),
        "class Profile:\n\
         \x20   @property\n\
         \x20   def name(self):\n\
         \x20       return normalize(self._name)\n",
    )
    .unwrap();

    let workspace = index(root.path(), home.path());
    let definitions = |symbol: &str| {
        search_symbols(
            &workspace,
            symbol,
            SymbolSearchMode::Definitions,
            Some(10),
            None,
        )
        .unwrap()
    };

    let run = definitions("run");
    assert!(
        run.iter()
            .any(|hit| hit.file_path == Path::new("Worker.java")),
        "annotated Java method must register its own name: {run:?}"
    );
    let name = definitions("name");
    assert!(
        name.iter()
            .any(|hit| hit.file_path == Path::new("profile.py")),
        "decorated Python method must register its own name: {name:?}"
    );
    for junk in [
        "if",
        "ready",
        "dispatch",
        "normalize",
        "property",
        "Override",
    ] {
        assert!(
            definitions(junk).is_empty(),
            "{junk} must not be registered as a definition"
        );
    }
}

#[test]
#[serial]
fn continuation_windows_register_no_fallback_symbols() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".git")).unwrap();
    // Well past the structural chunk limits so continuation windows exist.
    let mut source = String::from("pub fn long_pipeline() {\n");
    for index in 0..400 {
        source.push_str(&format!("    probe_callee({index});\n"));
    }
    source.push_str("}\n");
    fs::write(root.path().join("pipeline.rs"), source).unwrap();

    let workspace = index(root.path(), home.path());
    let definition = search_symbols(
        &workspace,
        "long_pipeline",
        SymbolSearchMode::Definitions,
        Some(10),
        None,
    )
    .unwrap();
    assert_eq!(definition.len(), 1, "{definition:?}");
    assert_eq!(definition[0].start_line, 1);

    let callee = search_symbols(
        &workspace,
        "probe_callee",
        SymbolSearchMode::Definitions,
        Some(10),
        None,
    )
    .unwrap();
    assert!(
        callee.is_empty(),
        "continuation windows must not register body callees: {callee:?}"
    );
}

#[test]
#[serial]
fn qualified_symbol_lookups_filter_by_owner() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".git")).unwrap();
    fs::write(
        root.path().join("shapes.py"),
        "class Outer:\n\
         \x20   def method(self):\n\
         \x20       return 1\n\
         \n\
         class Other:\n\
         \x20   def method(self):\n\
         \x20       return 2\n",
    )
    .unwrap();
    fs::write(
        root.path().join("engine.rs"),
        "pub struct Engine;\n\
         impl Engine {\n\
         \x20   pub fn method(&self) -> u8 { 1 }\n\
         }\n\
         pub struct Gearbox;\n\
         impl Gearbox {\n\
         \x20   pub fn method(&self) -> u8 { 2 }\n\
         }\n",
    )
    .unwrap();

    let workspace = index(root.path(), home.path());
    let definitions = |symbol: &str| {
        search_symbols(
            &workspace,
            symbol,
            SymbolSearchMode::Definitions,
            Some(10),
            None,
        )
        .unwrap()
    };

    let outer = definitions("Outer.method");
    assert_eq!(outer.len(), 1, "{outer:?}");
    assert_eq!(outer[0].file_path, Path::new("shapes.py"));
    assert_eq!(outer[0].start_line, 2);

    let other = definitions("Other#method");
    assert_eq!(other.len(), 1, "{other:?}");
    assert_eq!(other[0].start_line, 6);

    let engine = definitions("Engine::method");
    assert_eq!(engine.len(), 1, "{engine:?}");
    assert_eq!(engine[0].file_path, Path::new("engine.rs"));
    assert_eq!(engine[0].start_line, 3);

    let gearbox = definitions("Gearbox->method");
    assert_eq!(gearbox.len(), 1, "{gearbox:?}");
    assert_eq!(gearbox[0].start_line, 7);

    // Case-insensitive owner matches rank after exact ones but still filter.
    let folded = definitions("gearbox.method");
    assert_eq!(folded.len(), 1, "{folded:?}");
    assert_eq!(folded[0].start_line, 7);

    // Unknown owners fall back to the bare name.
    let bare = definitions("method");
    assert_eq!(bare.len(), 4, "{bare:?}");
    let unknown_owner = definitions("Missing.method");
    assert_eq!(unknown_owner.len(), 4, "{unknown_owner:?}");

    let mut command = AssertCommand::new(assert_cmd::cargo::cargo_bin!("ig"));
    let output = command
        .current_dir(root.path())
        .env("IVYGREP_HOME", home.path())
        .env("IVYGREP_NO_AUTOSPAWN", "1")
        .args(["--symbol", "--json", "Outer.method"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let hits = value[0]["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1, "{value}");
    assert_eq!(hits[0]["start_line"], 2);
}

#[test]
#[serial]
fn worktree_reference_languages_ignore_shadowed_base_definitions() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    git(root.path(), &["init"]);
    git(root.path(), &["symbolic-ref", "HEAD", "refs/heads/main"]);
    fs::write(root.path().join("service.rs"), "pub fn resolve() {}\n").unwrap();
    fs::write(
        root.path().join("client.py"),
        "def run():\n    return resolve()\n",
    )
    .unwrap();
    git(root.path(), &["add", "."]);
    git(root.path(), &["commit", "-m", "base"]);
    git(root.path(), &["checkout", "-b", "feature"]);
    git(root.path(), &["rm", "-q", "service.rs"]);
    git(root.path(), &["commit", "-m", "drop rust definition"]);
    git(root.path(), &["checkout", "main"]);

    let worktree_parent = tempfile::tempdir().unwrap();
    let worktree = worktree_parent.path().join("feature");
    git(
        root.path(),
        &["worktree", "add", worktree.to_str().unwrap(), "feature"],
    );

    index(root.path(), home.path());
    let overlay = index(&worktree, home.path());
    // The base still defines `resolve` in Rust, but the worktree deleted that
    // file; its language must not restrict the reference scan.
    let refs = search_symbols(
        &overlay,
        "resolve",
        SymbolSearchMode::References,
        Some(10),
        None,
    )
    .unwrap();
    assert!(
        refs.iter().any(|hit| hit.file_path.ends_with("client.py")),
        "python call site must survive when the only definition is shadowed: {refs:?}"
    );

    git(
        root.path(),
        &["worktree", "remove", worktree.to_str().unwrap(), "--force"],
    );
}

#[test]
#[serial]
fn references_are_restricted_to_definition_languages() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".git")).unwrap();
    fs::write(
        root.path().join("resolver.rs"),
        "pub fn resolve(name: &str) -> String { name.to_string() }\n\
         pub fn lookup() -> String { resolve(\"x\") }\n",
    )
    .unwrap();
    fs::write(
        root.path().join("paths.py"),
        "from pathlib import Path\n\
         def here():\n\
         \x20   return Path(__file__).resolve()\n",
    )
    .unwrap();

    let workspace = index(root.path(), home.path());
    for mode in [SymbolSearchMode::References, SymbolSearchMode::Callers] {
        let hits = search_symbols(&workspace, "resolve", mode, Some(10), None).unwrap();
        assert!(
            hits.iter()
                .any(|hit| hit.file_path == Path::new("resolver.rs")),
            "{mode:?} must keep the Rust call site: {hits:?}"
        );
        assert!(
            hits.iter()
                .all(|hit| hit.file_path != Path::new("paths.py")),
            "{mode:?} must not report a Python call for a Rust definition: {hits:?}"
        );
    }

    // Symbols without any known definition keep the plain substring scan.
    let unknown = search_symbols(
        &workspace,
        "Path",
        SymbolSearchMode::References,
        Some(10),
        None,
    )
    .unwrap();
    assert!(
        unknown
            .iter()
            .any(|hit| hit.file_path == Path::new("paths.py")),
        "{unknown:?}"
    );
}
