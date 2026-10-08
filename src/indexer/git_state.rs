use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::merkle::{MerkleSnapshot, normalized_indexable_content};
use crate::workspace::Workspace;

pub(super) fn files_have_same_contents(left: &Path, right: &Path) -> bool {
    match (fs::read(left), fs::read(right)) {
        (Ok(left_bytes), Ok(right_bytes)) => {
            left_bytes == right_bytes
                || normalized_indexable_content(left, &left_bytes)
                    == normalized_indexable_content(right, &right_bytes)
        }
        _ => false,
    }
}

/// The state of a clean checkout, with the inputs that a later check reads
/// again.
pub(super) struct CleanGitCheckout {
    state: String,
    head: String,
    index: PathBuf,
    index_hash: String,
    ignore_controls: IgnoreControls,
    ignore_state: String,
}

impl CleanGitCheckout {
    /// `None` if the worktree has changes or a part of the state is unknown.
    pub(super) fn capture(root: &Path) -> Option<Self> {
        let head = clean_git_head(root)?;
        let paths = git_paths(root)?;
        let index_hash = file_hash(&paths.index)?;
        let config = git_core_config(root);
        let ignore_controls = git_ignore_controls(root, &paths, config.as_ref())?;
        let ignore_state = ignore_controls_state(&ignore_controls)?;
        let state = format!(
            "{head}\n{index_hash}\n{}\n{ignore_state}",
            git_sparse_checkout_state(root, config.as_ref()),
        );
        Some(Self {
            state,
            head,
            index: paths.index,
            index_hash,
            ignore_controls,
            ignore_state,
        })
    }

    pub(super) fn state(&self) -> &str {
        &self.state
    }

    /// Whether the checkout is still clean at the same commit, with the same
    /// Git index and the same ignore file contents. An edit, a commit, a
    /// checkout, and a staging operation each change one of them. This check
    /// starts one Git process. It does not read the Git configuration or list
    /// the ignore files again.
    pub(super) fn is_unchanged(&self, root: &Path) -> bool {
        clean_git_head(root).as_deref() == Some(self.head.as_str())
            && file_hash(&self.index).as_deref() == Some(self.index_hash.as_str())
            && ignore_controls_state(&self.ignore_controls).as_deref()
                == Some(self.ignore_state.as_str())
    }
}

pub(super) fn indexed_git_state_path(workspace: &Workspace) -> PathBuf {
    workspace.index_dir.join("indexed_git_state")
}

/// Records the state that `expected` captured before indexing, if the
/// checkout did not change while the index was built.
pub(super) fn record_indexed_git_state(
    workspace: &Workspace,
    expected: Option<&CleanGitCheckout>,
) -> bool {
    if let Some(checkout) = expected
        && checkout.is_unchanged(&workspace.root)
        && fs::write(indexed_git_state_path(workspace), checkout.state()).is_ok()
    {
        return true;
    }
    let _ = fs::remove_file(indexed_git_state_path(workspace));
    false
}

pub(super) fn refresh_clean_base_metadata(workspace: &Workspace) -> Result<bool> {
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(workspace.lock_path())?;
    fs2::FileExt::lock_exclusive(&lock_file)?;
    if super::validate_existing_index_storage(workspace).is_err() {
        return Ok(false);
    }

    match base_index_checkout_state(workspace) {
        BaseIndexCheckoutState::Current => Ok(true),
        BaseIndexCheckoutState::Stale => Ok(false),
        BaseIndexCheckoutState::MetadataChanged => {
            let skip_gitignore = workspace
                .read_metadata()?
                .is_some_and(|metadata| metadata.skip_gitignore);
            let expected = CleanGitCheckout::capture(&workspace.root);
            MerkleSnapshot::build(&workspace.root, skip_gitignore)?
                .save(&workspace.merkle_snapshot_path())?;
            Ok(record_indexed_git_state(workspace, expected.as_ref()))
        }
    }
}

