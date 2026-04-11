use std::path::PathBuf;

use anyhow::Result;
use serde::Serialize;

use crate::embedding::create_hash_model;
use crate::indexer::{index_workspace, remove_workspace_index};
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
    DoctorReport::from_health(workspace, health, false, Vec::new())
}

pub fn inspect_and_maybe_fix(workspace: &Workspace, fix: bool) -> Result<DoctorReport> {
    let initial = workspace.index_health();
    if !fix {
        return Ok(DoctorReport::from_health(
            workspace,
            initial,
            false,
            Vec::new(),
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
    let mut findings = default_findings(&repaired);
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
}
