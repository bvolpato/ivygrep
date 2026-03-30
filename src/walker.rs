use std::path::Path;

use ignore::WalkBuilder;

/// Builds a standard file walker for the given root directory.
///
/// Configuration:
/// - Shows hidden files (but skips `.git/`)
/// - Respects `.gitignore`, `.git/info/exclude`, global gitignore, and `.ignore`
/// - Does not require a git repository
/// - Does not follow symlinks
pub fn source_walker(root: &Path) -> WalkBuilder {
    let mut walker = WalkBuilder::new(root);
    walker.hidden(false);
    walker.git_ignore(true);
    walker.git_exclude(true);
    walker.git_global(true);
    walker.ignore(true);
    walker.require_git(false);
    walker.follow_links(false);
    walker.filter_entry(|entry| {
        !(entry.file_type().is_some_and(|ft| ft.is_dir()) && entry.file_name() == ".git")
    });
    walker
}
