use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;

use ignore::WalkBuilder;

/// Builds a standard file walker for the given root directory.
///
/// Configuration:
/// - Shows hidden files (but skips `.git` repository/worktree metadata)
/// - Skips linked worktrees and clones nested in a Git workspace; submodules stay
/// - Respects `.gitignore`, `.git/info/exclude`, global gitignore, and `.ignore` (unless skip_gitignore is true)
/// - Does not require a git repository. The root of one bounds `.gitignore` and
///   `info/exclude` as in Git. `.ignore` files above the root always apply
/// - Does not follow symlinks
pub fn source_walker(root: &Path, skip_gitignore: bool) -> WalkBuilder {
    source_walker_scoped(root, skip_gitignore, None)
}

pub(crate) fn source_walker_scoped(
    root: &Path,
    skip_gitignore: bool,
    scope: Option<&Path>,
) -> WalkBuilder {
    let scope = scope.map(|path| root.join(path));
    let owned_root = owned_storage_root(root);
    let workspace_root = root.to_path_buf();
    let git_entry = root.join(".git");
    let may_have_external_common_dir = git_entry.is_file() || git_entry.join("commondir").is_file();
    let exclude = (!skip_gitignore && may_have_external_common_dir)
        .then(|| crate::workspace::git_common_dir(root))
        .flatten()
        .map(|common_dir| common_dir.join("info/exclude"));
    let mut walker = source_ignore_builder(root, skip_gitignore, exclude.as_deref());
    walker.follow_links(false);
    let nested_checkouts = crate::workspace::NestedCheckouts::new(root);
    let enclosing_rules = EnclosingGitRules::new(root, skip_gitignore).map(Mutex::new);
    let needs_enclosing_rules = AtomicBool::new(false);
    walker.filter_entry(move |entry| {
        let is_dir = entry.depth() > 0 && entry.file_type().is_some_and(|kind| kind.is_dir());
        scope.as_ref().is_none_or(|scope| scope.starts_with(entry.path()) || entry.path().starts_with(scope))
            && entry.file_name() != ".git"
            && !is_owned_storage_path(&workspace_root, entry.path(), owned_root.as_deref())
            // A nested worktree or clone is a workspace of its own.
            && !(is_dir && nested_checkouts.is_separate_checkout(&workspace_root, entry.path()))
            && !enclosing_rules.as_ref().is_some_and(|rules| {
                // The walk calls this for a directory before its entries. Most
                // workspaces need no lock, because no directory needs the rules.
                if !needs_enclosing_rules.load(Ordering::Relaxed) {
                    if is_dir && repository_boundary(entry.path()) == Some(false) {
                        needs_enclosing_rules.store(true, Ordering::Relaxed);
                    }
                    return false;
                }
                entry.path().strip_prefix(&workspace_root).is_ok_and(|rel| {
                    let matched = rules.lock().unwrap().matched(rel, is_dir);
                    matched.is_some_and(|(matched, _)| matched.is_ignore())
                })
            })
    });
    walker
}

pub(crate) fn source_ignore_builder(
    root: &Path,
    skip_gitignore: bool,
    exclude: Option<&Path>,
) -> WalkBuilder {
    let mut walker = WalkBuilder::new(root);
    walker.hidden(false);
    walker.git_ignore(!skip_gitignore);
    walker.git_exclude(!skip_gitignore);
    walker.git_global(!skip_gitignore);
    walker.ignore(!skip_gitignore);
    // The walker stops `.gitignore` rules at a repository root only in this
    // mode. Without it, a plain directory keeps its own and inherited rules.
    walker.require_git(crate::workspace::is_git_workspace_root(root));
    if !skip_gitignore
        && let Some(exclude) = exclude
        && exclude != root.join(".git/info/exclude")
        && exclude.is_file()
    {
        walker.current_dir(root);
        let _ = walker.add_ignore(exclude);
    }
    walker
}

