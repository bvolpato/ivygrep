use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::workspace::Workspace;

pub const WATCHER_HEARTBEAT_TTL_SECS: u64 = 15;
pub const INDEXING_HEARTBEAT_TTL_SECS: u64 = 20;
pub const ENHANCEMENT_HEARTBEAT_TTL_SECS: u64 = 20;
pub const ENHANCEMENT_PAUSE_WARN_SECS: u64 = 300;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Watcher,
    Indexing,
    Enhancement,
}

impl JobKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Watcher => "watcher",
            Self::Indexing => "indexing",
            Self::Enhancement => "enhancement",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub kind: JobKind,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub pid_start_time: Option<String>,
    #[serde(default)]
    pub nonce: Option<String>,
    #[serde(default)]
    pub generation: u64,
    #[serde(default)]
    pub started_at_unix: Option<u64>,
    #[serde(default)]
    pub heartbeat_at_unix: Option<u64>,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub attempt: u32,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub details: BTreeMap<String, String>,
    #[serde(default)]
    pub active: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobLedger {
    #[serde(default)]
    pub jobs: Vec<JobRecord>,
}

impl JobLedger {
    fn get(&self, kind: JobKind) -> Option<&JobRecord> {
        self.jobs.iter().find(|job| job.kind == kind)
    }

    pub(crate) fn contains(&self, kind: JobKind) -> bool {
        self.get(kind).is_some()
    }

