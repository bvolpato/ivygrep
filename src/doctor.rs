use std::path::PathBuf;

use anyhow::Result;
use serde::Serialize;

use crate::embedding::create_hash_model;
use crate::indexer::{index_workspace, remove_workspace_index};
use crate::jobs::{
    self, ENHANCEMENT_HEARTBEAT_TTL_SECS, ENHANCEMENT_PAUSE_WARN_SECS, INDEXING_HEARTBEAT_TTL_SECS,
    JobKind, WATCHER_HEARTBEAT_TTL_SECS,
};
use crate::workspace::{Workspace, WorkspaceIndexHealth, WorkspaceIndexState};

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub workspace_root: PathBuf,
    pub state: WorkspaceIndexState,
    pub healthy: bool,
    pub chunk_count: u64,
    pub file_count: u64,
    pub has_indexable_files: bool,
    pub findings: Vec<String>,
    pub repaired: bool,
}

impl DoctorReport {
    fn from_health(
        workspace: &Workspace,
        health: WorkspaceIndexHealth,
        repaired: bool,
        mut findings: Vec<String>,
    ) -> Self {
        if findings.is_empty() {
            findings = default_findings(&health);
        }

        Self {
            workspace_root: workspace.root.clone(),
            state: health.state,
            healthy: health.is_queryable(),
            chunk_count: health.chunk_count,
            file_count: health.file_count,
            has_indexable_files: health.has_indexable_files,
            findings,
            repaired,
        }
    }
}

pub fn inspect_workspace(workspace: &Workspace) -> DoctorReport {
    let health = workspace.index_health();
    DoctorReport::from_health(workspace, health, false, runtime_findings(workspace))
}

pub fn inspect_and_maybe_fix(workspace: &Workspace, fix: bool) -> Result<DoctorReport> {
    let initial = workspace.index_health();
    if !fix {
        return Ok(DoctorReport::from_health(
            workspace,
            initial,
            false,
            runtime_findings(workspace),
        ));
    }

    let should_rebuild = match initial.state {
        WorkspaceIndexState::Unhealthy => true,
        WorkspaceIndexState::NotIndexed => initial.has_indexable_files,
        WorkspaceIndexState::Healthy | WorkspaceIndexState::HealthyEmpty => false,
    };

    if !should_rebuild {
        let mut findings = default_findings(&initial);
        if findings.is_empty() {
            findings.push("no repair needed".to_string());
        }
        return Ok(DoctorReport::from_health(
            workspace, initial, false, findings,
        ));
    }

    rebuild_workspace_index(workspace)?;

    let repaired = workspace.index_health();
    let mut findings = runtime_findings(workspace);
    findings.extend(default_findings(&repaired));
    if repaired.is_queryable() {
        findings.insert(0, "index rebuilt successfully".to_string());
    } else {
        findings.insert(
            0,
            "rebuild completed, but the workspace still needs attention".to_string(),
        );
    }

    Ok(DoctorReport::from_health(
        workspace, repaired, true, findings,
    ))
}

fn runtime_findings(workspace: &Workspace) -> Vec<String> {
    let mut findings = Vec::new();

    let watcher_status = jobs::job_status(workspace, JobKind::Watcher, WATCHER_HEARTBEAT_TTL_SECS);
    if watcher_status.stalled {
        findings.push("watcher job heartbeat is stale".to_string());
    }
    if watcher_status.record.is_none() && workspace.watcher_pid_path().exists() {
        findings.push("legacy watcher pid file is stale".to_string());
    }
    if let Some(record) = watcher_status.record.as_ref()
        && record
            .details
            .get("coalesced_events")
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|value| value >= 32)
    {
        findings.push("watcher is coalescing a heavy burst of filesystem events".to_string());
    }

    let indexing_status =
        jobs::job_status(workspace, JobKind::Indexing, INDEXING_HEARTBEAT_TTL_SECS);
    if indexing_status.stalled {
        findings.push("indexing job heartbeat is stale".to_string());
    }
    if indexing_status.record.is_none() && workspace.indexing_pid_path().exists() {
        findings.push("legacy indexing pid file is stale".to_string());
    }

    let enhancement_status = jobs::job_status(
        workspace,
        JobKind::Enhancement,
        ENHANCEMENT_HEARTBEAT_TTL_SECS,
    );
    if enhancement_status.stalled {
        findings.push("background neural enhancement heartbeat is stale".to_string());
    }
    if enhancement_status.record.is_none() && workspace.enhancing_pid_path().exists() {
        findings.push("legacy enhancement pid file is stale".to_string());
    }
    if let Ok(meta) = std::fs::metadata(workspace.enhancing_paused_path())
        && let Ok(modified) = meta.modified()
        && let Ok(age) = modified.elapsed()
        && age.as_secs() > ENHANCEMENT_PAUSE_WARN_SECS
    {
        findings.push("neural enhancement has been paused for a long time".to_string());
    }

    findings
}

