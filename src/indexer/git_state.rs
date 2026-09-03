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

pub(super) fn clean_git_checkout_state(root: &Path) -> Option<String> {
    git_worktree_is_clean(root).then(|| git_checkout_state(root))?
}

pub(super) fn indexed_git_state_path(workspace: &Workspace) -> PathBuf {
    workspace.index_dir.join("indexed_git_state")
}

pub(super) fn record_indexed_git_state(
    workspace: &Workspace,
    expected_state: Option<&str>,
) -> bool {
    let current_state = clean_git_checkout_state(&workspace.root);
    if current_state.as_deref() == expected_state
        && let Some(state) = current_state
        && fs::write(indexed_git_state_path(workspace), state).is_ok()
    {
        return true;
    }
    let _ = fs::remove_file(indexed_git_state_path(workspace));
    false
}

pub(super) fn refresh_clean_base_metadata(workspace: &Workspace) -> Result<bool> {
    if super::validate_existing_index_storage(workspace).is_err() {
        return Ok(false);
    }
    match base_index_checkout_state(workspace) {
        BaseIndexCheckoutState::Current => return Ok(true),
        BaseIndexCheckoutState::Stale => return Ok(false),
        BaseIndexCheckoutState::MetadataChanged => {}
    }

    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(workspace.lock_path())?;
    fs2::FileExt::lock_exclusive(&lock_file)?;

    match base_index_checkout_state(workspace) {
        BaseIndexCheckoutState::Current => Ok(true),
        BaseIndexCheckoutState::Stale => Ok(false),
        BaseIndexCheckoutState::MetadataChanged => {
            let skip_gitignore = workspace
                .read_metadata()?
                .is_some_and(|metadata| metadata.skip_gitignore);
            let expected_state = clean_git_checkout_state(&workspace.root);
            MerkleSnapshot::build(&workspace.root, skip_gitignore)?
                .save(&workspace.merkle_snapshot_path())?;
            Ok(record_indexed_git_state(
                workspace,
                expected_state.as_deref(),
            ))
        }
    }
}

fn git_head(root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|head| !head.is_empty())
}

fn git_index_hash(root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--git-path", "index"])
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
    let path = PathBuf::from(raw);
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    let bytes = fs::read(path).ok()?;
    Some(hex::encode(
        xxhash_rust::xxh3::xxh3_128(&bytes).to_le_bytes(),
    ))
}

fn git_worktree_is_clean(root: &Path) -> bool {
    std::process::Command::new("git")
        .args(["status", "--porcelain=v1", "--untracked-files=normal"])
        .current_dir(root)
        .output()
        .is_ok_and(|output| output.status.success() && output.stdout.is_empty())
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
    let path = PathBuf::from(raw);
    Some(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
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

fn git_ignore_state(root: &Path) -> Option<String> {
    // The bool marks controls whose whitelist rules need a source walk because
    // Git does not necessarily track the files they include.
    let mut controls = BTreeMap::<PathBuf, bool>::new();
    for path in tracked_ignore_controls(root)? {
        controls.insert(path, true);
    }
    let configured_global_ignore =
        git_path(root, &["config", "--path", "--get", "core.excludesFile"]);
    let default_global_ignore = configured_global_ignore.is_none().then(|| {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
            .map(|config| config.join("git/ignore"))
    });
    for path in [
        git_path(root, &["rev-parse", "--git-path", "info/exclude"]),
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

    let ignored_controls = std::process::Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
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
        .filter(|path| !path.is_empty())
    {
        let path = root.join(std::str::from_utf8(raw_path).ok()?);
        let independent = path.file_name().is_some_and(|name| name == ".ignore");
        controls.entry(path).or_insert(independent);
    }

    let mut state = b"walker-inputs-v2\0".to_vec();
    for (path, independent) in controls {
        state.extend_from_slice(path.to_string_lossy().as_bytes());
        state.push(0);
        let contents = match fs::read(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                state.push(0);
                continue;
            }
            Err(_) => return None,
        };
        if independent {
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

fn git_checkout_state(root: &Path) -> Option<String> {
    Some(format!(
        "{}\n{}\n{}\n{}",
        git_head(root)?,
        git_index_hash(root)?,
        git_sparse_checkout_state(root),
        git_ignore_state(root)?,
    ))
}

fn git_sparse_checkout_state(root: &Path) -> String {
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

    let cone = std::process::Command::new("git")
        .args(["config", "--bool", "core.sparseCheckoutCone"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| output.stdout)
        .unwrap_or_default();
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
    if indexes_ignored_files
        || !workspace.quick_index_health().is_queryable()
        || !git_worktree_is_clean(&workspace.root)
    {
        return BaseIndexCheckoutState::Stale;
    }
    let Some(current_state) = git_checkout_state(&workspace.root) else {
        return BaseIndexCheckoutState::Stale;
    };
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
