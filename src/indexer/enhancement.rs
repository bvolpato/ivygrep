//! Enrichment computes outside the index lock, then publishes only into the
//! same store incarnation. A separate lock serializes vector writers without
//! stopping lexical indexing while a model is running or paused.
use std::fs::{self, File, OpenOptions};

use anyhow::{Context, Result};

use super::resources::{EnhancementTier, check_system_constraints};
use crate::workspace::Workspace;

pub(crate) struct IndexLock(File);

impl IndexLock {
    pub(super) fn acquire(workspace: &Workspace) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(workspace.lock_path())?;
        fs2::FileExt::lock_exclusive(&file).context("lock index for enhancement publication")?;
        Ok(Self(file))
    }
}

impl Drop for IndexLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

pub(super) fn lock_worker(workspace: &Workspace) -> Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(workspace.index_dir.join("enhancement.lock"))?;
    fs2::FileExt::lock_exclusive(&file).context("lock background enhancement worker")?;
    Ok(file)
}

#[derive(Debug)]
pub(crate) struct EnhancementSuperseded;

impl std::fmt::Display for EnhancementSuperseded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("index was replaced during background enhancement; discard obsolete vectors")
    }
}

impl std::error::Error for EnhancementSuperseded {}

pub(crate) struct EnhancementSnapshot {
    incarnation: String,
    pub(super) generation: u64,
}

impl EnhancementSnapshot {
    pub(crate) fn begin(workspace: &Workspace) -> Result<(Self, IndexLock)> {
        let lock = IndexLock::acquire(workspace)?;
        Ok((Self::capture(workspace)?, lock))
    }

    /// Caller holds the index lock until all initial stores and journals have
    /// been opened. Older indexes acquire an incarnation lazily under that lock.
    pub(super) fn capture(workspace: &Workspace) -> Result<Self> {
        let incarnation = match workspace.read_index_incarnation()? {
            Some(value) => value,
            None => {
                let value = uuid::Uuid::new_v4().to_string();
                fs::write(workspace.index_incarnation_path(), &value)?;
                value
            }
        };
        let generation = workspace
            .read_metadata()?
            .map(|metadata| metadata.index_generation)
            .unwrap_or(0);
        Ok(Self {
            incarnation,
            generation,
        })
    }

    pub(crate) fn lock_current(&self, workspace: &Workspace) -> Result<IndexLock> {
        let lock = IndexLock::acquire(workspace)?;
        self.verify_current(workspace)?;
        Ok(lock)
    }

    /// Caller holds index.lock, including when binding a CLI job across stages.
    pub(super) fn verify_current(&self, workspace: &Workspace) -> Result<()> {
        if workspace.read_index_incarnation()?.as_deref() != Some(self.incarnation.as_str()) {
            return Err(EnhancementSuperseded.into());
        }
        Ok(())
    }
}

/// The two kinds of background worker. Each has its own worker limit, so a
/// long neural run cannot hold up the cheap hash vectors that a fresh
/// worktree needs first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WorkerLane {
    /// `--enhance-hash-internal`: hash vectors only.
    Hash,
    /// `--enhance-internal`: hash vectors, then the neural model.
    Neural,
}

impl WorkerLane {
    fn name(self) -> &'static str {
        match self {
            Self::Hash => "hash",
            Self::Neural => "neural",
        }
    }
}

/// What a worker is about to do, which decides the guards that may hold it
/// back. A neural worker builds hash vectors first, and those stay available
/// on battery: only the neural pass answers to the Neural tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkerStage {
    HashPass,
    NeuralPass,
}

impl WorkerStage {
    fn tier(self) -> EnhancementTier {
        match self {
            Self::HashPass => EnhancementTier::Hash,
            Self::NeuralPass => EnhancementTier::Neural,
        }
    }
}

/// How long a worker that a guard holds back sleeps before it looks again.
const GUARD_PAUSE: std::time::Duration = std::time::Duration::from_secs(10);