/// The commit that `HEAD` names, if the worktree has no change against it.
fn clean_git_head(root: &Path) -> Option<String> {
    // The walker indexes submodule sources, so submodule ignore settings must
    // not hide their edits. The distance to the upstream branch is not needed.
    let output = std::process::Command::new("git")
        .args([
            "status",
            "--porcelain=v2",
            "--branch",
            "--no-ahead-behind",
            "--untracked-files=normal",
            "--ignore-submodules=none",
        ])
        // `git status` otherwise rewrites the index of the repository to
        // refresh its cached file data, and holds `index.lock` meanwhile.
        .env("GIT_OPTIONAL_LOCKS", "0")
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_clean_head(&output.stdout)
}

/// Header lines start with `# `. Every other line is a changed or untracked
/// path. A repository without a commit has the object name `(initial)`.
fn parse_clean_head(status: &[u8]) -> Option<String> {
    let mut head = None;
    for line in status.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        let header = line.strip_prefix(b"# ")?;
        if let Some(object) = header.strip_prefix(b"branch.oid ") {
            head = Some(std::str::from_utf8(object).ok()?.to_string());
        }
    }
    head.filter(|object| object != "(initial)")
}

struct GitPaths {
    index: PathBuf,
    info_exclude: PathBuf,
}

fn git_paths(root: &Path) -> Option<GitPaths> {
    let output = std::process::Command::new("git")
        .args([
            "rev-parse",
            "--git-path",
            "index",
            "--git-path",
            "info/exclude",
        ])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut paths = text.lines().map(|line| root.join(line.trim()));
    let (index, info_exclude) = (paths.next()?, paths.next()?);
    Some(GitPaths {
        index,
        info_exclude,
    })
}

fn file_hash(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(hex::encode(
        xxhash_rust::xxh3::xxh3_128(&bytes).to_le_bytes(),
    ))
}

fn git_path(root: &Path, args: &[&str]) -> Option<PathBuf> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }
    Some(root.join(raw))
}

/// `core.excludesFile` and the sparse-checkout switches.
#[derive(Debug, Default, PartialEq)]
struct GitCoreConfig {
    excludes_file: Option<PathBuf>,
    sparse_checkout: bool,
    sparse_cone: Option<bool>,
}

/// Reads the three settings with one Git process. `None` if Git cannot print
/// them in this form, for example for a Boolean without a value. The callers
/// then ask for each setting with its own command.
fn git_core_config(root: &Path) -> Option<GitCoreConfig> {
    let output = std::process::Command::new("git")
        .args([
            "config",
            "--path",
            "-z",
            "--get-regexp",
            r"^core\.(excludesfile|sparsecheckout|sparsecheckoutcone)$",
        ])
        .current_dir(root)
        .output()
        .ok()?;
    match output.status.code() {
        Some(0) => parse_core_config(root, &output.stdout),
        // No setting matches.
        Some(1) => Some(GitCoreConfig::default()),
        _ => None,
    }
}

/// Entries are `key\nvalue\0`. A later entry replaces an earlier one, as in
/// `git config --get`.
fn parse_core_config(root: &Path, entries: &[u8]) -> Option<GitCoreConfig> {
    let mut config = GitCoreConfig::default();
    for entry in entries
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let (key, value) = std::str::from_utf8(entry).ok()?.split_once('\n')?;
        match key {
            "core.excludesfile" => {
                config.excludes_file = (!value.is_empty()).then(|| root.join(value));
            }
            "core.sparsecheckout" => config.sparse_checkout = parse_git_bool(value)?,
            "core.sparsecheckoutcone" => config.sparse_cone = Some(parse_git_bool(value)?),
            _ => {}
        }
    }
    Some(config)
}

fn parse_git_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" => Some(true),
        "false" | "no" | "off" | "" => Some(false),
        number => number.parse::<i64>().ok().map(|number| number != 0),
    }
}

