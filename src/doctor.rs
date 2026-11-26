use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::embedding::{NeuralModelIdentity, create_hash_model};
use crate::indexer::{index_workspace, remove_workspace_index};
use crate::jobs::{
    self, ENHANCEMENT_HEARTBEAT_TTL_SECS, ENHANCEMENT_PAUSE_WARN_SECS, INDEXING_HEARTBEAT_TTL_SECS,
    JobKind, WATCHER_HEARTBEAT_TTL_SECS,
};
use crate::workspace::{
    IndexCompactionHealth, IndexComponentSizes, Workspace, WorkspaceIndexHealth,
    WorkspaceIndexState, WorkspaceMetadata,
};

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub workspace_root: PathBuf,
    pub state: WorkspaceIndexState,
    pub healthy: bool,
    pub chunk_count: u64,
    pub file_count: u64,
    pub vector_key_count: u64,
    pub hash_vector_count: u64,
    pub hash_coverage_percent: f64,
    pub neural_vector_count: u64,
    pub neural_coverage_percent: f64,
    pub neural_profile: String,
    pub neural_model: Option<NeuralModelIdentity>,
    pub neural_dimensions: usize,
    pub index_components: IndexComponentSizes,
    pub compaction: IndexCompactionHealth,
    pub reranker_candidate_limit: usize,
    pub reranker_mode: String,
    pub reranker_model: Option<String>,
    pub reranker_model_sha256: Option<String>,
    pub reranker_error: Option<String>,
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
        for issue in &health.issues {
            if !findings.contains(issue) {
                findings.push(issue.clone());
            }
        }
        let mut neural_metadata_invalid = false;
        for index_dir in
            std::iter::once(&workspace.index_dir).chain(workspace.base_index_dir.iter())
        {
            if let Err(error) =
                crate::neural_metadata::read_identity(&index_dir.join("neural_model.json"))
            {
                neural_metadata_invalid = true;
                findings.push(format!("{error:#}"));
            }
        }
        if findings.is_empty() {
            findings = default_findings(workspace, &health);
        }

        let unrepaired = neural_metadata_invalid || indexing_was_interrupted(workspace);
        let reranker = crate::reranker::runtime_status();
        Self {
            workspace_root: workspace.root.clone(),
            state: if unrepaired && health.is_queryable() {
                WorkspaceIndexState::Unhealthy
            } else {
                health.state
            },
            healthy: health.is_queryable() && !unrepaired,
            chunk_count: health.chunk_count,
            file_count: health.file_count,
            vector_key_count: workspace.vector_key_count(),
            hash_vector_count: workspace.hash_vector_count(),
            hash_coverage_percent: workspace.hash_coverage_percent(),
            neural_vector_count: workspace.neural_vector_count(),
            neural_coverage_percent: workspace.neural_coverage_percent(),
            neural_profile: workspace
                .neural_profile_name()
                .unwrap_or_else(|| crate::embedding::configured_neural_profile_name().to_string()),
            neural_model: workspace.neural_model_identity(),
            neural_dimensions: workspace
                .neural_model_identity()
                .map(|identity| identity.dimensions)
                .unwrap_or_else(|| crate::embedding::NeuralProfile::configured().dimensions()),
            index_components: workspace.index_component_sizes(),
            compaction: workspace.index_compaction_health(),
            reranker_candidate_limit: crate::search::rerank_candidate_limit(),
            reranker_mode: reranker.mode,
            reranker_model: reranker.model_id,
            reranker_model_sha256: reranker.model_sha256,
            reranker_error: reranker.error,
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

pub fn inspect_workspace_quick(workspace: &Workspace) -> DoctorReport {
    let health = workspace.quick_index_health();
    DoctorReport::from_health(workspace, health, false, runtime_findings(workspace))
}

pub fn inspect_and_maybe_fix(workspace: &Workspace, fix: bool, deep: bool) -> Result<DoctorReport> {
    inspect_and_fix_if_confirmed(workspace, fix, deep, &mut |_| Ok(()))
}

/// [`inspect_and_maybe_fix`] with a check that runs before `--fix` builds an
/// index. An error from it leaves the index as it is.
pub(crate) fn inspect_and_fix_if_confirmed(
    workspace: &Workspace,
    fix: bool,
    deep: bool,
    confirm_rebuild: &mut dyn FnMut(WorkspaceIndexState) -> Result<()>,
) -> Result<DoctorReport> {
    let mut cleanup_actions = if fix {
        workspace.cleanup_stale_legacy_runtime_files()
    } else {
        Vec::new()
    };
    if fix {
        cleanup_actions.extend(repair_invalid_neural_metadata(workspace)?);
    }
    let initial = if fix || deep {
        workspace.index_health()
    } else {
        workspace.quick_index_health()
    };
    if !fix {
        let mut findings = runtime_findings(workspace);
        findings.extend(daemon_finding());
        return Ok(DoctorReport::from_health(
            workspace, initial, false, findings,
        ));
    }

    let should_rebuild = match initial.state {
        WorkspaceIndexState::Unhealthy => true,
        WorkspaceIndexState::NotIndexed => initial.has_indexable_files,
        WorkspaceIndexState::Healthy | WorkspaceIndexState::HealthyEmpty => {
            indexing_was_interrupted(workspace)
        }
    };

    if !should_rebuild {
        let compacted = workspace.compact_sqlite_if_needed()?;
        let repaired_runtime = !cleanup_actions.is_empty() || compacted;
        let mut findings = cleanup_actions;
        if compacted {
            findings.push("SQLite index compacted successfully".to_string());
        }
        findings.extend(runtime_findings(workspace));
        findings.extend(daemon_finding());
        findings.extend(default_findings(workspace, &initial));
        if findings.is_empty() {
            findings.push("no repair needed".to_string());
        }
        return Ok(DoctorReport::from_health(
            workspace,
            initial,
            repaired_runtime,
            findings,
        ));
    }

    confirm_rebuild(initial.state)?;
    rebuild_workspace_index(workspace)?;

    let repaired = workspace.index_health();
    let mut findings = cleanup_actions;
    findings.extend(runtime_findings(workspace));
    findings.extend(daemon_finding());
    findings.extend(default_findings(workspace, &repaired));
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

/// Whether an indexing process ended before its run did. The job record keeps
/// this until the next run, unlike the PID file, which a health check removes.
fn indexing_was_interrupted(workspace: &Workspace) -> bool {
    let status = jobs::job_status(workspace, JobKind::Indexing, INDEXING_HEARTBEAT_TTL_SECS);
    status.stalled && !status.process_alive
}

/// A daemon endpoint that no daemon answers. Without an endpoint there is no
/// daemon, which is not a fault: the next search can start one.
fn daemon_finding() -> Option<String> {
    if !crate::ipc::socket_exists() {
        return None;
    }
    let answered = crate::daemon::request_blocking(&crate::protocol::DaemonRequest::Version, false)
        .is_ok_and(|response| response.is_some());
    if answered {
        None
    } else if crate::ipc::socket_exists() {
        Some("the daemon endpoint exists, but no daemon answers".to_string())
    } else {
        Some("removed the endpoint of a daemon that no longer runs".to_string())
    }
}

fn runtime_findings(workspace: &Workspace) -> Vec<String> {
    let mut findings = Vec::new();

    let watcher_status = jobs::job_status(workspace, JobKind::Watcher, WATCHER_HEARTBEAT_TTL_SECS);
    if watcher_status.stalled {
        findings.push("watcher job heartbeat is stale".to_string());
    } else if !watcher_status.active()
        && !workspace.is_watcher_alive()
        && workspace
            .read_metadata()
            .is_ok_and(|metadata| metadata.is_some_and(|metadata| metadata.watch_enabled))
    {
        findings.push("the watcher is configured, but it does not run".to_string());
    }
    findings.extend(workspace.stale_legacy_runtime_findings());
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
    if indexing_was_interrupted(workspace) {
        findings.push(
            "the last indexing run did not finish; run `ig --doctor --fix` to rebuild".to_string(),
        );
    } else if indexing_status.stalled {
        findings.push("indexing job heartbeat is stale".to_string());
    }

    let enhancement_status = jobs::job_status(
        workspace,
        JobKind::Enhancement,
        ENHANCEMENT_HEARTBEAT_TTL_SECS,
    );
    if enhancement_status.stalled {
        findings.push("background enhancement heartbeat is stale".to_string());
    }
    if let Some(error) = workspace.enhancement_error()
        && !error.contains("neural feature not compiled")
    {
        findings.push(format!(
            "background enhancement failed: {}",
            error.lines().next().unwrap_or_default()
        ));
    }
    if let Ok(meta) = std::fs::metadata(workspace.enhancing_paused_path())
        && let Ok(modified) = meta.modified()
        && let Ok(age) = modified.elapsed()
        && age.as_secs() > ENHANCEMENT_PAUSE_WARN_SECS
    {
        findings.push("background enhancement has been paused for a long time".to_string());
    }

    let compaction = workspace.index_compaction_health();
    if compaction.sqlite_page_bytes > 0
        && compaction.format_version != compaction.current_format_version
    {
        findings.push(format!(
            "index format v{} is incompatible with v{}; run `ig --doctor --fix` to rebuild",
            compaction.format_version, compaction.current_format_version
        ));
    } else if compaction.compaction_recommended {
        findings.push(format!(
            "SQLite has {:.1}% reclaimable pages; run `ig --doctor --fix` to compact",
            compaction.sqlite_free_percent
        ));
    }
    if compaction.legacy_graph_bytes > 0 {
        findings.push(
            "legacy persisted call graph is present; rebuild to reclaim its storage".to_string(),
        );
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

fn invalid_neural_identity(workspace: &Workspace) -> Result<bool> {
    match crate::neural_metadata::read_identity(&workspace.neural_model_path()) {
        Ok(_) => Ok(false),
        Err(error) if error.downcast_ref::<serde_json::Error>().is_some() => Ok(true),
        Err(error) => Err(error),
    }
}

fn repair_invalid_neural_metadata(workspace: &Workspace) -> Result<Vec<String>> {
    let mut targets = vec![workspace.clone()];
    if let Some(base_dir) = &workspace.base_index_dir
        && base_dir != &workspace.index_dir
        && crate::neural_metadata::read_identity(&base_dir.join("neural_model.json")).is_err()
    {
        let metadata: WorkspaceMetadata =
            serde_json::from_slice(&std::fs::read(base_dir.join("workspace.json"))?)?;
        targets.push(Workspace {
            id: metadata.id,
            root: metadata.root,
            index_dir: base_dir.clone(),
            repo_id: None,
            base_index_dir: None,
        });
    }

    let mut actions = Vec::new();
    for target in targets {
        if !invalid_neural_identity(&target)? {
            continue;
        }
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(target.lock_path())?;
        fs2::FileExt::lock_exclusive(&lock)?;
        let _jobs = jobs::lock_job_ledger(&target)?;
        if !invalid_neural_identity(&target)? {
            continue;
        }
        anyhow::ensure!(
            !target.is_enhancing_active(),
            "cannot repair neural metadata while enhancement is active for {}; retry after it finishes",
            target.root.display()
        );
        // Keep the invalid identity until all dependent artifacts are removed,
        // so an interrupted repair remains diagnosable and retryable.
        let vector = target.vector_neural_path();
        for path in [
            vector.clone(),
            vector.with_extension("usearch.bak"),
            vector.with_extension("usearch.tmp"),
            target.neural_profile_path(),
            target.neural_backend_path(),
            target.neural_tombstones_path(),
            target.neural_tombstones_processing_path(),
            target.neural_enhanced_generation_path(),
            target.neural_model_path(),
        ] {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("remove invalid neural artifact {}", path.display())
                    });
                }
            }
        }
        actions.push(format!(
            "invalid neural metadata and derived vectors removed for {}; re-enhancement is needed",
            target.root.display()
        ));
    }
    Ok(actions)
}

