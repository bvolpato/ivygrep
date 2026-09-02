use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::{Serialize, Serializer};

use crate::chunking::{language_for_path, resolve_type_alias};
use crate::path_glob::PathGlobMatcher;
use crate::search::SearchOptions;
use crate::workspace::Workspace;

const MAX_SERIALIZED_CHANGES: usize = 512;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextChangeSource {
    Since,
    Staged,
    Worktree,
    Untracked,
}

impl ContextChangeSource {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Since => "since",
            Self::Staged => "staged",
            Self::Worktree => "worktree",
            Self::Untracked => "untracked",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Unmerged,
    Unknown,
}

impl ContextChangeStatus {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Modified => "modified",
            Self::Deleted => "deleted",
            Self::Renamed => "renamed",
            Self::Copied => "copied",
            Self::TypeChanged => "type changed",
            Self::Unmerged => "unmerged",
            Self::Unknown => "changed",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct ContextChange {
    #[serde(serialize_with = "serialize_index_path")]
    pub file_path: PathBuf,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_index_path"
    )]
    pub old_path: Option<PathBuf>,
    pub status: ContextChangeStatus,
    pub sources: Vec<ContextChangeSource>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct ContextChangeScope {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_commit: Option<String>,
    pub dirty_worktree: bool,
    pub total_changes: usize,
    pub changes_truncated: bool,
    pub changes: Vec<ContextChange>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub struct ContextInputPath {
    #[serde(serialize_with = "serialize_index_path")]
    pub file_path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

pub(crate) fn serialize_index_path<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&crate::workspace::index_path_string(path))
}

fn serialize_optional_index_path<S>(
    path: &Option<PathBuf>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match path {
        Some(path) => serializer.serialize_some(&crate::workspace::index_path_string(path)),
        None => serializer.serialize_none(),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ContextSeed {
    pub file_path: PathBuf,
    pub line: Option<usize>,
    pub git_revision: Option<String>,
    pub reason: String,
    pub source: String,
    pub priority: u8,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ContextInput {
    pub change_scope: Option<ContextChangeScope>,
    pub referenced_paths: Vec<ContextInputPath>,
    pub seeds: Vec<ContextSeed>,
}

pub(crate) fn collect_context_input(
    workspace: &Workspace,
    task: &str,
    since: Option<&str>,
    options: &SearchOptions,
) -> Result<ContextInput> {
    let matcher = PathGlobMatcher::new(&options.include_globs, &options.exclude_globs)?;
    let path_allowed = |path: &Path| {
        matcher.matches(path)
            && options
                .scope_filter
                .as_ref()
                .is_none_or(|scope| scope.matches(path))
            && options.type_filter.as_deref().is_none_or(|filter| {
                let expected = resolve_type_alias(filter).unwrap_or(filter);
                language_for_path(path) == Some(expected)
            })
    };

    let (mut change_scope, seed_changes) = match collect_git_changes(&workspace.root, since)? {
        Some((scope, changes)) => (Some(scope), changes),
        None => (None, Vec::new()),
    };
    let seed_changes = seed_changes
        .into_iter()
        .filter(|change| path_allowed(&change.file_path))
        .collect::<Vec<_>>();
    if let Some(scope) = &mut change_scope {
        scope.dirty_worktree = seed_changes.iter().any(|change| {
            change.sources.iter().any(|source| {
                matches!(
                    source,
                    ContextChangeSource::Staged
                        | ContextChangeSource::Worktree
                        | ContextChangeSource::Untracked
                )
            })
        });
        scope.total_changes = seed_changes.len();
        scope.changes = seed_changes
            .iter()
            .take(MAX_SERIALIZED_CHANGES)
            .cloned()
            .collect();
        scope.changes_truncated = scope.total_changes > scope.changes.len();
    }
    let referenced_paths = referenced_task_paths(&workspace.root, task, options.skip_gitignore)
        .into_iter()
        .filter(|reference| {
            path_allowed(&reference.file_path)
                && (options.skip_gitignore
                    || !path_is_git_ignored(&workspace.root, &reference.file_path))
        })
        .collect::<Vec<_>>();
    let mut seeds = BTreeMap::<PathBuf, ContextSeed>::new();
    for reference in &referenced_paths {
        if !workspace.root.join(&reference.file_path).is_file() {
            continue;
        }
        seeds.insert(
            reference.file_path.clone(),
            ContextSeed {
                file_path: reference.file_path.clone(),
                line: reference.line,
                git_revision: None,
                reason: reference.line.map_or_else(
                    || "mentioned in task input".to_string(),
                    |line| format!("mentioned at line {line} in task or stack trace"),
                ),
                source: "task_input".to_string(),
                priority: 3,
            },
        );
    }
    let base_commit = change_scope
        .as_ref()
        .and_then(|scope| scope.base_commit.clone());
    for change in &seed_changes {
        let current_file = workspace.root.join(&change.file_path).is_file();
        let deleted_file = change.status == ContextChangeStatus::Deleted;
        if !current_file && !deleted_file {
            continue;
        }
        let sources = change
            .sources
            .iter()
            .map(|source| source.label())
            .collect::<Vec<_>>()
            .join(", ");
        seeds
            .entry(change.file_path.clone())
            .or_insert_with(|| ContextSeed {
                file_path: change.file_path.clone(),
                line: None,
                git_revision: deleted_file.then(|| {
                    if change.sources.iter().any(|source| {
                        matches!(
                            source,
                            ContextChangeSource::Staged | ContextChangeSource::Worktree
                        )
                    }) {
                        "HEAD".to_string()
                    } else {
                        base_commit.clone().unwrap_or_else(|| "HEAD".to_string())
                    }
                }),
                reason: format!("{} in {sources}", change.status.label()),
                source: if deleted_file {
                    "git_deleted"
                } else if change.sources.contains(&ContextChangeSource::Since) {
                    "git_since"
                } else {
                    "git_worktree"
                }
                .to_string(),
                priority: 2,
            });
    }
    let mut seeds = seeds.into_values().collect::<Vec<_>>();
    let task_terms = task
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .flat_map(crate::text::split_identifier_segments)
        .filter(|term| term.len() >= 3)
        .collect::<BTreeSet<_>>();
    seeds.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| {
                seed_path_relevance(&right.file_path, &task_terms)
                    .cmp(&seed_path_relevance(&left.file_path, &task_terms))
            })
            .then_with(|| left.file_path.cmp(&right.file_path))
    });

    Ok(ContextInput {
        change_scope,
        referenced_paths,
        seeds,
    })
}