    fn upsert(&mut self, record: JobRecord) {
        if let Some(existing) = self.jobs.iter_mut().find(|job| job.kind == record.kind) {
            *existing = record;
        } else {
            self.jobs.push(record);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct JobUpdate {
    pub phase: Option<String>,
    pub last_error: Option<Option<String>>,
    pub details: BTreeMap<String, String>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct JobStatus {
    pub record: Option<JobRecord>,
    pub process_alive: bool,
    pub heartbeat_stale: bool,
    pub stalled: bool,
}

impl JobStatus {
    pub fn active(&self) -> bool {
        self.record
            .as_ref()
            .is_some_and(|record| record.active && self.process_alive && !self.heartbeat_stale)
    }
}

pub fn read_job_ledger(workspace: &Workspace) -> JobLedger {
    let path = workspace.job_ledger_path();
    let Ok(raw) = fs::read(&path) else {
        return JobLedger::default();
    };
    serde_json::from_slice(&raw).unwrap_or_default()
}

pub fn start_job(
    workspace: &Workspace,
    kind: JobKind,
    phase: impl Into<String>,
    attempt: u32,
) -> Result<JobRecord> {
    let phase = phase.into();
    let pid = std::process::id();
    let pid_start_time = process_start_time_token(pid);
    let nonce = Uuid::new_v4().to_string();
    let now = now_unix();

    update_job(workspace, |ledger| {
        let generation = ledger.get(kind).map(|job| job.generation + 1).unwrap_or(1);
        let mut details = ledger
            .get(kind)
            .map(|job| job.details.clone())
            .unwrap_or_default();
        details.insert("last_started_at_unix".to_string(), now.to_string());
        let record = JobRecord {
            kind,
            pid: Some(pid),
            pid_start_time,
            nonce: Some(nonce),
            generation,
            started_at_unix: Some(now),
            heartbeat_at_unix: Some(now),
            phase,
            attempt,
            last_error: None,
            details,
            active: true,
        };
        ledger.upsert(record.clone());
        record
    })
}

pub fn heartbeat_job(workspace: &Workspace, kind: JobKind, update: JobUpdate) -> Result<JobRecord> {
    let now = now_unix();
    update_job(workspace, move |ledger| {
        let record = ledger.get(kind).cloned().unwrap_or(JobRecord {
            kind,
            pid: Some(std::process::id()),
            pid_start_time: process_start_time_token(std::process::id()),
            nonce: Some(Uuid::new_v4().to_string()),
            generation: 1,
            started_at_unix: Some(now),
            heartbeat_at_unix: Some(now),
            phase: String::new(),
            attempt: 1,
            last_error: None,
            details: BTreeMap::new(),
            active: true,
        });
        let record = apply_heartbeat_update(record, update, now);
        ledger.upsert(record.clone());
        record
    })
}

pub fn heartbeat_job_if_current(
    workspace: &Workspace,
    kind: JobKind,
    expected_nonce: &str,
    update: JobUpdate,
) -> Result<Option<JobRecord>> {
    let now = now_unix();
    update_job(workspace, move |ledger| {
        let record = ledger
            .get(kind)
            .filter(|record| record.nonce.as_deref() == Some(expected_nonce))
            .cloned()?;
        let record = apply_heartbeat_update(record, update, now);
        ledger.upsert(record.clone());
        Some(record)
    })
}

pub fn finish_job(
    workspace: &Workspace,
    kind: JobKind,
    phase: impl Into<String>,
    last_error: Option<String>,
) -> Result<JobRecord> {
    let phase = phase.into();
    let now = now_unix();
    update_job(workspace, move |ledger| {
        let record = ledger.get(kind).cloned().unwrap_or(JobRecord {
            kind,
            pid: None,
            pid_start_time: None,
            nonce: None,
            generation: 1,
            started_at_unix: Some(now),
            heartbeat_at_unix: Some(now),
            phase: String::new(),
            attempt: 1,
            last_error: None,
            details: BTreeMap::new(),
            active: false,
        });
        let record = finish_record(record, phase, last_error, now);
        ledger.upsert(record.clone());
        record
    })
}

pub fn finish_job_if_current(
    workspace: &Workspace,
    kind: JobKind,
    expected_nonce: &str,
    phase: impl Into<String>,
    last_error: Option<String>,
) -> Result<Option<JobRecord>> {
    let phase = phase.into();
    let now = now_unix();
    update_job(workspace, move |ledger| {
        let record = ledger
            .get(kind)
            .filter(|record| record.nonce.as_deref() == Some(expected_nonce))
            .cloned()?;
        let record = finish_record(record, phase, last_error, now);
        ledger.upsert(record.clone());
        Some(record)
    })
}

fn apply_heartbeat_update(mut record: JobRecord, update: JobUpdate, now: u64) -> JobRecord {
    record.heartbeat_at_unix = Some(now);
    if let Some(phase) = update.phase {
        record.phase = phase;
    }
    if let Some(last_error) = update.last_error {
        record.last_error = last_error;
    }
    if let Some(active) = update.active {
        record.active = active;
    }
    for (key, value) in update.details {
        record.details.insert(key, value);
    }
    record
}

fn finish_record(
    mut record: JobRecord,
    phase: String,
    last_error: Option<String>,
    now: u64,
) -> JobRecord {
    record.active = false;
    record.pid = None;
    record.pid_start_time = None;
    record.nonce = None;
    record.heartbeat_at_unix = Some(now);
    record.phase = phase;
    record.last_error = last_error;
    record
}

pub fn job_status(workspace: &Workspace, kind: JobKind, ttl_secs: u64) -> JobStatus {
    let ledger = read_job_ledger(workspace);
    job_status_at(&ledger, kind, ttl_secs, now_unix())
}

/// Derive status from one caller-owned ledger snapshot and observation time.
pub(crate) fn job_status_at(
    ledger: &JobLedger,
    kind: JobKind,
    ttl_secs: u64,
    observed_at_unix: u64,
) -> JobStatus {
    let Some(record) = ledger.get(kind).cloned() else {
        return JobStatus {
            record: None,
            process_alive: false,
            heartbeat_stale: false,
            stalled: false,
        };
    };

    let process_alive = record
        .pid
        .is_some_and(|pid| process_is_alive(pid, record.pid_start_time.as_deref()));
    let heartbeat_stale = record
        .heartbeat_at_unix
        .is_some_and(|ts| observed_at_unix.saturating_sub(ts) > ttl_secs);
    let stalled = record.active && (!process_alive || heartbeat_stale);

    JobStatus {
        record: Some(record),
        process_alive,
        heartbeat_stale,
        stalled,
    }
}

pub fn process_is_alive(pid: u32, expected_start_time: Option<&str>) -> bool {
    #[cfg(unix)]
    {
        let pid_i32 = pid as i32;
        let alive = unsafe { libc::kill(pid_i32, 0) } == 0;
        if !alive {
            forget_verified_process(pid);
            return false;
        }

        if let Some(expected) = expected_start_time {
            if process_identity_verified(pid, expected) {
                return true;
            }
            if let Some(actual) = process_start_time_token(pid) {
                let matches = actual == expected;
                if matches {
                    remember_verified_process(pid, expected);
                }
                return matches;
            }
        }

        alive
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        let _ = expected_start_time;
        true
    }
}

/// How long a positive `(pid, start time)` match is trusted before the start
/// time is probed again. Hot query paths check worker liveness several times
/// per second; forking `ps` (macOS) or reading `/proc` each time is the cost
/// being avoided. The bound caps the window in which a recycled pid could be
/// mistaken for the job that owned it.
#[cfg(unix)]
const VERIFIED_PROCESS_TTL: std::time::Duration = std::time::Duration::from_secs(30);

#[cfg(unix)]
type VerifiedProcesses = std::collections::HashMap<u32, (String, std::time::Instant)>;

#[cfg(unix)]
fn verified_processes() -> &'static std::sync::Mutex<VerifiedProcesses> {
    static VERIFIED: std::sync::OnceLock<std::sync::Mutex<VerifiedProcesses>> =
        std::sync::OnceLock::new();
    VERIFIED.get_or_init(|| std::sync::Mutex::new(VerifiedProcesses::new()))
}

#[cfg(unix)]
fn process_identity_verified(pid: u32, expected_start_time: &str) -> bool {
    verified_processes()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&pid)
        .is_some_and(|(token, verified_at)| {
            token == expected_start_time && verified_at.elapsed() < VERIFIED_PROCESS_TTL
        })
}

#[cfg(unix)]
fn remember_verified_process(pid: u32, start_time: &str) {
    let mut verified = verified_processes()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if verified.len() >= 1024 {
        verified.retain(|_, (_, verified_at)| verified_at.elapsed() < VERIFIED_PROCESS_TTL);
    }
    verified.insert(pid, (start_time.to_string(), std::time::Instant::now()));
}

#[cfg(unix)]
fn forget_verified_process(pid: u32) {
    verified_processes()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&pid);
}

pub fn process_start_time_token(pid: u32) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let (_, rest) = stat.rsplit_once(") ")?;
        let fields: Vec<&str> = rest.split_whitespace().collect();
        let start_time = fields.get(19)?;
        if start_time.is_empty() {
            None
        } else {
            Some((*start_time).to_string())
        }
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        use std::process::Command;

        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "lstart="])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if token.is_empty() { None } else { Some(token) }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        None
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn update_job<T>(workspace: &Workspace, updater: impl FnOnce(&mut JobLedger) -> T) -> Result<T> {
    workspace.ensure_dirs()?;
    let lock_path = workspace.job_lock_path();
    let lock = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("failed to open job lock {}", lock_path.display()))?;
    lock.lock_exclusive()
        .with_context(|| format!("failed to lock job ledger {}", lock_path.display()))?;

    let mut ledger = read_job_ledger(workspace);
    let record = updater(&mut ledger);
    write_job_ledger_locked(workspace.job_ledger_path(), &ledger)?;
    let _ = lock.unlock();
    Ok(record)
}

