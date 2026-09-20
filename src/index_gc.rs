//! Garbage collection of indexes whose workspace root no longer exists.
//!
//! Agent worktrees are created and deleted all day, and each one leaves an
//! index directory behind. A pass removes an index only when all of this holds:
//!
//! - its root has been missing (not merely unreadable) for a grace period,
//!   recorded in the index directory so it survives daemon restarts;
//! - no live worktree overlay still reads it as its base;
//! - nobody holds its index or enhancement lock and no index or enhancement
//!   job is active;
//! - its root is still missing once those locks are held.
//!
//! A daemon takes part through `IndexHolder`: it serializes the removal with
//! its own requests and lets go of its watcher and open stores first. `ig --gc`
//! therefore asks a running daemon to make the pass.
//!
//! The default grace period is long because a missing root can be a detached
//! disk or a network mount. A linked worktree that its repository no longer
//! lists is gone for certain, so its overlay waits a much shorter time.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::jobs::{self, JobKind};
use crate::workspace::{Workspace, WorkspaceMetadata, root_is_gone};

/// Records when a pass first found the root missing.
const MISSING_SINCE_FILE: &str = ".root_missing_since";
/// Grace for the overlay of a linked worktree that `git worktree list` in its
/// repository no longer reports. Capped by the configured grace period.
const DELETED_WORKTREE_GRACE: Duration = Duration::from_secs(10 * 60);
const MIN_PASS_INTERVAL: Duration = Duration::from_secs(5);
const MAX_PASS_INTERVAL: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GcReport {
    /// Indexes removed by this pass.
    pub collected: Vec<CollectedIndex>,
    /// Indexes whose root is missing but whose grace period has not passed.
    pub waiting: Vec<WaitingIndex>,
    /// Indexes past their grace period that a job or a live overlay still uses.
    pub in_use: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectedIndex {
    pub root: PathBuf,
    pub index_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitingIndex {
    pub root: PathBuf,
    pub collect_after_unix: u64,
}

/// How often the daemon runs a pass: a quarter of the grace period, between
/// five seconds and ten minutes. `None` when collection is disabled.
pub(crate) fn pass_interval() -> Option<Duration> {
    crate::config::index_gc_grace()
        .map(|grace| (grace / 4).clamp(MIN_PASS_INTERVAL, MAX_PASS_INTERVAL))
}

/// What the process that runs a pass holds of an index. A daemon holds cached
/// readers, a watcher, and requests in flight; a CLI holds nothing.
pub trait IndexHolder {
    /// Keep every other user of the workspace in this process out until the
    /// returned guard drops. The daemon takes the exclusive mutation lease
    /// that `Remove` takes, so a search that is still reading the stores
    /// finishes first and none starts during the removal.
    fn reserve(&mut self, workspace: &Workspace) -> Box<dyn std::any::Any>;
    /// Let go of everything that keeps the stores open. Called once the index
    /// is certain to be removed.
    fn release(&mut self, workspace: &Workspace);
}

/// A process that keeps nothing open between calls.
pub struct NoIndexHolder;

impl IndexHolder for NoIndexHolder {
    fn reserve(&mut self, _workspace: &Workspace) -> Box<dyn std::any::Any> {
        Box::new(())
    }

    fn release(&mut self, _workspace: &Workspace) {}
}

/// Run one pass with the configured grace period.
pub fn collect_orphaned_indexes(holder: &mut dyn IndexHolder) -> Result<GcReport> {
    let Some(grace) = crate::config::index_gc_grace() else {
        return Ok(GcReport::default());
    };
    collect_orphaned_indexes_at(jobs::now_unix(), grace, holder)
}

fn collect_orphaned_indexes_at(
    now_unix: u64,
    grace: Duration,
    holder: &mut dyn IndexHolder,
) -> Result<GcReport> {
    // The daemon's periodic pass and one that `ig --gc` asked for must not
    // remove the same directory at once.
    static PASS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _pass = PASS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let entries = crate::workspace::list_workspace_metadata()?;
    // Base indexes that an overlay with a live root still reads.
    let live_bases = entries
        .iter()
        .filter(|(_, metadata)| !root_is_gone(&metadata.root))
        .filter_map(|(index_dir, _)| read_base_ref(index_dir))
        .map(|base_ref| base_ref.base_index_dir)
        .collect::<HashSet<_>>();
    let mut registered_worktrees = HashMap::new();
    let mut report = GcReport::default();

    for (index_dir, metadata) in entries {
        let marker = index_dir.join(MISSING_SINCE_FILE);
        if !root_is_gone(&metadata.root) {
            let _ = fs::remove_file(&marker);
            continue;
        }
        let missing_since = match read_missing_since(&marker) {
            Some(since) => since.min(now_unix),
            None => {
                let _ = fs::write(&marker, now_unix.to_string());
                now_unix
            }
        };
        let unregistered =
            worktree_is_unregistered(&index_dir, &metadata, &mut registered_worktrees);
        let effective_grace = if unregistered {
            grace.min(DELETED_WORKTREE_GRACE)
        } else {
            grace
        };
        let collect_after_unix = missing_since.saturating_add(effective_grace.as_secs());
        if now_unix < collect_after_unix {
            report.waiting.push(WaitingIndex {
                root: metadata.root,
                collect_after_unix,
            });
            continue;
        }
        if live_bases.contains(&index_dir) {
            report.in_use.push(index_dir);
            continue;
        }

        let workspace = Workspace::ledger_only(index_dir.clone(), &metadata);
        // Only the shorter grace made this index due: it rests on what Git
        // said a while ago, so Git is asked again under the locks.
        let due_as_unregistered = now_unix < missing_since.saturating_add(grace.as_secs());
        let still_gone = || {
            root_is_gone(&metadata.root)
                && !(due_as_unregistered
                    && !worktree_is_unregistered(&index_dir, &metadata, &mut HashMap::new()))
        };
        match remove_unused_index(&workspace, holder, still_gone)? {
            Removal::Removed => report.collected.push(CollectedIndex {
                root: metadata.root,
                index_dir,
            }),
            Removal::InUse => report.in_use.push(index_dir),
            // The next pass finds the root and clears the marker.
            Removal::RootReturned => {}
        }
    }
    Ok(report)
}

fn read_missing_since(marker: &Path) -> Option<u64> {
    fs::read_to_string(marker).ok()?.trim().parse().ok()
}

struct BaseRef {
    base_index_dir: PathBuf,
    base_workspace_root: PathBuf,
}

fn read_base_ref(index_dir: &Path) -> Option<BaseRef> {
    let raw = fs::read(index_dir.join("base_ref.json")).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    Some(BaseRef {
        base_index_dir: PathBuf::from(value.get("base_index_dir")?.as_str()?),
        base_workspace_root: PathBuf::from(value.get("base_workspace_root")?.as_str()?),
    })
}

/// Whether the index is the overlay of a linked worktree that its repository
/// no longer lists. `git worktree list` is authoritative: a worktree removed
/// with `git worktree remove` or pruned is gone for good, while one whose
/// directory is only missing (a detached disk) stays listed as prunable.
fn worktree_is_unregistered(
    index_dir: &Path,
    metadata: &WorkspaceMetadata,
    registered: &mut HashMap<PathBuf, Option<HashSet<String>>>,
) -> bool {
    let Some(base_ref) = read_base_ref(index_dir) else {
        return false;
    };
    let root = comparable_path(&metadata.root);
    registered
        .entry(base_ref.base_workspace_root.clone())
        .or_insert_with(|| registered_worktrees(&base_ref.base_workspace_root))
        .as_ref()
        .is_some_and(|worktrees| !worktrees.contains(&root))
}

/// A path in the form both sides of the worktree comparison share. Git prints
/// forward slashes and no verbatim prefix on Windows, and a worktree whose
/// directory is missing cannot be canonicalized, so the existing part of the
/// path is canonicalized and the rest appended. A listed worktree that fails
/// to match would lose the long grace period, so the form is forgiving.
fn comparable_path(path: &Path) -> String {
    let mut existing = path;
    let mut missing = Vec::new();
    let resolved = loop {
        if let Ok(resolved) = existing.canonicalize() {
            break resolved;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name.to_os_string());
                existing = parent;
            }
            _ => break path.to_path_buf(),
        }
    };
    let resolved = missing
        .iter()
        .rev()
        .fold(resolved, |resolved, name| resolved.join(name));
    let text = resolved.to_string_lossy().replace('\\', "/");
    let text = text.strip_prefix("//?/").unwrap_or(&text);
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text.to_string()
    }
}