fn rebuild_workspace_index(workspace: &Workspace) -> Result<()> {
    let preserved_metadata = workspace.read_metadata().ok().flatten();

    if workspace.exists() {
        remove_workspace_index(workspace)?;
    }

    if let Some(mut metadata) = preserved_metadata {
        metadata.last_indexed_at_unix = None;
        workspace.ensure_dirs()?;
        workspace.write_metadata(&metadata)?;
    }

    let model = create_hash_model();
    let _ = index_workspace(workspace, model.as_ref())?;
    Ok(())
}

fn default_findings(health: &WorkspaceIndexHealth) -> Vec<String> {
    if !health.issues.is_empty() {
        return health.issues.clone();
    }

    match health.state {
        WorkspaceIndexState::Healthy => {
            vec!["index is healthy".to_string()]
        }
        WorkspaceIndexState::HealthyEmpty => {
            vec!["workspace has no indexable files, so an empty index is valid".to_string()]
        }
        WorkspaceIndexState::NotIndexed => {
            vec!["workspace is not indexed yet".to_string()]
        }
        WorkspaceIndexState::Unhealthy => {
            vec!["workspace index is unhealthy".to_string()]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::WorkspaceMetadata;
    use serial_test::serial;

    #[test]
    #[serial]
    fn doctor_flags_zero_chunk_index_as_unhealthy() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            root.path().join("lib.rs"),
            "pub fn answer() -> usize { 42 }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(root.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        workspace
            .write_metadata(&WorkspaceMetadata {
                id: workspace.id.clone(),
                root: workspace.root.clone(),
                created_at_unix: 0,
                last_indexed_at_unix: Some(1),
                watch_enabled: false,
                skip_gitignore: false,
            })
            .unwrap();
        std::fs::write(workspace.sqlite_path(), "").unwrap();
        std::fs::create_dir_all(workspace.tantivy_dir()).unwrap();
        std::fs::write(workspace.vector_path(), "").unwrap();

        let report = inspect_workspace(&workspace);
        assert_eq!(report.state, WorkspaceIndexState::Unhealthy);
        assert!(!report.healthy);
    }

    #[test]
    #[serial]
    fn doctor_fix_rebuilds_unhealthy_index() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            root.path().join("lib.rs"),
            "pub fn answer() -> usize { 42 }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(root.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        workspace
            .write_metadata(&WorkspaceMetadata {
                id: workspace.id.clone(),
                root: workspace.root.clone(),
                created_at_unix: 0,
                last_indexed_at_unix: Some(1),
                watch_enabled: false,
                skip_gitignore: false,
            })
            .unwrap();
        std::fs::write(workspace.sqlite_path(), "").unwrap();
        std::fs::create_dir_all(workspace.tantivy_dir()).unwrap();
        std::fs::write(workspace.vector_path(), "").unwrap();

        let report = inspect_and_maybe_fix(&workspace, true).unwrap();
        assert!(report.repaired);
        assert!(report.healthy);
        assert!(report.chunk_count >= 1);
    }

    #[test]
    #[serial]
    fn doctor_reports_stalled_indexing_job() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            root.path().join("lib.rs"),
            "pub fn answer() -> usize { 42 }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(root.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        let _ = crate::jobs::start_job(&workspace, crate::jobs::JobKind::Indexing, "scanning", 1);

        let mut ledger: serde_json::Value =
            serde_json::from_slice(&std::fs::read(workspace.job_ledger_path()).unwrap()).unwrap();
        ledger["jobs"][0]["heartbeat_at_unix"] = serde_json::json!(0);
        std::fs::write(
            workspace.job_ledger_path(),
            serde_json::to_vec_pretty(&ledger).unwrap(),
        )
        .unwrap();

        let report = inspect_workspace(&workspace);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.contains("indexing job heartbeat is stale")),
            "expected stalled indexing finding, got {:#?}",
            report.findings
        );
    }

    #[test]
    #[serial]
    fn doctor_reports_watcher_event_storms() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            root.path().join("lib.rs"),
            "pub fn answer() -> usize { 42 }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(root.path()).unwrap();
        workspace.ensure_dirs().unwrap();
        let _ = crate::jobs::start_job(&workspace, crate::jobs::JobKind::Watcher, "dirty", 1);
        let mut ledger: serde_json::Value =
            serde_json::from_slice(&std::fs::read(workspace.job_ledger_path()).unwrap()).unwrap();
        ledger["jobs"][0]["details"]["coalesced_events"] = serde_json::json!("64");
        std::fs::write(
            workspace.job_ledger_path(),
            serde_json::to_vec_pretty(&ledger).unwrap(),
        )
        .unwrap();

        let report = inspect_workspace(&workspace);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.contains("heavy burst of filesystem events")),
            "expected watcher coalescing finding, got {:#?}",
            report.findings
        );
    }
}
