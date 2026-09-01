#![cfg(unix)]

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

use ivygrep::embedding::HashEmbeddingModel;
use ivygrep::indexer::index_workspace;
use ivygrep::search::{SearchContext, SearchOptions, hybrid_search_with_context};
use ivygrep::workspace::Workspace;
use serial_test::serial;

const SOURCE: &str = "fn preview_boundary() {}\nfn inside() { preview_boundary(); }\n";
const OUTSIDE: &str = "fn outside() { preview_boundary(); } // OUTSIDE_PREVIEW_SENTINEL\n";

struct Fixture {
    _directory: tempfile::TempDir,
    workspace: Workspace,
    home: PathBuf,
    source: PathBuf,
    outside: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("container/workspace");
        let home = directory.path().join("home");
        let outside = directory.path().join("outside");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir(&outside).unwrap();
        let source = root.join("src/victim.rs");
        fs::write(&source, SOURCE).unwrap();
        fs::write(outside.join("victim.rs"), OUTSIDE).unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", &home) };
        let workspace = Workspace::resolve(&root).unwrap();
        index_workspace(&workspace, &HashEmbeddingModel::new(256)).unwrap();
        Self {
            _directory: directory,
            workspace,
            home,
            source,
            outside,
        }
    }

    fn replace_with_symlink(&self, parent: bool) {
        if parent {
            fs::rename(
                self.source.parent().unwrap(),
                self.workspace.root.join("original"),
            )
            .unwrap();
            symlink(&self.outside, self.workspace.root.join("src")).unwrap();
        } else {
            fs::rename(&self.source, self.workspace.root.join("original.rs")).unwrap();
            symlink(self.outside.join("victim.rs"), &self.source).unwrap();
        }
    }

    fn cli(&self, args: &[&str], root: &Path) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_ig"))
            .args(args)
            .arg(root)
            .env("IVYGREP_HOME", &self.home)
            .env("IVYGREP_NO_AUTOSPAWN", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
}

#[test]
#[serial]
fn cached_and_uncached_search_previews_reject_replaced_symlinks() {
    for parent in [false, true] {
        let fixture = Fixture::new();
        let options = SearchOptions::default();
        let context = SearchContext::load(&fixture.workspace, None, false).unwrap();
        let search = |context: &SearchContext| {
            hybrid_search_with_context(
                context,
                &fixture.workspace,
                "preview_boundary",
                None,
                &options,
            )
            .unwrap()
        };
        let before = search(&context);
        assert!(!before.is_empty());
        assert!(before.iter().any(|hit| hit.preview.contains("fn inside")));
        fixture.replace_with_symlink(parent);
        for current in [
            context,
            SearchContext::load(&fixture.workspace, None, false).unwrap(),
        ] {
            let after = search(&current);
            assert!(
                !after.is_empty(),
                "indexed fallback should remain available"
            );
            assert!(
                after
                    .iter()
                    .all(|hit| !hit.preview.contains("OUTSIDE_PREVIEW_SENTINEL")
                        && !hit.reason.contains("OUTSIDE_PREVIEW_SENTINEL")),
                "parent={parent}: {after:?}"
            );
            assert!(
                after
                    .iter()
                    .any(|hit| hit.preview.contains("preview_boundary"))
            );
        }
        assert_eq!(
            fs::read_to_string(fixture.outside.join("victim.rs")).unwrap(),
            OUTSIDE
        );
    }
}

#[test]
#[serial]
fn cached_workspace_previews_reject_replaced_root_and_ancestors() {
    for ancestor in [false, true] {
        let fixture = Fixture::new();
        for relative in ["src/victim.rs", "workspace/src/victim.rs"] {
            let target = fixture.outside.join(relative);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, OUTSIDE).unwrap();
        }
        let context = SearchContext::load(&fixture.workspace, None, false).unwrap();
        let options = SearchOptions::default();
        let before = hybrid_search_with_context(
            &context,
            &fixture.workspace,
            "preview_boundary",
            None,
            &options,
        )
        .unwrap();
        assert!(!before.is_empty());
        let replaced = if ancestor {
            fixture.workspace.root.parent().unwrap()
        } else {
            &fixture.workspace.root
        };
        fs::rename(replaced, fixture._directory.path().join("original-root")).unwrap();
        symlink(&fixture.outside, replaced).unwrap();
        let snapshot = fs::read(fixture.workspace.merkle_snapshot_path()).unwrap();
        let error = index_workspace(&fixture.workspace, &HashEmbeddingModel::new(256)).unwrap_err();
        assert!(format!("{error:#}").contains("workspace root"), "{error:#}");
        assert_eq!(
            fs::read(fixture.workspace.merkle_snapshot_path()).unwrap(),
            snapshot,
            "a redirected root must not replace the selected workspace index"
        );
        // Keep the previously selected Workspace, as a daemon's cached context
        // does. Resolving the new symlink as an explicitly selected root would
        // be a different workspace and would not exercise this boundary.
        for current in [
            context,
            SearchContext::load(&fixture.workspace, None, false).unwrap(),
        ] {
            let after = hybrid_search_with_context(
                &current,
                &fixture.workspace,
                "preview_boundary",
                None,
                &options,
            )
            .unwrap();
            assert!(
                !after.is_empty(),
                "indexed fallback should remain available"
            );
            assert!(
                after
                    .iter()
                    .all(|hit| !hit.preview.contains("OUTSIDE_PREVIEW_SENTINEL")
                        && !hit.reason.contains("OUTSIDE_PREVIEW_SENTINEL")),
                "ancestor={ancestor}: {after:?}"
            );
        }
    }
}

#[test]
#[serial]
fn cli_search_modes_and_context_do_not_read_symlink_targets() {
    for parent in [false, true] {
        let fixture = Fixture::new();
        fixture.replace_with_symlink(parent);
        for mode in [
            "--lexical-only",
            "--literal",
            "--regex",
            "--refs",
            "--callers",
        ] {
            let output = fixture.cli(
                &[mode, "--json", "--no-watch", "preview_boundary"],
                &fixture.workspace.root,
            );
            assert!(
                !output.contains("OUTSIDE_PREVIEW_SENTINEL"),
                "{mode}, parent={parent}: {output}"
            );
        }
        let output = fixture.cli(
            &[
                "context",
                "inspect src/victim.rs preview_boundary",
                "--lexical-only",
                "--json",
                "--no-watch",
            ],
            &fixture.workspace.root,
        );
        assert!(
            !output.contains("OUTSIDE_PREVIEW_SENTINEL"),
            "context, parent={parent}: {output}"
        );
        assert_eq!(
            fs::read_to_string(fixture.outside.join("victim.rs")).unwrap(),
            OUTSIDE
        );
    }
}

#[test]
#[serial]
fn cli_accepts_an_explicitly_symlinked_workspace_root() {
    let fixture = Fixture::new();
    let alias = fixture._directory.path().join("selected-workspace");
    symlink(&fixture.workspace.root, &alias).unwrap();
    let output = fixture.cli(
        &["--lexical-only", "--json", "--no-watch", "preview_boundary"],
        &alias,
    );
    assert!(output.contains("fn inside"), "{output}");
    assert!(!output.contains("live file unavailable"), "{output}");
}
