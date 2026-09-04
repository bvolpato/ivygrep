use std::path::{Component, Path, PathBuf};

use anyhow::Result;

use ignore::WalkBuilder;

/// Builds a standard file walker for the given root directory.
///
/// Configuration:
/// - Shows hidden files (but skips `.git` repository/worktree metadata)
/// - Respects `.gitignore`, `.git/info/exclude`, global gitignore, and `.ignore` (unless skip_gitignore is true)
/// - Does not require a git repository
/// - Does not follow symlinks
pub fn source_walker(root: &Path, skip_gitignore: bool) -> WalkBuilder {
    let owned_root = owned_storage_root(root);
    let workspace_root = root.to_path_buf();
    let mut walker = WalkBuilder::new(root);
    walker.hidden(false);
    walker.git_ignore(!skip_gitignore);
    walker.git_exclude(!skip_gitignore);
    walker.git_global(!skip_gitignore);
    walker.ignore(!skip_gitignore);
    walker.require_git(false);
    walker.follow_links(false);
    walker.filter_entry(move |entry| {
        entry.file_name() != ".git"
            && !is_owned_storage_path(&workspace_root, entry.path(), owned_root.as_deref())
    });
    let git_entry = root.join(".git");
    let may_have_external_common_dir = git_entry.is_file() || git_entry.join("commondir").is_file();
    if !skip_gitignore
        && may_have_external_common_dir
        && let Some(common_dir) = crate::workspace::git_common_dir(root)
    {
        let exclude = common_dir.join("info/exclude");
        let checkout_local_exclude = root.join(".git/info/exclude");
        if exclude != checkout_local_exclude && exclude.is_file() {
            walker.current_dir(root);
            let _ = walker.add_ignore(exclude);
        }
    }
    walker
}

/// Request-local ignore rules for source paths, including deleted Git files.
pub(crate) struct SourcePathMatcher {
    root: PathBuf,
    owned_root: Option<PathBuf>,
    ignore: ignore::IncrementalIgnore,
    error: Option<ignore::Error>,
}

impl SourcePathMatcher {
    pub(crate) fn new(root: &Path, skip_gitignore: bool) -> Self {
        Self {
            root: root.to_path_buf(),
            owned_root: owned_storage_root(root),
            ignore: source_walker(root, skip_gitignore)
                .build_matchers()
                .pop()
                .expect("source walker has exactly one root"),
            error: None,
        }
    }

    pub(crate) fn allows(&mut self, path: &Path) -> Result<bool> {
        if path.as_os_str().is_empty()
            || path.components().any(|part| match part {
                Component::Normal(name) => name == ".git",
                Component::CurDir => false,
                _ => true,
            })
            || is_owned_storage_path(
                &self.root,
                &self.root.join(path),
                self.owned_root.as_deref(),
            )
        {
            return Ok(false);
        }
        if let Some(error) = &self.error {
            return Err(error.clone().into());
        }
        let (matched, error) = self.ignore.matched_with_errors(path, false);
        if let Some(error) = error.filter(|error| !is_ignore_pattern_warning(error)) {
            // Directory rules are cached, including failed reads. Keep failures
            // sticky so a later path cannot use a partially loaded policy.
            self.error = Some(error.clone());
            return Err(error.into());
        }
        Ok(!matched.is_ignore())
    }
}

pub(crate) fn is_ivygrep_owned_path(root: &Path, path: &Path) -> bool {
    is_owned_storage_path(root, path, owned_storage_root(root).as_deref())
}

fn owned_storage_root(root: &Path) -> Option<PathBuf> {
    let home = crate::config::app_home().ok()?;
    let canonical_home = home.canonicalize().ok()?;
    let canonical_root = root.canonicalize().ok()?;
    let relative_home = canonical_home.strip_prefix(&canonical_root).ok()?;

    if relative_home.as_os_str().is_empty() {
        Some(root.join("indexes"))
    } else {
        Some(root.join(relative_home))
    }
}

fn is_owned_storage_path(root: &Path, path: &Path, owned_root: Option<&Path>) -> bool {
    let Some(owned_root) = owned_root else {
        return false;
    };
    if path.starts_with(owned_root) {
        return true;
    }

    owned_root == root.join("indexes")
        && path.parent().is_some_and(|parent| parent == root)
        && path.file_name().is_some_and(|name| {
            matches!(
                name.to_str(),
                Some(
                    "daemon.lock"
                        | "daemon.log"
                        | "daemon.log.1"
                        | "daemon.pid"
                        | "daemon.port"
                        | "daemon.sock"
                )
            )
        })
}

