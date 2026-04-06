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
