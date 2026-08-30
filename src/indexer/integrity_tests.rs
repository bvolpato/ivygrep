use std::ffi::OsString;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use serial_test::serial;
use tempfile::tempdir;

use crate::embedding::HashEmbeddingModel;
use crate::search::{SearchContext, SearchOptions, literal_search};

use super::*;

fn git(root: &Path, args: &[&str]) {
    let result = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[serial]
fn unchanged_sources_repair_invalid_stores_before_noop() {
    for (use_git, live_watcher) in [(false, false), (true, false), (false, true)] {
        for damage in ["hash", "tantivy", "empty_tantivy"] {
            let home = tempdir().unwrap();
            unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
            let root = tempdir().unwrap();
            fs::write(root.path().join("a.rs"), "pub fn retained_alpha() {}\n").unwrap();
            fs::write(root.path().join("b.rs"), "pub fn retained_beta() {}\n").unwrap();
            if use_git {
                git(root.path(), &["init", "-q"]);
                git(root.path(), &["config", "core.autocrlf", "false"]);
                git(root.path(), &["add", "."]);
                git(
                    root.path(),
                    &[
                        "-c",
                        "user.name=Fixture",
                        "-c",
                        "user.email=fixture@example.invalid",
                        "-c",
                        "commit.gpgsign=false",
                        "commit",
                        "-qm",
                        "fixture",
                    ],
                );
            }
            let workspace = Workspace::resolve(root.path()).unwrap();
            let model = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
            index_workspace(&workspace, &model).unwrap();
            if live_watcher {
                jobs::start_job(&workspace, JobKind::Watcher, "watching", 1).unwrap();
                assert!(workspace.is_watcher_alive());
            }
            let before = workspace.read_metadata().unwrap().unwrap();
            let snapshot = fs::read(workspace.merkle_snapshot_path()).unwrap();
            match damage {
                "hash" => fs::write(workspace.vector_path(), "invalid vector header").unwrap(),
                "tantivy" => fs::write(workspace.tantivy_dir().join("meta.json"), "{").unwrap(),
                _ => {
                    let (index, _) = open_tantivy_index(&workspace.tantivy_dir()).unwrap();
                    let mut writer = index
                        .writer_with_num_threads::<TantivyDocument>(1, 50_000_000)
                        .unwrap();
                    writer.delete_all_documents().unwrap();
                    writer.commit().unwrap();
                    writer.wait_merging_threads().unwrap();
                }
            }
            let recovered = index_workspace(&workspace, &model).unwrap();
            assert_eq!(
                recovered.indexed_files, 2,
                "{damage}, git={use_git}, watcher={live_watcher}"
            );
            assert_eq!(recovered.total_chunks, 2);
            assert!(workspace.index_health().is_queryable());
            assert_eq!(
                fs::read(workspace.merkle_snapshot_path()).unwrap(),
                snapshot
            );
            let after = workspace.read_metadata().unwrap().unwrap();
            assert_eq!(after.index_generation, before.index_generation + 1);
            for term in ["retained_alpha", "retained_beta"] {
                assert_eq!(
                    literal_search(&workspace, term, &SearchOptions::default())
                        .unwrap()
                        .len(),
                    1
                );
            }
            assert_eq!(
                index_workspace(&workspace, &model).unwrap().indexed_files,
                0
            );
            assert_eq!(
                workspace.read_metadata().unwrap().unwrap().index_generation,
                after.index_generation
            );
        }
    }
}

#[test]
#[serial]
fn invalid_optional_identity_does_not_disable_search_and_repairs_without_reindex() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let root = tempdir().unwrap();
    fs::write(
        root.path().join("source.rs"),
        "pub fn optional_identity_marker() {}\n",
    )
    .unwrap();
    let workspace = Workspace::resolve(root.path()).unwrap();
    let model = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
    index_workspace(&workspace, &model).unwrap();
    enhance_workspace_hash(&workspace, &model).unwrap();
    enhance_workspace_neural(&workspace, &model).unwrap();
    let primary = [
        workspace.sqlite_path(),
        workspace.tantivy_dir().join("meta.json"),
        workspace.vector_path(),
        workspace.merkle_snapshot_path(),
        workspace.metadata_path(),
    ]
    .map(|path| {
        let bytes = fs::read(&path).unwrap();
        (path, bytes)
    });
    fs::write(workspace.neural_model_path(), "{\n").unwrap();
    assert_eq!(
        literal_search(
            &workspace,
            "optional_identity_marker",
            &SearchOptions::default()
        )
        .unwrap()
        .len(),
        1
    );
    let hash = SearchContext::load(&workspace, Some(crate::EMBEDDING_DIMENSIONS), false).unwrap();
    assert!(hash.hash_vectors.is_some());
    assert!(SearchContext::load(&workspace, Some(crate::EMBEDDING_DIMENSIONS), true).is_err());
    drop(hash);
    let report = crate::doctor::inspect_workspace(&workspace);
    assert!(!report.healthy);
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("neural model metadata"))
    );
    jobs::start_job(&workspace, JobKind::Enhancement, "neural", 1).unwrap();
    let error = crate::doctor::inspect_and_maybe_fix(&workspace, true, true).unwrap_err();
    assert!(error.to_string().contains("enhancement is active"));
    assert_eq!(fs::read(workspace.neural_model_path()).unwrap(), b"{\n");
    assert!(workspace.vector_neural_path().exists());
    jobs::finish_job(&workspace, JobKind::Enhancement, "completed", None).unwrap();
    let repaired = crate::doctor::inspect_and_maybe_fix(&workspace, true, true).unwrap();
    assert!(repaired.repaired && repaired.healthy, "{repaired:?}");
    assert!(!workspace.neural_model_path().exists());
    assert!(!workspace.vector_neural_path().exists());
    for (path, bytes) in primary {
        assert_eq!(
            fs::read(&path).unwrap(),
            bytes,
            "primary artifact changed: {}",
            path.display()
        );
    }
    assert_eq!(
        literal_search(
            &workspace,
            "optional_identity_marker",
            &SearchOptions::default()
        )
        .unwrap()
        .len(),
        1
    );
}