/// Git status cannot observe edits hidden by index flags. Collect tracked
/// .ignore files here too: their whitelist rules can include Git-ignored code.
fn tracked_ignore_controls(root: &Path) -> Option<Vec<PathBuf>> {
    let output = std::process::Command::new("git")
        .args(["ls-files", "-z", "-v", "--cached"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let mut controls = Vec::new();
    for record in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let tag = *record.first()?;
        let relative = Path::new(std::str::from_utf8(record.get(2..)?).ok()?);
        if tag.is_ascii_lowercase() || (tag == b'S' && root.join(relative).try_exists().ok()?) {
            return None;
        }
        if relative.file_name().is_some_and(|name| name == ".ignore") {
            controls.push(root.join(relative));
        }
    }
    Some(controls)
}

/// Ignore files that the walker can read. The bool marks controls whose
/// whitelist rules need a source walk because Git does not necessarily track
/// the files they include.
type IgnoreControls = BTreeMap<PathBuf, bool>;

fn git_ignore_controls(
    root: &Path,
    paths: &GitPaths,
    config: Option<&GitCoreConfig>,
) -> Option<IgnoreControls> {
    let mut controls = IgnoreControls::new();
    for path in tracked_ignore_controls(root)? {
        controls.insert(path, true);
    }
    let configured_global_ignore = match config {
        Some(config) => config.excludes_file.clone(),
        None => git_path(root, &["config", "--path", "--get", "core.excludesFile"]),
    };
    let default_global_ignore = configured_global_ignore.is_none().then(|| {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
            .map(|config| config.join("git/ignore"))
    });
    for path in [
        Some(paths.info_exclude.clone()),
        configured_global_ignore,
        default_global_ignore.flatten(),
    ]
    .into_iter()
    .flatten()
    {
        controls.entry(path).or_insert(false);
    }
    for directory in root.ancestors() {
        controls.insert(directory.join(".ignore"), true);
        controls.insert(directory.join(".gitignore"), directory != root);
    }

    // `--directory` keeps Git out of ignored directories. The walker does not
    // enter them either, so their ignore files cannot change what is indexed.
    let ignored_controls = std::process::Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
            "--",
            ":(glob)**/.gitignore",
            ":(glob)**/.ignore",
        ])
        .current_dir(root)
        .output()
        .ok()?;
    if !ignored_controls.status.success() {
        return None;
    }
    for raw_path in ignored_controls
        .stdout
        .split(|byte| *byte == 0)
        // Git names an ignored directory or a nested repository with a
        // trailing slash. Neither is an ignore file.
        .filter(|path| !path.is_empty() && !path.ends_with(b"/"))
    {
        let path = root.join(std::str::from_utf8(raw_path).ok()?);
        let independent = path.file_name().is_some_and(|name| name == ".ignore");
        controls.entry(path).or_insert(independent);
    }
    Some(controls)
}

/// Hashes the contents of the ignore files. `None` if a file cannot be read
/// or an independent control has whitelist rules.
fn ignore_controls_state(controls: &IgnoreControls) -> Option<String> {
    let mut state = b"walker-inputs-v2\0".to_vec();
    for (path, independent) in controls {
        state.extend_from_slice(path.to_string_lossy().as_bytes());
        state.push(0);
        let contents = match fs::read(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                state.push(0);
                continue;
            }
            Err(_) => return None,
        };
        if *independent {
            let mut builder = ignore::gitignore::GitignoreBuilder::new(path.parent()?);
            let text = std::str::from_utf8(&contents).ok()?;
            for line in text.trim_start_matches('\u{feff}').lines() {
                builder.add_line(None, line).ok()?;
            }
            if builder.build().ok()?.num_whitelists() != 0 {
                return None;
            }
        }
        state.push(1);
        state.extend_from_slice(&contents);
        state.push(0);
    }
    Some(hex::encode(
        xxhash_rust::xxh3::xxh3_128(&state).to_le_bytes(),
    ))
}