fn registered_worktrees(base_root: &Path) -> Option<HashSet<String>> {
    if !base_root.is_dir() {
        return None;
    }
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain", "-z"])
        .current_dir(base_root)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    Some(
        output
            .stdout
            .split(|byte| *byte == b'\0')
            .filter_map(|field| field.strip_prefix(b"worktree "))
            .map(|path| comparable_path(Path::new(String::from_utf8_lossy(path).as_ref())))
            .collect(),
    )
}

enum Removal {
    Removed,
    /// A job, or a lock holder in another process, still uses the index.
    InUse,
    /// A checkout came back at the root while the pass was getting here.
    RootReturned,
}

/// Remove the index unless something uses it or its root is back.
fn remove_unused_index(
    workspace: &Workspace,
    holder: &mut dyn IndexHolder,
    still_gone: impl FnOnce() -> bool,
) -> Result<Removal> {
    // First in line: taking the daemon's lease can wait for a long search,
    // and nothing else should be held meanwhile.
    let _reservation = holder.reserve(workspace);
    let job_is_active = |kind, ttl| jobs::job_status(workspace, kind, ttl).active();
    if job_is_active(JobKind::Indexing, jobs::INDEXING_HEARTBEAT_TTL_SECS)
        || job_is_active(JobKind::Enhancement, jobs::ENHANCEMENT_HEARTBEAT_TTL_SECS)
    {
        return Ok(Removal::InUse);
    }
    // Index runs hold `index.lock`; vector writers hold `enhancement.lock`.
    let mut locks = Vec::new();
    for name in ["index.lock", "enhancement.lock"] {
        let path = workspace.index_dir.join(name);
        if !path.exists() {
            continue;
        }
        let lock = fs::OpenOptions::new().write(true).open(&path)?;
        if fs2::FileExt::try_lock_exclusive(&lock).is_err() {
            return Ok(Removal::InUse);
        }
        locks.push(lock);
    }
    // Listing the indexes, asking Git, and waiting for the lease and the
    // locks all took time. A checkout that returned meanwhile keeps its
    // index: with the locks held, nothing can start an index run on it
    // between this check and the removal.
    if !still_gone() {
        return Ok(Removal::RootReturned);
    }
    holder.release(workspace);
    // Stores go while the locks are held. The lock files go last, after their
    // handles close: Windows cannot remove a directory that holds open files.
    // A root that is gone cannot start a job in between.
    for entry in fs::read_dir(&workspace.index_dir)? {
        let entry = entry?;
        if entry.file_name() == "index.lock" || entry.file_name() == "enhancement.lock" {
            continue;
        }
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    drop(locks);
    match fs::remove_dir_all(&workspace.index_dir) {
        Ok(()) => Ok(Removal::Removed),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Removal::Removed),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::tempdir;

    const DAY: u64 = 24 * 60 * 60;
    const GRACE: Duration = Duration::from_secs(7 * DAY);

    fn indexed_root(parent: &Path, name: &str) -> (PathBuf, Workspace) {
        let root = parent.join(name);
        fs::create_dir(&root).unwrap();
        fs::write(root.join("lib.rs"), format!("pub fn {name}() {{}}\n")).unwrap();
        let workspace = Workspace::resolve(&root).unwrap();
        crate::indexer::index_workspace(&workspace, crate::embedding::create_hash_model().as_ref())
            .unwrap();
        (root, workspace)
    }

    fn pass(now_unix: u64) -> GcReport {
        collect_orphaned_indexes_at(now_unix, GRACE, &mut NoIndexHolder).unwrap()
    }

    /// Records the order of the holder calls, and can bring a root back while
    /// the pass waits for its reservation, as a checkout that returns does.
    #[derive(Default)]
    struct RecordingHolder {
        calls: Vec<String>,
        recreate_on_reserve: Option<PathBuf>,
    }

    impl IndexHolder for RecordingHolder {
        fn reserve(&mut self, workspace: &Workspace) -> Box<dyn std::any::Any> {
            self.calls.push(format!("reserve {}", workspace.id));
            if let Some(root) = self.recreate_on_reserve.take() {
                fs::create_dir_all(root).unwrap();
            }
            Box::new(())
        }

        fn release(&mut self, workspace: &Workspace) {
            self.calls.push(format!("release {}", workspace.id));
        }
    }

    #[test]
    #[serial]
    fn an_orphan_is_collected_after_its_grace_period_and_a_live_index_is_kept() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let parent = tempdir().unwrap();
        let (_live_root, live) = indexed_root(parent.path(), "live");
        let (orphan_root, orphan) = indexed_root(parent.path(), "orphan");
        let now = jobs::now_unix();

        assert!(pass(now).waiting.is_empty(), "both roots exist");
        fs::remove_dir_all(&orphan_root).unwrap();

        // The first miss only starts the grace period.
        let report = pass(now);
        assert!(report.collected.is_empty());
        assert_eq!(report.waiting[0].collect_after_unix, now + 7 * DAY);
        assert!(pass(now + 7 * DAY - 1).collected.is_empty());
        assert!(orphan.metadata_path().exists());

        // A root that comes back resets it.
        fs::create_dir(&orphan_root).unwrap();
        assert!(pass(now + 6 * DAY).waiting.is_empty());
        fs::remove_dir_all(&orphan_root).unwrap();
        assert!(pass(now + 6 * DAY).collected.is_empty());
        assert!(pass(now + 8 * DAY).collected.is_empty());

        let mut holder = RecordingHolder::default();
        let report = collect_orphaned_indexes_at(now + 13 * DAY, GRACE, &mut holder).unwrap();
        assert_eq!(report.collected.len(), 1);
        assert_eq!(report.collected[0].root, orphan.root);
        assert_eq!(
            holder.calls,
            [
                format!("reserve {}", orphan.id),
                format!("release {}", orphan.id)
            ],
            "the holder is reserved before its stores are released, and only for the orphan"
        );
        assert!(!orphan.index_dir.exists());
        assert!(live.metadata_path().exists(), "a live index must be kept");
        assert_eq!(
            crate::workspace::list_workspace_roots().unwrap(),
            std::slice::from_ref(&live.root),
            "status must stop listing the collected workspace"
        );
    }

    #[test]
    #[serial]
    fn a_root_that_returns_while_the_pass_waits_keeps_its_index() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let parent = tempdir().unwrap();
        let (root, workspace) = indexed_root(parent.path(), "returning");
        fs::remove_dir_all(&root).unwrap();
        let now = jobs::now_unix();
        assert!(pass(now).collected.is_empty());

        // The pass found the root missing and past its grace period. While it
        // waits for the holder (the daemon's lease), the checkout comes back.
        let mut holder = RecordingHolder {
            recreate_on_reserve: Some(root.clone()),
            ..Default::default()
        };
        let report = collect_orphaned_indexes_at(now + 8 * DAY, GRACE, &mut holder).unwrap();
        assert!(report.collected.is_empty() && report.in_use.is_empty());
        assert_eq!(holder.calls, [format!("reserve {}", workspace.id)]);
        assert!(
            workspace.sqlite_path().exists(),
            "a returning checkout must keep its index"
        );

        // The next pass sees a live root and forgets that it was ever missing.
        assert!(pass(now + 8 * DAY).waiting.is_empty());
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            pass(now + 8 * DAY).waiting[0].collect_after_unix,
            now + 15 * DAY
        );
    }

    #[test]
    #[serial]
    fn a_locked_or_active_orphan_is_kept() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let parent = tempdir().unwrap();
        let (root, workspace) = indexed_root(parent.path(), "busy");
        fs::remove_dir_all(&root).unwrap();
        let now = jobs::now_unix();
        assert!(pass(now).collected.is_empty());

        // A second handle on the lock stands in for an index run in another process.
        let lock = fs::OpenOptions::new()
            .write(true)
            .open(workspace.lock_path())
            .unwrap();
        fs2::FileExt::lock_exclusive(&lock).unwrap();
        let report = pass(now + 8 * DAY);
        assert!(report.collected.is_empty());
        assert_eq!(report.in_use, std::slice::from_ref(&workspace.index_dir));
        assert!(workspace.metadata_path().exists());
        drop(lock);

        jobs::start_job(&workspace, JobKind::Enhancement, "embedding", 1).unwrap();
        assert_eq!(
            pass(now + 8 * DAY).in_use,
            std::slice::from_ref(&workspace.index_dir)
        );
        jobs::finish_job(&workspace, JobKind::Enhancement, "done", None).unwrap();

        assert_eq!(pass(now + 8 * DAY).collected.len(), 1);
        assert!(!workspace.index_dir.exists());
    }

    #[test]
    #[serial]
    fn a_removed_linked_worktree_is_collected_sooner_and_a_base_with_a_live_overlay_is_kept() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let parent = tempdir().unwrap();
        let main = parent.path().join("main");
        fs::create_dir(&main).unwrap();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .args(["-c", "commit.gpgSign=false", "-c", "user.name=Test"])
                .args(["-c", "user.email=test@example.com"])
                .args(args)
                .current_dir(&main)
                .output()
                .unwrap();
            assert!(output.status.success(), "git {args:?}: {output:?}");
        };
        git(&["init", "-b", "main"]);
        fs::write(main.join("lib.rs"), "pub fn base() {}\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-m", "initial"]);
        let model = crate::embedding::create_hash_model();
        let mut overlays = Vec::new();
        for name in ["removed", "detached", "kept"] {
            let root = parent.path().join(format!("worktree-{name}"));
            git(&["worktree", "add", "-b", name, root.to_str().unwrap()]);
            let workspace = Workspace::resolve(&root).unwrap();
            crate::indexer::index_workspace(&workspace, model.as_ref()).unwrap();
            assert!(workspace.base_ref_path().exists());
            overlays.push(workspace);
        }
        let base = Workspace::resolve(&main).unwrap();
        let now = jobs::now_unix();

        // `git worktree remove` unregisters; a vanished directory stays listed.
        // Git gets the path as created: it rejects Windows verbatim paths.
        let removed = parent.path().join("worktree-removed");
        git(&["worktree", "remove", "--force", removed.to_str().unwrap()]);
        fs::remove_dir_all(&overlays[1].root).unwrap();
        assert!(pass(now).collected.is_empty());
        let report = pass(now + DELETED_WORKTREE_GRACE.as_secs());
        assert_eq!(report.collected.len(), 1);
        assert_eq!(report.collected[0].root, overlays[0].root);
        assert!(
            overlays[1].metadata_path().exists(),
            "only missing, still registered"
        );
        assert!(overlays[2].metadata_path().exists());

        // The base repository is deleted while one worktree directory remains.
        fs::remove_dir_all(&main).unwrap();
        assert!(pass(now).collected.is_empty());
        let report = pass(now + 8 * DAY);
        assert_eq!(report.in_use, std::slice::from_ref(&base.index_dir));
        assert_eq!(
            report.collected.len(),
            1,
            "the detached overlay: {report:?}"
        );
        assert!(
            base.metadata_path().exists(),
            "a live overlay still reads it"
        );
        assert!(overlays[2].metadata_path().exists());
    }
}