fn incremental_ignore(walker: &WalkBuilder) -> ignore::IncrementalIgnore {
    walker
        .build_matchers()
        .pop()
        .expect("source walker has exactly one root")
}

/// Whether `dir` is a repository. `None` if it has no entry that stops Git
/// rules in the `ignore` crate.
fn repository_boundary(dir: &Path) -> Option<bool> {
    [".git", ".jj"]
        .iter()
        .any(|name| dir.join(name).exists())
        .then(|| crate::workspace::is_git_workspace_root(dir))
}

/// Git rules below a directory with a `.git` or `.jj` entry that is not a
/// repository, such as a worktree whose repository moved. The `ignore` crate
/// stops Git rules there. In Git, the rules of the enclosing repository apply.
///
/// These rules only hide more: below such a directory, a `!` rule of the
/// enclosing repository does not override the global ignore file or an
/// `info/exclude` that `add_ignore` adds.
struct EnclosingGitRules {
    root: PathBuf,
    /// The `repository_boundary` of each directory below `root`.
    boundaries: HashMap<PathBuf, Option<bool>>,
    /// For each enclosing repository.
    matchers: HashMap<PathBuf, ignore::IncrementalIgnore>,
}

impl EnclosingGitRules {
    /// `None` if the walker does not stop Git rules below `root`.
    fn new(root: &Path, skip_gitignore: bool) -> Option<Self> {
        (!skip_gitignore && crate::workspace::is_git_workspace_root(root)).then(|| Self {
            root: root.to_path_buf(),
            boundaries: HashMap::new(),
            matchers: HashMap::new(),
        })
    }

    /// `None` if `rel` is not below such a directory.
    fn matched(
        &mut self,
        rel: &Path,
        is_dir: bool,
    ) -> Option<(ignore::IncrementalMatch, Option<ignore::Error>)> {
        let mut repository = Path::new("");
        let mut stops_early = false;
        for dir in rel.ancestors().skip(1).filter(|dir| *dir != Path::new("")) {
            let boundary = self
                .boundaries
                .entry(dir.to_path_buf())
                .or_insert_with(|| repository_boundary(&self.root.join(dir)));
            if *boundary == Some(true) {
                repository = dir;
                break;
            }
            stops_early |= boundary.is_some();
        }
        if !stops_early {
            return None;
        }
        let matcher = self
            .matchers
            .entry(repository.to_path_buf())
            .or_insert_with(|| {
                let mut walker = source_ignore_builder(&self.root.join(repository), false, None);
                walker.require_git(false).parents(false);
                incremental_ignore(&walker)
            });
        Some(matcher.matched_with_errors(rel.strip_prefix(repository).ok()?, is_dir))
    }
}

/// The ignore rules of the source walker for single paths.
pub(crate) struct SourceIgnore {
    rules: ignore::IncrementalIgnore,
    enclosing_rules: Option<EnclosingGitRules>,
}

impl SourceIgnore {
    pub(crate) fn new(walker: &WalkBuilder, skip_gitignore: bool) -> Self {
        let rules = incremental_ignore(walker);
        Self {
            enclosing_rules: EnclosingGitRules::new(rules.root(), skip_gitignore),
            rules,
        }
    }

    /// Whether the walker ignores `rel`. A malformed pattern is not an error.
    pub(crate) fn ignores(&mut self, rel: &Path, is_dir: bool) -> Result<bool, ignore::Error> {
        let enclosing = self.enclosing_rules.as_mut();
        let enclosing = enclosing.and_then(|rules| rules.matched(rel, is_dir));
        let mut ignored = false;
        for (matched, error) in [Some(self.rules.matched_with_errors(rel, is_dir)), enclosing]
            .into_iter()
            .flatten()
        {
            if let Some(error) = error.filter(|error| !is_ignore_pattern_warning(error)) {
                return Err(error);
            }
            ignored |= matched.is_ignore();
        }
        Ok(ignored)
    }
}