pub(crate) fn is_ignore_pattern_warning(error: &ignore::Error) -> bool {
    // The walker reports malformed parent ignore patterns through the same
    // callback as traversal failures. Only pure pattern warnings are harmless;
    // a partial error containing any filesystem failure must still abort.
    match error {
        ignore::Error::Glob { .. } => true,
        ignore::Error::Partial(errors) => {
            !errors.is_empty() && errors.iter().all(is_ignore_pattern_warning)
        }
        ignore::Error::WithLineNumber { err, .. }
        | ignore::Error::WithPath { err, .. }
        | ignore::Error::WithDepth { err, .. } => is_ignore_pattern_warning(err),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::collections::HashSet;

    fn collect_files(root: &Path, skip_gitignore: bool) -> HashSet<String> {
        source_walker(root, skip_gitignore)
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_some_and(|ft| ft.is_file()))
            .map(|e| crate::workspace::index_path_string(e.path().strip_prefix(root).unwrap()))
            .collect()
    }

    #[test]
    fn incremental_source_rules_match_walk_and_cover_deleted_paths() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("nested")).unwrap();
        std::fs::create_dir_all(tmp.path().join("blocked")).unwrap();
        std::fs::write(tmp.path().join(".gitignore"), "*.rs\nblocked/\n").unwrap();
        std::fs::write(tmp.path().join(".ignore"), "!visible.rs\nmissing.rs\n").unwrap();
        std::fs::write(tmp.path().join("nested/.ignore"), "!allowed.rs\n").unwrap();
        std::fs::write(tmp.path().join("blocked/.ignore"), "!child.rs\n").unwrap();
        let paths = [
            "visible.rs",
            "hidden.rs",
            "nested/allowed.rs",
            "blocked/child.rs",
        ];
        for path in paths {
            std::fs::write(tmp.path().join(path), "fn marker() {}\n").unwrap();
        }
        for skip in [false, true] {
            let visible = collect_files(tmp.path(), skip);
            let mut matcher = SourcePathMatcher::new(tmp.path(), skip);
            for path in paths {
                assert_eq!(
                    matcher.allows(Path::new(path)).unwrap(),
                    visible.contains(path),
                    "{path}, skip={skip}"
                );
            }
            assert_eq!(matcher.allows(Path::new("missing.rs")).unwrap(), skip);
            assert!(!matcher.allows(Path::new(".git/config")).unwrap());
            assert!(!matcher.allows(Path::new("../outside.rs")).unwrap());
        }
    }

    #[test]
    fn excludes_dot_git_directory() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git/objects")).unwrap();
        std::fs::write(tmp.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(tmp.path().join("main.rs"), "fn main() {}\n").unwrap();

        let files = collect_files(tmp.path(), false);
        assert!(files.contains("main.rs"));
        assert!(!files.iter().any(|f| f.starts_with(".git/")));
    }

    #[test]
    fn excludes_dot_git_worktree_file_when_skipping_gitignore() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(".git"),
            "gitdir: /tmp/repo/.git/worktrees/example\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join("main.rs"), "fn main() {}\n").unwrap();

        let files = collect_files(tmp.path(), true);
        assert!(files.contains("main.rs"));
        assert!(!files.contains(".git"));
    }

    #[test]
    fn includes_hidden_files() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".env"), "SECRET=42\n").unwrap();
        std::fs::write(tmp.path().join("visible.rs"), "fn f() {}\n").unwrap();

        let files = collect_files(tmp.path(), false);
        assert!(files.contains(".env"), "hidden files should be included");
        assert!(files.contains("visible.rs"));
    }

    #[test]
    fn respects_gitignore_when_not_skipping() {
        let tmp = tempfile::tempdir().unwrap();
        // init a git repo so gitignore is respected
        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        std::fs::write(tmp.path().join(".gitignore"), "ignored.log\n").unwrap();
        std::fs::write(tmp.path().join("ignored.log"), "log data\n").unwrap();
        std::fs::write(tmp.path().join("kept.rs"), "fn f() {}\n").unwrap();

        let files = collect_files(tmp.path(), false);
        assert!(files.contains("kept.rs"));
        assert!(
            !files.contains("ignored.log"),
            "gitignored files should be excluded"
        );
    }

    #[test]
    fn skip_gitignore_includes_ignored_files() {
        let tmp = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(tmp.path())
            .output()
            .unwrap();
        std::fs::write(tmp.path().join(".gitignore"), "ignored.log\n").unwrap();
        std::fs::write(tmp.path().join("ignored.log"), "log data\n").unwrap();
        std::fs::write(tmp.path().join("kept.rs"), "fn f() {}\n").unwrap();

        let files = collect_files(tmp.path(), true);
        assert!(files.contains("kept.rs"));
        assert!(
            files.contains("ignored.log"),
            "skip_gitignore should include ignored files"
        );
    }

    #[test]
    #[serial]
    fn excludes_nested_ivygrep_home_even_when_skipping_gitignore() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("local-state");
        std::fs::create_dir_all(home.join("indexes/workspace")).unwrap();
        std::fs::write(home.join("daemon.log"), "internal state").unwrap();
        std::fs::write(home.join("indexes/workspace/job.json"), "{}").unwrap();
        std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", &home) };

        for skip_gitignore in [false, true] {
            let files = collect_files(root.path(), skip_gitignore);
            assert!(files.contains("main.rs"));
            assert!(!files.iter().any(|path| path.starts_with("local-state/")));
        }
    }

    #[test]
    #[serial]
    fn excludes_only_owned_artifacts_when_home_is_workspace_root() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("indexes/workspace")).unwrap();
        std::fs::write(root.path().join("indexes/workspace/job.json"), "{}").unwrap();
        std::fs::write(root.path().join("daemon.log"), "internal state").unwrap();
        std::fs::write(root.path().join("daemon.log.1"), "rotated internal state").unwrap();
        std::fs::write(root.path().join("main.rs"), "fn main() {}\n").unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", root.path()) };

        let files = collect_files(root.path(), true);
        assert!(files.contains("main.rs"));
        assert!(!files.contains("daemon.log"));
        assert!(!files.contains("daemon.log.1"));
        assert!(!files.iter().any(|path| path.starts_with("indexes/")));
    }
}