fn write_job_ledger_locked(path: impl AsRef<Path>, ledger: &JobLedger) -> Result<()> {
    let path = path.as_ref();
    let tmp = path.with_extension("tmp");
    let data = serde_json::to_vec_pretty(ledger)?;
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn ledger_start_and_finish_roundtrip() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();

        let started = start_job(&workspace, JobKind::Indexing, "scanning", 1).unwrap();
        assert!(started.active);
        assert_eq!(started.phase, "scanning");

        let finished = finish_job(&workspace, JobKind::Indexing, "completed", None).unwrap();
        assert!(!finished.active);
        assert_eq!(finished.phase, "completed");
    }

    #[test]
    #[serial]
    fn status_snapshot_is_stable_after_ledger_changes() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();

        start_job(&workspace, JobKind::Indexing, "scanning", 1).unwrap();
        let snapshot = read_job_ledger(&workspace);
        assert!(snapshot.contains(JobKind::Indexing));

        finish_job(&workspace, JobKind::Indexing, "completed", None).unwrap();

        assert!(job_status_at(&snapshot, JobKind::Indexing, 20, now_unix()).active());
        assert!(!job_status(&workspace, JobKind::Indexing, 20).active());
    }

    #[test]
    #[serial]
    fn stale_job_generation_cannot_overwrite_current_record() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let workspace = Workspace::resolve(root.path()).unwrap();

        let first = start_job(&workspace, JobKind::Watcher, "first", 1).unwrap();
        let second = start_job(&workspace, JobKind::Watcher, "second", 1).unwrap();
        let first_nonce = first.nonce.as_deref().unwrap();
        let second_nonce = second.nonce.as_deref().unwrap();

        assert!(
            heartbeat_job_if_current(
                &workspace,
                JobKind::Watcher,
                first_nonce,
                JobUpdate {
                    phase: Some("stale-heartbeat".to_string()),
                    ..Default::default()
                },
            )
            .unwrap()
            .is_none()
        );
        assert!(
            finish_job_if_current(
                &workspace,
                JobKind::Watcher,
                first_nonce,
                "stale-finish",
                None,
            )
            .unwrap()
            .is_none()
        );

        let current = read_job_ledger(&workspace)
            .get(JobKind::Watcher)
            .unwrap()
            .clone();
        assert_eq!(current.nonce.as_deref(), Some(second_nonce));
        assert_eq!(current.phase, "second");
        assert!(current.active);
    }

    #[cfg(unix)]
    #[test]
    fn process_liveness_memoizes_only_verified_identities() {
        let pid = std::process::id();
        let token = process_start_time_token(pid).expect("own start time");

        forget_verified_process(pid);
        assert!(!process_identity_verified(pid, &token));
        assert!(process_is_alive(pid, Some(&token)));
        assert!(
            process_identity_verified(pid, &token),
            "a confirmed start time is remembered"
        );

        assert!(
            !process_is_alive(pid, Some("Thu Jan  1 00:00:00 1970")),
            "a different start time means a different process"
        );
        assert!(
            process_identity_verified(pid, &token),
            "a mismatch does not poison the verified entry"
        );
        assert!(!process_identity_verified(pid, "Thu Jan  1 00:00:00 1970"));

        // A child that has already exited is dead regardless of its token,
        // and its entry is dropped.
        let mut child = std::process::Command::new("true")
            .spawn()
            .expect("spawn true");
        let child_pid = child.id();
        remember_verified_process(child_pid, "stale");
        child.wait().expect("wait child");
        assert!(!process_is_alive(child_pid, Some("stale")));
        assert!(!process_identity_verified(child_pid, "stale"));
    }
}