pub(crate) fn path_is_git_ignored(root: &Path, path: &Path) -> bool {
    Command::new("git")
        .args(["check-ignore", "-q", "--"])
        .arg(path)
        .current_dir(root)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn seed_path_relevance(path: &Path, task_terms: &BTreeSet<String>) -> usize {
    path.to_string_lossy()
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .flat_map(crate::text::split_identifier_segments)
        .filter(|term| task_terms.contains(term))
        .map(|term| term.len())
        .sum()
}

fn collect_git_changes(
    root: &Path,
    since: Option<&str>,
) -> Result<Option<(ContextChangeScope, Vec<ContextChange>)>> {
    let Some(repo_root) = git_output(root, &["rev-parse", "--show-toplevel"], false)? else {
        if since.is_some() {
            bail!("--since requires a Git workspace");
        }
        return Ok(None);
    };
    let repo_root = PathBuf::from(repo_root.trim());
    let workspace_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let repo_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let workspace_prefix = workspace_root
        .strip_prefix(&repo_root)
        .unwrap_or_else(|_| Path::new(""));
    let mut changes = BTreeMap::<PathBuf, ContextChange>::new();
    let mut base_commit = None;

    if let Some(reference) = since {
        validate_git_reference(reference)?;
        let verified = git_output(
            root,
            &["rev-parse", "--verify", &format!("{reference}^{{commit}}")],
            true,
        )?
        .context("Git did not resolve --since reference")?;
        let merge_base = git_output(root, &["merge-base", verified.trim(), "HEAD"], true)?
            .context("Git could not find merge base for --since reference")?;
        base_commit = Some(merge_base.trim().to_string());
        merge_name_status(
            &mut changes,
            &git_bytes_scoped(
                root,
                &[
                    "diff",
                    "--name-status",
                    "-z",
                    "--find-renames",
                    merge_base.trim(),
                    "HEAD",
                ],
                workspace_prefix,
            )?,
            ContextChangeSource::Since,
            workspace_prefix,
        );
    }

    let worktree_status = git_bytes_scoped(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        workspace_prefix,
    )?;
    merge_porcelain_status(&mut changes, &worktree_status, workspace_prefix);

    let dirty_worktree = changes.values().any(|change| {
        change.sources.iter().any(|source| {
            matches!(
                source,
                ContextChangeSource::Staged
                    | ContextChangeSource::Worktree
                    | ContextChangeSource::Untracked
            )
        })
    });
    let changes = changes.into_values().collect::<Vec<_>>();
    let total_changes = changes.len();
    let serialized_changes = changes
        .iter()
        .take(MAX_SERIALIZED_CHANGES)
        .cloned()
        .collect::<Vec<_>>();
    Ok(Some((
        ContextChangeScope {
            since: since.map(ToString::to_string),
            base_commit,
            dirty_worktree,
            total_changes,
            changes_truncated: total_changes > serialized_changes.len(),
            changes: serialized_changes,
        },
        changes,
    )))
}

fn validate_git_reference(reference: &str) -> Result<()> {
    if reference.is_empty()
        || reference.starts_with('-')
        || reference.contains("..")
        || reference.contains("@{")
        || !reference
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "/._-".contains(character))
    {
        bail!("invalid --since Git reference: {reference:?}");
    }
    Ok(())
}