fn default_findings(workspace: &Workspace, health: &WorkspaceIndexHealth) -> Vec<String> {
    if !health.issues.is_empty() {
        return health.issues.clone();
    }

    match health.state {
        WorkspaceIndexState::Healthy => {
            vec!["index is healthy".to_string()]
        }
        WorkspaceIndexState::HealthyEmpty if workspace.is_worktree() => {
            vec!["worktree has no indexed changes; its files come from the base index".to_string()]
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

    fn indexed_workspace(root: &std::path::Path, home: &std::path::Path) -> Workspace {
        unsafe { std::env::set_var("IVYGREP_HOME", home) };
        std::fs::write(root.join("lib.rs"), "pub fn answer() -> usize { 42 }\n").unwrap();
        let workspace = Workspace::resolve(root).unwrap();
        index_workspace(&workspace, create_hash_model().as_ref()).unwrap();
        workspace
    }

    #[test]
    #[serial]
    fn doctor_fix_rebuilds_after_an_interrupted_indexing_run() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let workspace = indexed_workspace(root.path(), home.path());
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        child.wait().unwrap();
        jobs::start_job(&workspace, JobKind::Indexing, "scanning", 1).unwrap();
        let mut ledger = jobs::read_job_ledger(&workspace);
        for job in &mut ledger.jobs {
            job.pid = Some(child.id());
        }
        std::fs::write(
            workspace.job_ledger_path(),
            serde_json::to_vec(&ledger).unwrap(),
        )
        .unwrap();
        std::fs::write(workspace.indexing_pid_path(), child.id().to_string()).unwrap();

        for _ in 0..2 {
            let report = inspect_and_maybe_fix(&workspace, false, false).unwrap();
            assert_eq!(report.state, WorkspaceIndexState::Unhealthy);
            assert!(!report.healthy);
        }
        let report = inspect_and_maybe_fix(&workspace, true, false).unwrap();
        assert!(report.repaired && report.healthy, "{:?}", report.findings);
        assert!(!indexing_was_interrupted(&workspace));
    }

    #[test]
    #[serial]
    fn doctor_reports_an_offline_watcher_and_a_failed_enhancement() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let workspace = indexed_workspace(root.path(), home.path());
        let report = inspect_and_maybe_fix(&workspace, false, false).unwrap();
        assert_eq!(report.findings, ["index is healthy"]);

        let mut metadata = workspace.read_metadata().unwrap().unwrap();
        metadata.watch_enabled = true;
        workspace.write_metadata(&metadata).unwrap();
        std::fs::write(
            workspace.index_dir.join(".enhancing.error"),
            "Enhancement error: no model\n\nCaused by:\n    offline\n",
        )
        .unwrap();
        let report = inspect_and_maybe_fix(&workspace, false, false).unwrap();
        assert_eq!(
            report.findings,
            [
                "the watcher is configured, but it does not run",
                "background enhancement failed: Enhancement error: no model",
            ]
        );
    }

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
                index_generation: 0,
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
                index_generation: 0,
            })
            .unwrap();
        std::fs::write(workspace.sqlite_path(), "").unwrap();
        std::fs::create_dir_all(workspace.tantivy_dir()).unwrap();
        std::fs::write(workspace.vector_path(), "").unwrap();

        let report = inspect_and_maybe_fix(&workspace, true, true).unwrap();
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

    #[test]
    #[serial]
    fn doctor_fix_removes_stale_legacy_watcher_pid() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

        std::fs::write(
            root.path().join("lib.rs"),
            "pub fn answer() -> usize { 42 }\n",
        )
        .unwrap();

        let workspace = Workspace::resolve(root.path()).unwrap();
        let model = create_hash_model();
        let _ = index_workspace(&workspace, model.as_ref()).unwrap();
        std::fs::write(workspace.watcher_pid_path(), "999999").unwrap();

        let report = inspect_and_maybe_fix(&workspace, true, true).unwrap();
        assert!(report.repaired);
        assert!(!workspace.watcher_pid_path().exists());
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.contains("removed stale legacy watcher pid file")),
            "expected stale watcher cleanup finding, got {:#?}",
            report.findings
        );
    }
}