#[test]
#[serial]
fn unchanged_overlay_repairs_own_and_inherited_primary_stores() {
    for corrupt_base in [false, true] {
        let home = tempdir().unwrap();
        unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
        let root = tempdir().unwrap();
        let (base_root, linked_root) = super::overlay_tests::linked_workspace(root.path());
        let base = Workspace::resolve(&base_root).unwrap();
        let linked = Workspace::resolve(&linked_root).unwrap();
        let model = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        index_workspace(&base, &model).unwrap();
        index_workspace(&linked, &model).unwrap();
        let snapshot = fs::read(linked.merkle_snapshot_path()).unwrap();
        let base_generation = base.read_metadata().unwrap().unwrap().index_generation;
        let damaged = if corrupt_base {
            base.tantivy_dir()
        } else {
            linked.overlay_tantivy_dir()
        };
        fs::write(damaged.join("meta.json"), "{").unwrap();
        index_workspace(&linked, &model).unwrap();
        assert!(!linked.worktree_overlay_is_stale().unwrap());
        assert_eq!(fs::read(linked.merkle_snapshot_path()).unwrap(), snapshot);
        assert_eq!(
            base.read_metadata().unwrap().unwrap().index_generation,
            base_generation + u64::from(corrupt_base)
        );
        for term in [
            "old_overlay_marker",
            "shared_overlay_marker",
            "inherited_overlay_marker",
        ] {
            assert_eq!(
                literal_search(&linked, term, &SearchOptions::default())
                    .unwrap()
                    .len(),
                1
            );
        }
        assert_eq!(index_workspace(&linked, &model).unwrap().indexed_files, 0);
    }
}