fn git_output(root: &Path, args: &[&str], required: bool) -> Result<Option<String>> {
    let output = match Command::new("git").args(args).current_dir(root).output() {
        Ok(output) => output,
        Err(error) if !required && error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if output.status.success() {
        return Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()));
    }
    if required {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(None)
}

fn git_bytes_scoped(root: &Path, args: &[&str], workspace_prefix: &Path) -> Result<Vec<u8>> {
    let mut command = Command::new("git");
    command.args(args).arg("--");
    if !workspace_prefix.as_os_str().is_empty() {
        command.arg(workspace_prefix);
    }
    let output = command.current_dir(root).output()?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}

fn merge_name_status(
    changes: &mut BTreeMap<PathBuf, ContextChange>,
    bytes: &[u8],
    source: ContextChangeSource,
    workspace_prefix: &Path,
) {
    let fields = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();
    let mut index = 0;
    while index < fields.len() {
        let status = String::from_utf8_lossy(fields[index]);
        index += 1;
        let code = status.as_bytes().first().copied().unwrap_or(b'?');
        let needs_two_paths = matches!(code, b'R' | b'C');
        if index >= fields.len() || (needs_two_paths && index + 1 >= fields.len()) {
            break;
        }
        let old_path = needs_two_paths
            .then(|| workspace_relative_path(fields[index], workspace_prefix))
            .flatten();
        if needs_two_paths {
            index += 1;
        }
        let path = workspace_relative_path(fields[index], workspace_prefix);
        index += 1;
        let Some(path) = path else {
            continue;
        };
        let status = match code {
            b'A' => ContextChangeStatus::Added,
            b'M' => ContextChangeStatus::Modified,
            b'D' => ContextChangeStatus::Deleted,
            b'R' => ContextChangeStatus::Renamed,
            b'C' => ContextChangeStatus::Copied,
            b'T' => ContextChangeStatus::TypeChanged,
            b'U' => ContextChangeStatus::Unmerged,
            _ => ContextChangeStatus::Unknown,
        };
        merge_change(changes, path, old_path, status, source);
    }
}

