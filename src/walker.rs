use std::path::Path;

use ignore::WalkBuilder;

/// Builds a standard file walker for the given root directory.
///
/// Configuration:
/// - Shows hidden files (but skips `.git/`)
/// - Respects `.gitignore`, `.git/info/exclude`, global gitignore, and `.ignore` (unless skip_gitignore is true)
/// - Does not require a git repository
/// - Does not follow symlinks
pub fn source_walker(root: &Path, skip_gitignore: bool) -> WalkBuilder {
    let mut walker = WalkBuilder::new(root);
    walker.hidden(false);
    walker.git_ignore(!skip_gitignore);
    walker.git_exclude(!skip_gitignore);
    walker.git_global(!skip_gitignore);
    walker.ignore(!skip_gitignore);
    walker.require_git(false);
    walker.follow_links(false);
    walker.filter_entry(|entry| {
        !(entry.file_type().is_some_and(|ft| ft.is_dir()) && entry.file_name() == ".git")
    });
    walker
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn collect_files(root: &Path, skip_gitignore: bool) -> HashSet<String> {
        source_walker(root, skip_gitignore)
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_some_and(|ft| ft.is_file()))
            .map(|e| {
                e.path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect()
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
}