fn git_sparse_checkout_state(root: &Path, config: Option<&GitCoreConfig>) -> String {
    // `git sparse-checkout list` fails when `core.sparseCheckout` is off.
    if config.is_some_and(|config| !config.sparse_checkout) {
        return "disabled".to_string();
    }
    let list = std::process::Command::new("git")
        .args(["sparse-checkout", "list"])
        .current_dir(root)
        .output();
    let Ok(list) = list else {
        return "disabled".to_string();
    };
    if !list.status.success() {
        return "disabled".to_string();
    }

    // The same bytes as the output of `git config --bool`.
    let cone = match config {
        Some(config) => match config.sparse_cone {
            Some(true) => b"true\n".to_vec(),
            Some(false) => b"false\n".to_vec(),
            None => Vec::new(),
        },
        None => std::process::Command::new("git")
            .args(["config", "--bool", "core.sparseCheckoutCone"])
            .current_dir(root)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| output.stdout)
            .unwrap_or_default(),
    };
    let mut state = list.stdout;
    state.extend_from_slice(&cone);
    format!(
        "enabled:{}",
        hex::encode(xxhash_rust::xxh3::xxh3_128(&state).to_le_bytes())
    )
}

enum BaseIndexCheckoutState {
    Current,
    MetadataChanged,
    Stale,
}

fn base_index_checkout_state(workspace: &Workspace) -> BaseIndexCheckoutState {
    let indexes_ignored_files = workspace
        .read_metadata()
        .ok()
        .flatten()
        .is_some_and(|metadata| metadata.skip_gitignore);
    if indexes_ignored_files || !workspace.quick_index_health().is_queryable() {
        return BaseIndexCheckoutState::Stale;
    }
    let Some(checkout) = CleanGitCheckout::capture(&workspace.root) else {
        return BaseIndexCheckoutState::Stale;
    };
    let current_state = checkout.state();
    let Some(indexed_state) = fs::read_to_string(indexed_git_state_path(workspace)).ok() else {
        return BaseIndexCheckoutState::Stale;
    };
    if indexed_state == current_state {
        return BaseIndexCheckoutState::Current;
    }

    let same_head = indexed_state.lines().next() == current_state.lines().next();
    let same_sparse_checkout = indexed_state.lines().nth(2) == current_state.lines().nth(2);
    let same_ignore_state = indexed_state.lines().nth(3) == current_state.lines().nth(3);
    if same_head && same_sparse_checkout && same_ignore_state {
        BaseIndexCheckoutState::MetadataChanged
    } else {
        BaseIndexCheckoutState::Stale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_head_needs_a_commit_and_no_changed_path() {
        let headers = "# branch.oid 6e5b717bb2e8932f2abfcdb298891feb29673848\n# branch.head main\n";
        assert_eq!(
            parse_clean_head(headers.as_bytes()).as_deref(),
            Some("6e5b717bb2e8932f2abfcdb298891feb29673848")
        );
        for change in [
            "1 .M N... 100644 100644 100644 1111111 1111111 src/lib.rs\n",
            "? untracked.rs\n",
        ] {
            assert_eq!(
                parse_clean_head(format!("{headers}{change}").as_bytes()),
                None
            );
        }
        assert_eq!(
            parse_clean_head(b"# branch.oid (initial)\n# branch.head main\n"),
            None
        );
        assert_eq!(parse_clean_head(b"# branch.head main\n"), None);
    }

    #[test]
    fn core_config_reads_the_last_value_of_each_setting() {
        let root = Path::new("/workspace");
        let entries = b"core.sparsecheckout\nfalse\0core.excludesfile\n/home/user/ignore\0\
            core.sparsecheckout\nYes\0core.sparsecheckoutcone\n0\0core.excludesfile\nrelative/ignore\0";
        assert_eq!(
            parse_core_config(root, entries),
            Some(GitCoreConfig {
                excludes_file: Some(PathBuf::from("/workspace/relative/ignore")),
                sparse_checkout: true,
                sparse_cone: Some(false),
            })
        );
        assert_eq!(parse_core_config(root, b""), Some(GitCoreConfig::default()));
        // Git prints a key without a value differently. The caller then asks
        // for each setting with its own command.
        assert_eq!(parse_core_config(root, b"core.sparsecheckout\0"), None);
        assert_eq!(
            parse_core_config(root, b"core.sparsecheckout\nmaybe\0"),
            None
        );
    }
}