fn merge_porcelain_status(
    changes: &mut BTreeMap<PathBuf, ContextChange>,
    bytes: &[u8],
    workspace_prefix: &Path,
) {
    let fields = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();
    let mut index = 0;
    while index < fields.len() {
        let entry = fields[index];
        index += 1;
        if entry.len() < 4 || entry[2] != b' ' {
            continue;
        }
        let (index_status, worktree_status) = (entry[0], entry[1]);
        let renamed = matches!(index_status, b'R' | b'C') || matches!(worktree_status, b'R' | b'C');
        let old_path = if renamed && index < fields.len() {
            let path = workspace_relative_path(fields[index], workspace_prefix);
            index += 1;
            path
        } else {
            None
        };
        let Some(path) = workspace_relative_path(&entry[3..], workspace_prefix) else {
            continue;
        };
        if index_status == b'?' && worktree_status == b'?' {
            merge_change(
                changes,
                path,
                None,
                ContextChangeStatus::Added,
                ContextChangeSource::Untracked,
            );
            continue;
        }
        if matches!(
            (index_status, worktree_status),
            (b'D', b'D')
                | (b'A', b'U')
                | (b'U', b'D')
                | (b'U', b'A')
                | (b'D', b'U')
                | (b'A', b'A')
                | (b'U', b'U')
        ) {
            for source in [ContextChangeSource::Staged, ContextChangeSource::Worktree] {
                merge_change(
                    changes,
                    path.clone(),
                    old_path.clone(),
                    ContextChangeStatus::Unmerged,
                    source,
                );
            }
            continue;
        }
        for (status, source) in [
            (index_status, ContextChangeSource::Staged),
            (worktree_status, ContextChangeSource::Worktree),
        ] {
            let status = match status {
                b' ' | b'?' | b'!' => continue,
                b'A' => ContextChangeStatus::Added,
                b'M' => ContextChangeStatus::Modified,
                b'D' => ContextChangeStatus::Deleted,
                b'R' => ContextChangeStatus::Renamed,
                b'C' => ContextChangeStatus::Copied,
                b'T' => ContextChangeStatus::TypeChanged,
                b'U' => ContextChangeStatus::Unmerged,
                _ => ContextChangeStatus::Unknown,
            };
            merge_change(changes, path.clone(), old_path.clone(), status, source);
        }
    }
}

fn merge_change(
    changes: &mut BTreeMap<PathBuf, ContextChange>,
    path: PathBuf,
    old_path: Option<PathBuf>,
    status: ContextChangeStatus,
    source: ContextChangeSource,
) {
    let change = changes.entry(path.clone()).or_insert(ContextChange {
        file_path: path,
        old_path: None,
        status,
        sources: Vec::new(),
    });
    if change.old_path.is_none() {
        change.old_path = old_path;
    }
    if !matches!(
        change.status,
        ContextChangeStatus::Added | ContextChangeStatus::Renamed
    ) || !matches!(status, ContextChangeStatus::Modified)
    {
        change.status = status;
    }
    if !change.sources.contains(&source) {
        change.sources.push(source);
        change.sources.sort();
    }
}

fn workspace_relative_path(raw: &[u8], workspace_prefix: &Path) -> Option<PathBuf> {
    let repo_path = normalize_relative(Path::new(std::str::from_utf8(raw).ok()?))?;
    repo_path
        .strip_prefix(workspace_prefix)
        .ok()
        .and_then(normalize_relative)
}

fn normalize_relative(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!normalized.as_os_str().is_empty())
        .then(|| PathBuf::from(crate::workspace::index_path_string(&normalized)))
}

