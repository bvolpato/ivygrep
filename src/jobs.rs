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

    update_job(workspace, kind, |ledger| {
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
    update_job(workspace, kind, move |ledger| {
        let mut record = ledger.get(kind).cloned().unwrap_or(JobRecord {
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
        ledger.upsert(record.clone());
        record
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
    update_job(workspace, kind, move |ledger| {
        let mut record = ledger.get(kind).cloned().unwrap_or(JobRecord {
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

        record.active = false;
        record.pid = None;
        record.pid_start_time = None;
        record.nonce = None;
        record.heartbeat_at_unix = Some(now);
        record.phase = phase;
        record.last_error = last_error;
        ledger.upsert(record.clone());
        record
    })
}

pub fn job_status(workspace: &Workspace, kind: JobKind, ttl_secs: u64) -> JobStatus {
    let ledger = read_job_ledger(workspace);
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
        .is_some_and(|ts| now_unix().saturating_sub(ts) > ttl_secs);
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
            return false;
        }

        if let Some(expected) = expected_start_time
            && let Some(actual) = process_start_time_token(pid)
        {
            return actual == expected;
        }

        alive
    }
    #[cfg(not(unix))]
    {
        let _ = expected_start_time;
        true
    }
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

fn update_job(
    workspace: &Workspace,
    kind: JobKind,
    updater: impl FnOnce(&mut JobLedger) -> JobRecord,
) -> Result<JobRecord> {
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
    let _ = kind;
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
}