/// Request-local ignore rules for source paths, including deleted Git files.
pub(crate) struct SourcePathMatcher {
    root: PathBuf,
    owned_root: Option<PathBuf>,
    nested_checkouts: crate::workspace::NestedCheckouts,
    ignore: SourceIgnore,
    error: Option<ignore::Error>,
}

impl SourcePathMatcher {
    pub(crate) fn new(root: &Path, skip_gitignore: bool) -> Self {
        Self {
            root: root.to_path_buf(),
            owned_root: owned_storage_root(root),
            nested_checkouts: crate::workspace::NestedCheckouts::new(root),
            ignore: SourceIgnore::new(&source_walker(root, skip_gitignore), skip_gitignore),
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
            || self
                .nested_checkouts
                .contains(&self.root, &self.root.join(path))
        {
            return Ok(false);
        }
        if let Some(error) = &self.error {
            return Err(error.clone().into());
        }
        match self.ignore.ignores(path, false) {
            Ok(ignored) => Ok(!ignored),
            Err(error) => {
                // Directory rules are cached, including failed reads. Keep
                // failures sticky so a later path cannot use a partially
                // loaded policy.
                self.error = Some(error.clone());
                Err(error.into())
            }
        }
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

    fn write_git_dir(path: &Path, linked_worktree: bool) {
        std::fs::create_dir_all(path).unwrap();
        std::fs::write(path.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        if linked_worktree {
            std::fs::write(path.join("commondir"), "../..\n").unwrap();
        } else {
            std::fs::create_dir_all(path.join("objects")).unwrap();
            std::fs::create_dir_all(path.join("refs")).unwrap();
        }
    }

    /// A workspace holding a linked worktree, a nested clone, a clone whose
    /// Git directory is elsewhere (`git clone --separate-git-dir`), an
    /// absorbed submodule, a submodule with an embedded `.git` directory, and
    /// a listed submodule whose Git directory is elsewhere. `outside` is a
    /// directory beside the workspace for the Git directories kept elsewhere.
    fn write_nested_checkouts(root: &Path, outside: &Path) {
        let file = |path: &str, contents: &str| {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        };
        file("detached/lib.rs", "fn detached() {}\n");
        write_git_dir(&outside.join("detached.git"), false);
        file(
            "detached/.git",
            &format!("gitdir: {}\n", outside.join("detached.git").display()),
        );
        file("vendor/listed/lib.rs", "fn listed() {}\n");
        write_git_dir(&outside.join("listed.git"), false);
        file(
            "vendor/listed/.git",
            &format!("gitdir: {}\n", outside.join("listed.git").display()),
        );
        file("main.rs", "fn main() {}\n");
        file(".claude/notes.md", "kept\n");
        file(".claude/worktrees/agent/main.rs", "fn main() {}\n");
        file(".claude/worktrees/agent/.gitignore", "target/\n");
        write_git_dir(&root.join(".git/worktrees/agent"), true);
        file(
            ".claude/worktrees/agent/.git",
            &format!("gitdir: {}\n", root.join(".git/worktrees/agent").display()),
        );
        file("clone/lib.rs", "fn clone() {}\n");
        write_git_dir(&root.join("clone/.git"), false);
        file("vendor/absorbed/lib.rs", "fn absorbed() {}\n");
        write_git_dir(&root.join(".git/modules/absorbed"), false);
        file(
            "vendor/absorbed/.git",
            "gitdir: ../../.git/modules/absorbed\n",
        );
        file("vendor/embedded/lib.rs", "fn embedded() {}\n");
        write_git_dir(&root.join("vendor/embedded/.git"), false);
        file(
            ".gitmodules",
            "[submodule \"embedded\"]\n\tpath = vendor/embedded\n\turl = https://example.invalid/e.git\n\
             [submodule \"listed\"]\n\tpath = vendor/listed\n\turl = https://example.invalid/l.git\n",
        );
    }

    #[test]
    fn nested_worktrees_and_clones_are_separate_workspaces_but_submodules_stay() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        write_git_dir(&root.join(".git"), false);
        write_nested_checkouts(&root, &tmp.path().join("git-dirs"));

        for skip_gitignore in [false, true] {
            let mut files = collect_files(&root, skip_gitignore)
                .into_iter()
                .collect::<Vec<_>>();
            files.sort();
            assert_eq!(
                files,
                [
                    ".claude/notes.md",
                    ".gitmodules",
                    "main.rs",
                    "vendor/absorbed/lib.rs",
                    "vendor/embedded/lib.rs",
                    "vendor/listed/lib.rs"
                ]
            );
            let mut matcher = SourcePathMatcher::new(&root, skip_gitignore);
            for (path, allowed) in [
                ("main.rs", true),
                ("vendor/absorbed/lib.rs", true),
                ("vendor/embedded/lib.rs", true),
                ("vendor/listed/lib.rs", true),
                (".claude/worktrees/agent/main.rs", false),
                (".claude/worktrees/agent/deleted.rs", false),
                ("clone/lib.rs", false),
                // `git clone --separate-git-dir`: a full Git directory that
                // is neither a listed submodule nor below `.git/modules`.
                ("detached/lib.rs", false),
                // A deleted worktree's paths must still reach the index as deletions.
                (".claude/worktrees/removed/main.rs", true),
            ] {
                assert_eq!(matcher.allows(Path::new(path)).unwrap(), allowed, "{path}");
            }
        }
    }

    /// The same rule against checkouts that Git itself made.
    #[test]
    fn git_made_submodule_stays_and_separate_git_dir_clone_and_worktree_do_not() {
        let tmp = tempfile::tempdir().unwrap();
        let git = |dir: &Path, args: &[&str]| {
            let output = std::process::Command::new("git")
                .args([
                    "-c",
                    "user.name=ivygrep",
                    "-c",
                    "user.email=ivygrep@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "protocol.file.allow=always",
                ])
                .args(args)
                .current_dir(dir)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        let upstream = tmp.path().join("upstream");
        let root = tmp.path().join("repo");
        for repository in [&upstream, &root] {
            std::fs::create_dir(repository).unwrap();
            git(repository, &["init", "-b", "main"]);
            std::fs::write(repository.join("lib.rs"), "fn lib() {}\n").unwrap();
            git(repository, &["add", "."]);
            git(repository, &["commit", "-m", "initial"]);
        }
        let upstream_url = upstream.to_str().unwrap();
        git(&root, &["submodule", "add", upstream_url, "vendor/sub"]);
        git(&root, &["commit", "-m", "submodule"]);
        let detached_git_dir = tmp.path().join("detached.git");
        git(
            &root,
            &[
                "clone",
                "--separate-git-dir",
                detached_git_dir.to_str().unwrap(),
                upstream_url,
                "detached",
            ],
        );
        git(
            &root,
            &["worktree", "add", "-b", "agent", ".claude/worktrees/agent"],
        );
        // Git absorbed the submodule's directory and gave the clone a `.git`
        // file as well: only where they point tells the two apart.
        assert!(root.join("vendor/sub/.git").is_file());
        assert!(root.join("detached/.git").is_file());

        let mut files = collect_files(&root, false).into_iter().collect::<Vec<_>>();
        files.sort();
        assert_eq!(files, [".gitmodules", "lib.rs", "vendor/sub/lib.rs"]);
    }

    #[test]
    fn a_plain_directory_of_clones_keeps_indexing_them() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("clones");
        write_nested_checkouts(&root, &tmp.path().join("git-dirs"));

        let files = collect_files(&root, false);
        assert!(files.contains("clone/lib.rs"));
        assert!(files.contains("detached/lib.rs"));
        assert!(files.contains(".claude/worktrees/agent/main.rs"));
    }

    #[test]
    fn git_rules_stop_at_the_repository_root() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".gitignore"), "*\n").unwrap();
        std::fs::write(tmp.path().join(".ignore"), "hidden.rs\n").unwrap();
        let repo = tmp.path().join("repo");
        write_git_dir(&repo.join(".git"), false);
        std::fs::write(repo.join(".gitignore"), ".worktrees/**\n").unwrap();
        let worktree = repo.join(".worktrees/agent");
        let worktree_git_dir = repo.join(".git/worktrees/agent");
        write_git_dir(&worktree_git_dir, true);
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", worktree_git_dir.display()),
        )
        .unwrap();
        let plain = tmp.path().join("plain");
        std::fs::create_dir(&plain).unwrap();

        for (root, applies_outer_git_rules) in [(&repo, false), (&worktree, false), (&plain, true)]
        {
            std::fs::write(root.join("visible.rs"), "fn visible() {}\n").unwrap();
            std::fs::write(root.join("hidden.rs"), "fn hidden() {}\n").unwrap();
            let files = collect_files(root, false);
            let mut matcher = SourcePathMatcher::new(root, false);
            for (path, allowed) in [
                ("visible.rs", !applies_outer_git_rules),
                ("hidden.rs", false),
            ] {
                assert_eq!(files.contains(path), allowed, "{root:?} {path}");
                assert_eq!(
                    matcher.allows(Path::new(path)).unwrap(),
                    allowed,
                    "{root:?} {path}"
                );
            }
        }
    }

    /// Git does not accept these entries as repositories.
    #[test]
    fn git_rules_continue_below_an_entry_that_is_not_a_repository() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        let file = |path: String, contents: &str| {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        };
        write_git_dir(&root.join(".git"), false);
        file(".git/info/exclude".into(), "excluded.rs\n");
        file(".gitignore".into(), "*.env\n");
        // A submodule has rules of its own.
        let submodule = |path: &str| {
            let git_dir = root.join(".git/modules").join(path);
            write_git_dir(&git_dir, false);
            file(
                format!("{path}/.git"),
                &format!("gitdir: {}\n", git_dir.display()),
            );
            file(format!("{path}/.gitignore"), "*.log\n");
        };
        submodule("sub");
        submodule("jj/sub");

        let mut paths = Vec::new();
        for parent in ["", "sub/", "jj/"] {
            for (entry, contents) in [
                ("empty-directory/.git/info", None),
                ("broken-file/.git", Some("gitdir: ../missing\n")),
                ("empty-file/.git", Some("")),
                ("jj/.jj", None),
            ] {
                let entry = format!("{parent}{entry}");
                match contents {
                    Some(contents) => file(entry.clone(), contents),
                    None => std::fs::create_dir_all(root.join(&entry)).unwrap(),
                }
                let dir = entry.split("/.").next().unwrap();
                file(format!("{dir}/own/.gitignore"), "!own.env\n");
                let in_submodule = parent == "sub/";
                for (name, allowed) in [
                    ("lib.rs", true),
                    ("secret.env", in_submodule),
                    ("deep/secret.env", in_submodule),
                    ("own/own.env", true),
                    ("excluded.rs", in_submodule),
                    ("debug.log", !in_submodule),
                ] {
                    paths.push((format!("{dir}/{name}"), allowed));
                }
            }
        }
        paths.push(("jj/sub/secret.env".into(), true));
        paths.push(("jj/sub/debug.log".into(), false));
        for (path, _) in &paths {
            file(path.clone(), "marker\n");
        }

        let files = collect_files(&root, false);
        let mut matcher = SourcePathMatcher::new(&root, false);
        for (path, allowed) in &paths {
            assert_eq!(files.contains(path), *allowed, "{path}");
            assert_eq!(matcher.allows(Path::new(path)).unwrap(), *allowed, "{path}");
        }
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