fn referenced_task_paths(root: &Path, task: &str, skip_gitignore: bool) -> Vec<ContextInputPath> {
    static PYTHON_TRACE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"File [\"']([^\"']+)[\"'], line ([0-9]+)"#).expect("valid trace regex")
    });
    static PATH_WITH_LINE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)([A-Za-z0-9_./\\@+:-]+\.[A-Za-z0-9_+-]+):(?:line\s+)?([0-9]+)(?::[0-9]+)?")
            .expect("valid path regex")
    });
    static PATH_WITH_HASH_LINE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)([A-Za-z0-9_./\\@+:-]+\.[A-Za-z0-9_+-]+)#L([0-9]+)")
            .expect("valid hash-line regex")
    });
    static PATH_WITH_PARENS_LINE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)([A-Za-z0-9_./\\@+:-]+\.[A-Za-z0-9_+-]+)\(([0-9]+)(?:,[0-9]+)?\)")
            .expect("valid parenthesized-line regex")
    });
    static BARE_PATH: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)([A-Za-z0-9_./\\@+:-]+\.[A-Za-z0-9_+-]+)").expect("valid path regex")
    });

    let mut paths = BTreeSet::new();
    let mut workspace_files = None;
    for captures in PYTHON_TRACE.captures_iter(task) {
        insert_task_path(
            &mut paths,
            root,
            captures.get(1).map(|value| value.as_str()).unwrap_or(""),
            captures
                .get(2)
                .and_then(|value| value.as_str().parse().ok()),
            skip_gitignore,
            &mut workspace_files,
        );
    }
    for pattern in [
        &*PATH_WITH_LINE,
        &*PATH_WITH_HASH_LINE,
        &*PATH_WITH_PARENS_LINE,
    ] {
        for captures in pattern.captures_iter(task) {
            insert_task_path(
                &mut paths,
                root,
                captures.get(1).map(|value| value.as_str()).unwrap_or(""),
                captures
                    .get(2)
                    .and_then(|value| value.as_str().parse().ok()),
                skip_gitignore,
                &mut workspace_files,
            );
        }
    }
    for captures in BARE_PATH.captures_iter(task) {
        insert_task_path(
            &mut paths,
            root,
            captures.get(1).map(|value| value.as_str()).unwrap_or(""),
            None,
            skip_gitignore,
            &mut workspace_files,
        );
    }
    paths
        .into_iter()
        .map(|(file_path, line)| ContextInputPath { file_path, line })
        .collect()
}

fn insert_task_path(
    paths: &mut BTreeSet<(PathBuf, Option<usize>)>,
    root: &Path,
    raw_path: &str,
    line: Option<usize>,
    skip_gitignore: bool,
    workspace_files: &mut Option<Vec<PathBuf>>,
) {
    let line = line.filter(|line| *line > 0);
    let normalized_separators = raw_path.replace('\\', "/");
    let raw_path = Path::new(&normalized_separators);
    let has_windows_drive = normalized_separators.as_bytes().get(1) == Some(&b':')
        && normalized_separators
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic);
    let external_path = raw_path.is_absolute() || has_windows_drive;
    let relative = if raw_path.is_absolute() {
        raw_path
            .strip_prefix(root)
            .ok()
            .and_then(normalize_relative)
            .or_else(|| normalize_external_path(raw_path, false))
    } else if has_windows_drive {
        normalize_external_path(raw_path, true)
    } else {
        normalize_relative(raw_path)
    };
    let Some(mut relative) = relative else {
        return;
    };
    if !root.join(&relative).is_file() {
        let files = workspace_files.get_or_insert_with(|| {
            crate::walker::source_walker(root, skip_gitignore)
                .build()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
                .filter_map(|entry| entry.path().strip_prefix(root).ok().map(Path::to_path_buf))
                .collect()
        });
        let mut matches = files
            .iter()
            .filter(|candidate| {
                if external_path {
                    relative.ends_with(candidate.as_path())
                } else {
                    candidate.ends_with(&relative)
                }
            })
            .collect::<Vec<_>>();
        if external_path {
            matches.sort_by_key(|candidate| std::cmp::Reverse(candidate.components().count()));
        }
        let Some(candidate) = matches.first() else {
            return;
        };
        let best_depth = candidate.components().count();
        if matches
            .get(1)
            .is_some_and(|candidate| !external_path || candidate.components().count() == best_depth)
        {
            return;
        }
        relative = (*candidate).clone();
    }
    paths.retain(|(path, existing_line)| path != &relative || existing_line.is_some());
    if !paths.iter().any(|(path, existing_line)| {
        path == &relative && (existing_line == &line || existing_line.is_some())
    }) {
        paths.insert((relative, line));
    }
}