#[test]
#[serial]
fn doctor_repairs_optional_base_identity_without_rebuilding_overlay() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let root = tempdir().unwrap();
    let (base_root, linked_root) = super::overlay_tests::linked_workspace(root.path());
    let base = Workspace::resolve(&base_root).unwrap();
    let linked = Workspace::resolve(&linked_root).unwrap();
    let model = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
    index_workspace(&base, &model).unwrap();
    index_workspace(&linked, &model).unwrap();
    enhance_workspace_hash(&base, &model).unwrap();
    enhance_workspace_neural(&base, &model).unwrap();
    fs::write(base.neural_model_path(), "{\n").unwrap();
    let base_state = super::overlay_tests::stored_state(&base, false);
    let linked_state = super::overlay_tests::stored_state(&linked, true);
    assert!(SearchContext::load(&linked, Some(crate::EMBEDDING_DIMENSIONS), false).is_ok());
    let report = crate::doctor::inspect_and_maybe_fix(&linked, true, true).unwrap();
    assert!(report.repaired && report.healthy, "{report:?}");
    assert!(!base.neural_model_path().exists());
    assert!(!base.vector_neural_path().exists());
    assert_eq!(super::overlay_tests::stored_state(&base, false), base_state);
    assert_eq!(
        super::overlay_tests::stored_state(&linked, true),
        linked_state
    );
}

struct RestoreBatch(Option<OsString>);

impl Drop for RestoreBatch {
    fn drop(&mut self) {
        unsafe {
            if let Some(value) = &self.0 {
                std::env::set_var("IVYGREP_NEURAL_BATCH_SIZE", value);
            } else {
                std::env::remove_var("IVYGREP_NEURAL_BATCH_SIZE");
            }
        }
    }
}

struct InterruptedModel(AtomicUsize);

impl EmbeddingModel for InterruptedModel {
    fn dimensions(&self) -> usize {
        8
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let hash = xxhash_rust::xxh3::xxh3_64(text.as_bytes());
        (0..8)
            .map(|offset| ((hash.rotate_left(offset * 7) & 65535) + 1) as f32 / 65536.0)
            .collect()
    }

    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        if self.0.fetch_add(1, Ordering::Relaxed) == 17 {
            return vec![vec![0.0; 8]; texts.len()];
        }
        texts.iter().map(|text| self.embed(text)).collect()
    }
}

fn assert_neural_checkpoint(batch_size: usize, expected: u64) {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let _batch = RestoreBatch(std::env::var_os("IVYGREP_NEURAL_BATCH_SIZE"));
    unsafe { std::env::set_var("IVYGREP_NEURAL_BATCH_SIZE", batch_size.to_string()) };
    let root = tempdir().unwrap();
    let workspace = Workspace::resolve(root.path()).unwrap();
    workspace.ensure_dirs().unwrap();
    let mut sqlite = open_sqlite(&workspace.sqlite_path()).unwrap();
    let tx = sqlite.transaction().unwrap();
    {
        let mut insert = tx.prepare(
            "INSERT INTO chunks (chunk_key, file_path, start_line, end_line, language, kind, text, vector_key, modified_unix, is_ignored) VALUES (?1, 'source.rs', ?1, ?1, 'rust', 'Function', ?2, ?1, 0, 0)"
        ).unwrap();
        for key in 1..=18_000_i64 {
            insert
                .execute(params![
                    key,
                    compress_text(&format!("neural checkpoint source {key}"))
                ])
                .unwrap();
        }
    }
    tx.commit().unwrap();
    drop(sqlite);
    let error =
        enhance_workspace_neural(&workspace, &InterruptedModel(AtomicUsize::new(0))).unwrap_err();
    assert!(error.to_string().contains("zero vector"), "{error:#}");
    assert_eq!(
        VectorStore::read_count(&workspace.vector_neural_path(), 8).unwrap_or(0),
        expected
    );
}

#[test]
#[serial]
fn neural_checkpoint_crosses_non_divisor_batch_boundary() {
    assert_neural_checkpoint(1000, 17_000);
}

#[test]
#[serial]
fn neural_checkpoint_preserves_divisor_batch_boundary() {
    assert_neural_checkpoint(1024, 16_384);
}