/// One of the `IVYGREP_ENHANCE_MAX_WORKERS` places of a lane, held for as long
/// as a worker runs. It is a lock on a file under the app home, so the limit
/// holds across every process that starts workers (daemon, CLI, MCP), and a
/// worker that dies frees its place. `None` inside means the places cannot be
/// locked on this host and the worker runs without one.
pub(crate) struct WorkerSlot(#[allow(dead_code)] Option<File>);

/// What one place of a lane looks like to a process that tries to take it.
enum SlotProbe {
    Free(File),
    Held,
    /// The slot directory or file cannot be created or opened, or the file
    /// system does not support locks. Nobody can hold a place then, so the
    /// limit cannot be enforced through the places.
    Unusable(std::io::Error),
}

fn probe_slot(lane: WorkerLane, index: usize) -> SlotProbe {
    let open = || -> std::io::Result<File> {
        let directory = crate::config::app_home()
            .map_err(std::io::Error::other)?
            .join("enhancement-slots");
        fs::create_dir_all(&directory)?;
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(directory.join(format!("{}-{index}.lock", lane.name())))
    };
    let file = match open() {
        Ok(file) => file,
        Err(error) => return SlotProbe::Unusable(error),
    };
    match fs2::FileExt::try_lock_exclusive(&file) {
        Ok(()) => SlotProbe::Free(file),
        Err(error) if error.raw_os_error() == fs2::lock_contended_error().raw_os_error() => {
            SlotProbe::Held
        }
        Err(error) => SlotProbe::Unusable(error),
    }
}

/// Take a free place of `lane` without waiting. When the places cannot be
/// locked at all, the worker runs without one, as every worker did before the
/// limit existed: waiting would never end, and failing would leave the
/// workspace without vectors. The daemon still keeps the workers it starts
/// itself within the limit.
pub(crate) fn try_acquire_worker_slot(lane: WorkerLane) -> Result<Option<WorkerSlot>> {
    for index in 0..crate::config::enhance_max_workers() {
        match probe_slot(lane, index) {
            SlotProbe::Free(file) => return Ok(Some(WorkerSlot(Some(file)))),
            SlotProbe::Held => {}
            SlotProbe::Unusable(error) => {
                tracing::warn!(
                    "enhancement worker places cannot be locked ({error}); running without the IVYGREP_ENHANCE_MAX_WORKERS limit"
                );
                return Ok(Some(WorkerSlot(None)));
            }
        }
    }
    Ok(None)
}

/// Places of `lane` that nobody holds right now. The probe locks one place at
/// a time and lets it go at once, so a worker that polls in that instant
/// misses at most that place and looks again, and two probes can never hold
/// all places between them. A place that cannot be locked counts as free,
/// because no worker can hold it either.
pub(crate) fn free_worker_slots(lane: WorkerLane) -> usize {
    (0..crate::config::enhance_max_workers())
        .filter(|index| !matches!(probe_slot(lane, *index), SlotProbe::Held))
        .count()
}

/// What a worker reports while it waits for a place, through the phase file
/// that `--wait-for-enhancement` reads.
pub(crate) const QUEUED_PHASE: &str = "queued";

/// Block until this worker may run `stage`: the guards of that stage (memory
/// and load, for the neural pass also battery and thermal state) allow it, and
/// the lane has a free place. The worker has loaded no model and opened no
/// store yet, so a waiting worker costs a few MiB. Without this every edited
/// workspace got a running worker at once, and workers that a guard paused
/// kept their model and stores while they waited. `None` means the workspace
/// root vanished while the worker waited.
pub(crate) fn admit_worker(
    workspace: &Workspace,
    lane: WorkerLane,
    stage: WorkerStage,
) -> Result<Option<WorkerSlot>> {
    admit_worker_when(
        workspace,
        lane,
        stage,
        &mut check_system_constraints,
        GUARD_PAUSE,
    )
}

/// A neural worker has built its hash vectors and is about to load the model.
/// If a guard of the neural pass holds it back now (battery, for one), it
/// gives its place to a workspace that can use it and waits with nothing
/// loaded, as it would have at the start.
pub(crate) fn readmit_for_neural_pass(
    workspace: &Workspace,
    slot: WorkerSlot,
) -> Result<Option<WorkerSlot>> {
    readmit_for_neural_pass_when(workspace, slot, &mut check_system_constraints, GUARD_PAUSE)
}

fn readmit_for_neural_pass_when(
    workspace: &Workspace,
    slot: WorkerSlot,
    constraint: &mut dyn FnMut(EnhancementTier) -> Option<String>,
    pause: std::time::Duration,
) -> Result<Option<WorkerSlot>> {
    if constraint(WorkerStage::NeuralPass.tier()).is_none() {
        return Ok(Some(slot));
    }
    drop(slot);
    admit_worker_when(
        workspace,
        WorkerLane::Neural,
        WorkerStage::NeuralPass,
        constraint,
        pause,
    )
}

fn admit_worker_when(
    workspace: &Workspace,
    lane: WorkerLane,
    stage: WorkerStage,
    constraint: &mut dyn FnMut(EnhancementTier) -> Option<String>,
    pause: std::time::Duration,
) -> Result<Option<WorkerSlot>> {
    let paused_path = workspace.enhancing_paused_path();
    let phase_path = workspace.enhancing_phase_path();
    loop {
        if crate::workspace::root_is_gone(&workspace.root) {
            let _ = fs::remove_file(&paused_path);
            let _ = fs::remove_file(&phase_path);
            return Ok(None);
        }
        if let Some(reason) = constraint(stage.tier()) {
            let _ = fs::write(&paused_path, &reason);
            std::thread::sleep(pause);
            continue;
        }
        let _ = fs::remove_file(&paused_path);
        if let Some(slot) = try_acquire_worker_slot(lane)? {
            let _ = fs::remove_file(&phase_path);
            return Ok(Some(slot));
        }
        let _ = fs::write(&phase_path, QUEUED_PHASE);
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

#[cfg(test)]
mod worker_slot_tests {
    use super::*;
    use serial_test::serial;
    use tempfile::tempdir;

    #[test]
    #[serial]
    fn a_lane_never_runs_more_workers_than_its_limit_and_lanes_do_not_share_places() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        unsafe { std::env::set_var("IVYGREP_ENHANCE_MAX_WORKERS", "2") };

        assert_eq!(free_worker_slots(WorkerLane::Neural), 2);
        let first = try_acquire_worker_slot(WorkerLane::Neural)
            .unwrap()
            .unwrap();
        let second = try_acquire_worker_slot(WorkerLane::Neural)
            .unwrap()
            .unwrap();
        assert!(
            try_acquire_worker_slot(WorkerLane::Neural)
                .unwrap()
                .is_none()
        );
        assert_eq!(free_worker_slots(WorkerLane::Neural), 0);
        // Hash vectors do not wait behind neural runs.
        assert_eq!(free_worker_slots(WorkerLane::Hash), 2);
        let hash = try_acquire_worker_slot(WorkerLane::Hash).unwrap().unwrap();

        // A worker that ends, however it ends, frees its place.
        drop(first);
        assert_eq!(free_worker_slots(WorkerLane::Neural), 1);
        let third = try_acquire_worker_slot(WorkerLane::Neural)
            .unwrap()
            .unwrap();
        assert!(
            try_acquire_worker_slot(WorkerLane::Neural)
                .unwrap()
                .is_none()
        );
        drop((second, third, hash));

        unsafe { std::env::set_var("IVYGREP_ENHANCE_MAX_WORKERS", "invalid") };
        assert_eq!(free_worker_slots(WorkerLane::Hash), 2);
        unsafe { std::env::remove_var("IVYGREP_ENHANCE_MAX_WORKERS") };
    }

    #[test]
    #[serial]
    fn neural_worker_builds_hash_vectors_on_battery_and_waits_for_the_model_without_a_place() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        unsafe { std::env::set_var("IVYGREP_ENHANCE_MAX_WORKERS", "1") };
        let repo = tempdir().unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        let pause = std::time::Duration::from_millis(10);

        // On battery the Neural tier is held back and the Hash tier is not.
        let on_battery =
            |tier| (tier == EnhancementTier::Neural).then(|| "Battery Power".to_string());
        let (admitted, outcome) = std::sync::mpsc::channel();
        let hash_pass_workspace = workspace.clone();
        std::thread::spawn(move || {
            let slot = admit_worker_when(
                &hash_pass_workspace,
                WorkerLane::Neural,
                WorkerStage::HashPass,
                &mut { on_battery },
                pause,
            );
            let _ = admitted.send(slot.unwrap());
        });
        let slot = outcome
            .recv_timeout(std::time::Duration::from_secs(60))
            .expect("the hash pass of a neural worker must start on battery")
            .expect("the root exists");
        assert_eq!(free_worker_slots(WorkerLane::Neural), 0);

        // Before the model loads, the neural guards apply. The worker waits
        // with its place given up, and takes one again when power returns.
        let mut checks = 0;
        let mut place_was_free_while_held_back = false;
        let paused_path = workspace.enhancing_paused_path();
        let mut battery_then_mains = |tier| {
            assert_eq!(tier, EnhancementTier::Neural);
            checks += 1;
            if checks > 1 {
                place_was_free_while_held_back |= free_worker_slots(WorkerLane::Neural) == 1
                    && fs::read_to_string(&paused_path)
                        .is_ok_and(|reason| reason == "Battery Power");
            }
            (checks <= 3).then(|| "Battery Power".to_string())
        };
        let slot = readmit_for_neural_pass_when(&workspace, slot, &mut battery_then_mains, pause)
            .unwrap()
            .expect("the root exists");
        assert!(place_was_free_while_held_back);
        assert_eq!(free_worker_slots(WorkerLane::Neural), 0);
        assert!(!paused_path.exists());

        // With nothing holding it back, the worker keeps the place it has.
        let mut unconstrained = |_| None;
        let slot = readmit_for_neural_pass_when(&workspace, slot, &mut unconstrained, pause)
            .unwrap()
            .unwrap();
        drop(slot);
        assert_eq!(free_worker_slots(WorkerLane::Neural), 1);
        unsafe { std::env::remove_var("IVYGREP_ENHANCE_MAX_WORKERS") };
    }

    #[test]
    #[serial]
    fn places_that_cannot_be_locked_do_not_stop_enhancement() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        unsafe { std::env::set_var("IVYGREP_ENHANCE_MAX_WORKERS", "2") };
        // A file sits where the slot directory belongs, so no place can be
        // created, let alone locked. That is not a full lane.
        fs::write(home.path().join("enhancement-slots"), b"").unwrap();

        // The daemon sees capacity and starts workers within its own count.
        assert_eq!(free_worker_slots(WorkerLane::Hash), 2);
        // A worker runs without a place instead of waiting forever or failing.
        let repo = tempdir().unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        let first = admit_worker(&workspace, WorkerLane::Hash, WorkerStage::HashPass).unwrap();
        let second = admit_worker(&workspace, WorkerLane::Hash, WorkerStage::HashPass).unwrap();
        let third = admit_worker(&workspace, WorkerLane::Hash, WorkerStage::HashPass).unwrap();
        assert!(first.is_some() && second.is_some() && third.is_some());
        unsafe { std::env::remove_var("IVYGREP_ENHANCE_MAX_WORKERS") };
    }

    #[test]
    #[serial]
    fn a_queued_worker_says_so_then_runs_when_a_place_frees_or_gives_up_with_its_root() {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        unsafe { std::env::set_var("IVYGREP_ENHANCE_MAX_WORKERS", "1") };
        let repo = tempdir().unwrap();
        fs::write(repo.path().join("lib.rs"), "pub fn queued() {}\n").unwrap();
        let workspace = Workspace::resolve(repo.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        let queued_phase = || {
            fs::read_to_string(workspace.enhancing_phase_path())
                .is_ok_and(|phase| phase == QUEUED_PHASE)
        };

        let running = try_acquire_worker_slot(WorkerLane::Hash).unwrap().unwrap();
        let queued_workspace = workspace.clone();
        let queued = std::thread::spawn(move || {
            admit_worker(&queued_workspace, WorkerLane::Hash, WorkerStage::HashPass).unwrap()
        });
        for _ in 0..600 {
            if queued_phase() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(queued_phase());
        assert!(!queued.is_finished());
        drop(running);
        assert!(queued.join().unwrap().is_some(), "the queue must drain");
        assert!(!workspace.enhancing_phase_path().exists());

        // A worker queued for a workspace that is deleted meanwhile gives up.
        let running = try_acquire_worker_slot(WorkerLane::Hash).unwrap().unwrap();
        let queued_workspace = workspace.clone();
        let queued = std::thread::spawn(move || {
            admit_worker(&queued_workspace, WorkerLane::Hash, WorkerStage::HashPass).unwrap()
        });
        drop(repo);
        assert!(queued.join().unwrap().is_none());
        drop(running);
        unsafe { std::env::remove_var("IVYGREP_ENHANCE_MAX_WORKERS") };
    }
}