fn normalize_external_path(path: &Path, drop_first_component: bool) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    let mut normal_index = 0usize;
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                if drop_first_component && normal_index == 0 {
                    normal_index += 1;
                    continue;
                }
                normal_index += 1;
                normalized.push(value);
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
            Component::ParentDir => return None,
        }
    }
    (!normalized.as_os_str().is_empty())
        .then(|| PathBuf::from(crate::workspace::index_path_string(&normalized)))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn parses_stack_trace_paths_and_prefers_line_numbers() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(root.path().join("src/auth.rs"), "fn auth() {}\n").unwrap();
        fs::write(root.path().join("src/api.py"), "def api(): pass\n").unwrap();
        let task = "panic at src/auth.rs:42:7\n  File \"src/api.py\", line 9\nsrc/auth.rs";
        let paths = referenced_task_paths(root.path(), task, false);
        assert_eq!(
            paths,
            vec![
                ContextInputPath {
                    file_path: PathBuf::from("src/api.py"),
                    line: Some(9),
                },
                ContextInputPath {
                    file_path: PathBuf::from("src/auth.rs"),
                    line: Some(42),
                },
            ]
        );
    }

    #[test]
    fn resolves_common_stack_trace_and_issue_line_formats() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src/main/java/com/acme")).unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(
            root.path().join("src/main/java/com/acme/Auth.java"),
            "class Auth {}\n",
        )
        .unwrap();
        fs::write(root.path().join("src/session.cs"), "class Session {}\n").unwrap();
        fs::write(root.path().join("session.cs"), "class OtherSession {}\n").unwrap();
        fs::write(root.path().join("src/token.rs"), "fn token() {}\n").unwrap();

        let task = "at com.acme.Auth.refresh(Auth.java:42)\n\
                    at Service in C:\\agent\\work\\repo\\src\\session.cs:line 17\n\
                    issue points to src/token.rs#L9";
        assert_eq!(
            referenced_task_paths(root.path(), task, false),
            vec![
                ContextInputPath {
                    file_path: PathBuf::from("src/main/java/com/acme/Auth.java"),
                    line: Some(42),
                },
                ContextInputPath {
                    file_path: PathBuf::from("src/session.cs"),
                    line: Some(17),
                },
                ContextInputPath {
                    file_path: PathBuf::from("src/token.rs"),
                    line: Some(9),
                },
            ]
        );
    }

    #[test]
    fn stack_trace_paths_become_high_priority_seeds() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(root.path().join("src/auth.rs"), "fn auth() {}\n").unwrap();
        let workspace = Workspace::resolve(root.path()).unwrap();
        let input = collect_context_input(
            &workspace,
            "panic at src/auth.rs:1:4",
            None,
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(input.seeds.iter().any(|seed| {
            seed.file_path == Path::new("src/auth.rs")
                && seed.line == Some(1)
                && seed.source == "task_input"
        }));
    }

    #[test]
    fn merges_branch_and_dirty_changes() {
        let root = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .args(["-c", "commit.gpgsign=false"])
                .args(args)
                .current_dir(root.path())
                .status()
                .unwrap();
            assert!(status.success());
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "test@example.com"]);
        git(&["config", "user.name", "Test"]);
        fs::write(root.path().join("base.rs"), "fn base() {}\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "base"]);
        git(&["switch", "-qc", "feature"]);
        fs::write(root.path().join("committed.rs"), "fn committed() {}\n").unwrap();
        fs::write(root.path().join("temporary.rs"), "fn temporary() {}\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "feature"]);
        fs::remove_file(root.path().join("temporary.rs")).unwrap();
        fs::write(root.path().join("base.rs"), "fn changed() {}\n").unwrap();
        fs::write(root.path().join("staged.rs"), "fn staged() {}\n").unwrap();
        git(&["add", "staged.rs"]);
        fs::write(root.path().join("new.rs"), "fn new() {}\n").unwrap();

        let (scope, _) = collect_git_changes(root.path(), Some("main"))
            .unwrap()
            .unwrap();
        assert_eq!(scope.since.as_deref(), Some("main"));
        assert!(scope.dirty_worktree);
        assert_eq!(scope.total_changes, 5);
        assert!(scope.changes.iter().any(|change| {
            change.file_path == Path::new("committed.rs")
                && change.sources == vec![ContextChangeSource::Since]
        }));
        assert!(scope.changes.iter().any(|change| {
            change.file_path == Path::new("base.rs")
                && change.sources == vec![ContextChangeSource::Worktree]
        }));
        assert!(scope.changes.iter().any(|change| {
            change.file_path == Path::new("staged.rs")
                && change.sources == vec![ContextChangeSource::Staged]
        }));
        assert!(scope.changes.iter().any(|change| {
            change.file_path == Path::new("new.rs")
                && change.sources == vec![ContextChangeSource::Untracked]
        }));
        let workspace = Workspace::resolve(root.path()).unwrap();
        let input = collect_context_input(
            &workspace,
            "review this deletion",
            Some("main"),
            &SearchOptions::default(),
        )
        .unwrap();
        assert!(input.seeds.iter().any(|seed| {
            seed.file_path == Path::new("temporary.rs")
                && seed.git_revision.as_deref() == Some("HEAD")
                && seed.source == "git_deleted"
        }));

        let staged_only = collect_context_input(
            &workspace,
            "review staged change",
            Some("main"),
            &SearchOptions {
                scope_filter: Some(crate::workspace::WorkspaceScope {
                    rel_path: PathBuf::from("staged.rs"),
                    is_file: true,
                }),
                ..SearchOptions::default()
            },
        )
        .unwrap();
        let scope = staged_only.change_scope.unwrap();
        assert_eq!(scope.total_changes, 1);
        assert!(scope.dirty_worktree);
        assert_eq!(scope.changes[0].file_path, Path::new("staged.rs"));

        let committed_only = collect_context_input(
            &workspace,
            "review committed change",
            Some("main"),
            &SearchOptions {
                scope_filter: Some(crate::workspace::WorkspaceScope {
                    rel_path: PathBuf::from("committed.rs"),
                    is_file: true,
                }),
                ..SearchOptions::default()
            },
        )
        .unwrap();
        let scope = committed_only.change_scope.unwrap();
        assert_eq!(scope.total_changes, 1);
        assert!(!scope.dirty_worktree);
        assert_eq!(scope.changes[0].file_path, Path::new("committed.rs"));
    }

    #[test]
    fn parses_porcelain_renames_and_multi_stage_changes() {
        let mut changes = BTreeMap::new();
        merge_porcelain_status(
            &mut changes,
            b"R  moved.rs\0old.rs\0AM both.rs\0?? new.rs\0",
            Path::new(""),
        );
        let moved = changes.get(Path::new("moved.rs")).unwrap();
        assert_eq!(moved.old_path.as_deref(), Some(Path::new("old.rs")));
        assert_eq!(moved.status, ContextChangeStatus::Renamed);
        assert_eq!(moved.sources, vec![ContextChangeSource::Staged]);
        let both = changes.get(Path::new("both.rs")).unwrap();
        assert_eq!(both.status, ContextChangeStatus::Added);
        assert_eq!(
            both.sources,
            vec![ContextChangeSource::Staged, ContextChangeSource::Worktree]
        );
        assert_eq!(
            changes.get(Path::new("new.rs")).unwrap().sources,
            vec![ContextChangeSource::Untracked]
        );
    }

    #[test]
    fn rejects_option_like_git_references() {
        assert!(validate_git_reference("--output=/tmp/file").is_err());
        assert!(validate_git_reference("main").is_ok());
        assert!(validate_git_reference("origin/main").is_ok());
    }

    #[test]
    fn structured_paths_use_platform_neutral_separators() {
        let change = ContextChange {
            file_path: PathBuf::from("src").join("auth.rs"),
            old_path: Some(PathBuf::from("old").join("auth.rs")),
            status: ContextChangeStatus::Renamed,
            sources: vec![ContextChangeSource::Staged],
        };
        let value = serde_json::to_value(change).unwrap();
        assert_eq!(value["file_path"], "src/auth.rs");
        assert_eq!(value["old_path"], "old/auth.rs");

        let reference = ContextInputPath {
            file_path: PathBuf::from("tests").join("auth.rs"),
            line: Some(7),
        };
        let value = serde_json::to_value(reference).unwrap();
        assert_eq!(value["file_path"], "tests/auth.rs");
    }
}
