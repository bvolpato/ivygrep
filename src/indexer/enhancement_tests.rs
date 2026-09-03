#[cfg(unix)]
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serial_test::serial;
use tempfile::tempdir;

use super::*;
use crate::embedding::HashEmbeddingModel;
use crate::vector_store::VectorTier;

// Run a storage mutation exactly while the enhancer computes a batch. The
// callback also proves lexical mutation does not wait for model inference.
struct MutatingModel<F> {
    mutation: Mutex<Option<F>>,
    hash: HashEmbeddingModel,
    processed: AtomicUsize,
    mutate_at: usize,
}

impl<F: FnOnce() + Send> EmbeddingModel for MutatingModel<F> {
    fn dimensions(&self) -> usize {
        self.hash.dimensions()
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        self.hash.embed(text)
    }

    fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        let processed = self.processed.fetch_add(texts.len(), Ordering::Relaxed) + texts.len();
        if processed >= self.mutate_at
            && let Some(mutation) = self.mutation.lock().unwrap().take()
        {
            mutation();
        }
        self.hash.embed_batch(texts)
    }

    fn backend_info(&self) -> Option<&'static str> {
        Some("test enhancement backend")
    }
}

fn model_with_mutation<F: FnOnce() + Send>(mutation: F) -> MutatingModel<F> {
    MutatingModel {
        mutation: Mutex::new(Some(mutation)),
        hash: HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS),
        processed: AtomicUsize::new(0),
        mutate_at: 1,
    }
}

fn enhance(workspace: &Workspace, model: &dyn EmbeddingModel, neural: bool) -> Result<usize> {
    if neural {
        enhance_workspace_neural(workspace, model)
    } else {
        enhance_workspace_hash(workspace, model)
    }
}

#[cfg(unix)]
fn derived_artifacts(workspace: &Workspace) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    [
        workspace.vector_path(),
        workspace.overlay_vector_path(),
        workspace.vector_neural_path(),
        workspace.hash_tombstones_path(),
        workspace.hash_tombstones_processing_path(),
        workspace.neural_tombstones_path(),
        workspace.neural_tombstones_processing_path(),
        workspace.hash_enhanced_generation_path(),
        workspace.neural_enhanced_generation_path(),
        workspace.neural_model_path(),
        workspace.neural_profile_path(),
        workspace.neural_backend_path(),
        workspace.enhancing_phase_path(),
        workspace.enhancing_progress_path(),
        workspace.enhancing_paused_path(),
    ]
    .into_iter()
    .map(|path| {
        let bytes = fs::read(&path).ok();
        (path, bytes)
    })
    .collect()
}

#[test]
#[cfg(unix)]
#[serial]
fn enhancement_rejects_recreated_index_even_when_generation_repeats() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    for neural in [false, true] {
        // One row exercises the final tail publication; 2050 exercises a full
        // batch before publication/checkpoint. Both use real SQLite and USearch.
        for chunks in [1, 2050] {
            let root = tempdir().unwrap();
            let source = root.path().join("lib.rs");
            fs::write(
                &source,
                (0..chunks)
                    .map(|i| format!("pub fn obsolete_{i}() {{}}\n"))
                    .collect::<String>(),
            )
            .unwrap();
            let workspace = Workspace::resolve(root.path()).unwrap();
            let hash = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
            index_workspace(&workspace, &hash).unwrap();
            let original_generation = workspace.read_metadata().unwrap().unwrap().index_generation;
            let original_incarnation = workspace.read_index_incarnation().unwrap();
            // A claimed journal must not be deleted from the replacement.
            let processing = if neural {
                workspace.neural_tombstones_processing_path()
            } else {
                workspace.hash_tombstones_processing_path()
            };
            fs::write(&processing, "123\n").unwrap();
            let replacement = Mutex::new(None);
            let model = model_with_mutation(|| {
                // The worker lock survives removal and continues to serialize
                // other workers while the index lock remains available.
                let worker_lock = fs::OpenOptions::new()
                    .write(true)
                    .open(workspace.index_dir.join("enhancement.lock"))
                    .unwrap();
                assert!(fs2::FileExt::try_lock_exclusive(&worker_lock).is_err());
                remove_workspace_index(&workspace).unwrap();
                fs::write(&source, "pub fn replacement_current() {}\n").unwrap();
                index_workspace(&workspace, &hash).unwrap();
                assert_eq!(
                    workspace.read_metadata().unwrap().unwrap().index_generation,
                    original_generation
                );
                assert_ne!(
                    workspace.read_index_incarnation().unwrap(),
                    original_incarnation
                );
                let replacement_lock = fs::OpenOptions::new()
                    .write(true)
                    .open(workspace.index_dir.join("enhancement.lock"))
                    .unwrap();
                assert!(fs2::FileExt::try_lock_exclusive(&replacement_lock).is_err());
                fs::write(&processing, "456\n").unwrap();
                fs::write(workspace.enhancing_progress_path(), "replacement progress").unwrap();
                *replacement.lock().unwrap() = Some(derived_artifacts(&workspace));
            });
            let error = enhance(&workspace, &model, neural).unwrap_err();
            assert!(error.is::<EnhancementSuperseded>(), "{error:#}");
            assert_eq!(
                derived_artifacts(&workspace),
                replacement.into_inner().unwrap().unwrap()
            );
            // A subsequent current worker can consume the replacement journal
            // and embed its current key, with no obsolete vectors left behind.
            assert_eq!(enhance(&workspace, &hash, neural).unwrap(), 1);
            assert_current_vector_keys(&workspace, neural);
        }
    }
}

fn assert_current_vector_keys(workspace: &Workspace, neural: bool) {
    let overlay = workspace.has_overlay();
    let sqlite = open_sqlite_readonly(&if overlay {
        workspace.overlay_sqlite_path()
    } else {
        workspace.sqlite_path()
    })
    .unwrap();
    let keys: Vec<u64> = sqlite
        .prepare("SELECT DISTINCT vector_key FROM chunks")
        .unwrap()
        .query_map([], |row| Ok(row.get::<_, i64>(0)? as u64))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let path = if neural {
        workspace.vector_neural_path()
    } else if overlay {
        workspace.overlay_vector_path()
    } else {
        workspace.vector_path()
    };
    let store = VectorStore::open_readonly(
        &path,
        crate::EMBEDDING_DIMENSIONS,
        HASH_VECTOR_QUANTIZATION,
        if neural {
            VectorTier::Neural
        } else {
            VectorTier::Hash
        },
    )
    .unwrap();
    assert_eq!(store.size(), keys.len());
    assert!(keys.iter().all(|key| store.contains(*key)));
}

#[test]
#[serial]
fn enhancement_allows_incremental_indexing_and_drains_its_tombstones() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    for neural in [false, true] {
        let root = tempdir().unwrap();
        let source = root.path().join("lib.rs");
        fs::write(&source, "pub fn initial_chunk() {}\n").unwrap();
        let workspace = Workspace::resolve(root.path()).unwrap();
        let hash = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &hash).unwrap();
        let incarnation = workspace.read_index_incarnation().unwrap();
        let model = model_with_mutation(|| {
            fs::write(&source, "pub fn changed_during_embedding() {}\n").unwrap();
            index_workspace(&workspace, &hash).unwrap();
            assert_eq!(workspace.read_index_incarnation().unwrap(), incarnation);
        });
        assert_eq!(enhance(&workspace, &model, neural).unwrap(), 1);
        let completion = if neural {
            workspace.neural_enhanced_generation_path()
        } else {
            workspace.hash_enhanced_generation_path()
        };
        assert!(
            !completion.exists(),
            "changed generation must remain incomplete"
        );
        assert_eq!(enhance(&workspace, &hash, neural).unwrap(), 1);
        assert_current_vector_keys(&workspace, neural);
    }
}

#[test]
#[cfg(unix)]
#[serial]
fn enhancement_rejects_recreated_overlay() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    for neural in [false, true] {
        let parent = tempdir().unwrap();
        let (_, linked) = super::overlay_tests::linked_workspace(parent.path());
        let workspace = Workspace::resolve(&linked).unwrap();
        let hash = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &hash).unwrap();
        let incarnation = workspace.read_index_incarnation().unwrap();
        let replacement = Mutex::new(None);
        let model = model_with_mutation(|| {
            fs::write(
                linked.join("lib.rs"),
                "pub fn replacement_overlay_chunk() {}\n",
            )
            .unwrap();
            index_workspace_with_options(&workspace, &hash, false, None, true).unwrap();
            assert_ne!(workspace.read_index_incarnation().unwrap(), incarnation);
            *replacement.lock().unwrap() = Some(derived_artifacts(&workspace));
        });
        let error = enhance(&workspace, &model, neural).unwrap_err();
        assert!(error.is::<EnhancementSuperseded>(), "{error:#}");
        assert_eq!(
            derived_artifacts(&workspace),
            replacement.into_inner().unwrap().unwrap()
        );
        enhance(&workspace, &hash, neural).unwrap();
        assert_current_vector_keys(&workspace, neural);
    }
}

#[test]
#[cfg(unix)]
#[serial]
fn enhancement_rejects_staged_main_recovery() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    for neural in [false, true] {
        let root = tempdir().unwrap();
        let source = root.path().join("lib.rs");
        fs::write(&source, "pub fn old_main_chunk() {}\n").unwrap();
        let workspace = Workspace::resolve(root.path()).unwrap();
        let hash = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
        index_workspace(&workspace, &hash).unwrap();
        let incarnation = workspace.read_index_incarnation().unwrap();
        let replacement = Mutex::new(None);
        let model = model_with_mutation(|| {
            // Deep storage validation, rather than removal, triggers a staged
            // rebuild. The generation changes but the old worker still must
            // not publish into its replacement or remove replacement journals.
            fs::write(workspace.tantivy_dir().join("meta.json"), "{").unwrap();
            fs::write(&source, "pub fn new_main_recovery_chunk() {}\n").unwrap();
            index_workspace(&workspace, &hash).unwrap();
            assert_ne!(workspace.read_index_incarnation().unwrap(), incarnation);
            *replacement.lock().unwrap() = Some(derived_artifacts(&workspace));
        });
        let error = enhance(&workspace, &model, neural).unwrap_err();
        assert!(error.is::<EnhancementSuperseded>(), "{error:#}");
        assert_eq!(
            derived_artifacts(&workspace),
            replacement.into_inner().unwrap().unwrap()
        );
        enhance(&workspace, &hash, neural).unwrap();
        assert_current_vector_keys(&workspace, neural);
    }
}

#[test]
#[cfg(unix)]
#[serial]
fn enhancement_rejects_neural_checkpoint_after_replacement() {
    let home = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let root = tempdir().unwrap();
    for file in 0..65 {
        fs::write(
            root.path().join(format!("batch_{file}.rs")),
            (0..256)
                .map(|row| format!("pub fn checkpoint_old_{file}_{row}() {{}}\n"))
                .collect::<String>(),
        )
        .unwrap();
    }
    let workspace = Workspace::resolve(root.path()).unwrap();
    let hash = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
    assert_eq!(
        index_workspace(&workspace, &hash).unwrap().total_chunks,
        16_640
    );
    let replacement = Mutex::new(None);
    let mut model = model_with_mutation(|| {
        assert!(
            !workspace.vector_neural_path().exists(),
            "first checkpoint has not published yet"
        );
        remove_workspace_index(&workspace).unwrap();
        for file in 0..65 {
            fs::remove_file(root.path().join(format!("batch_{file}.rs"))).unwrap();
        }
        fs::write(
            root.path().join("lib.rs"),
            "pub fn checkpoint_replacement() {}\n",
        )
        .unwrap();
        index_workspace(&workspace, &hash).unwrap();
        *replacement.lock().unwrap() = Some(derived_artifacts(&workspace));
    });
    model.mutate_at = 16_384;
    let error = enhance_workspace_neural(&workspace, &model).unwrap_err();
    assert!(error.is::<EnhancementSuperseded>(), "{error:#}");
    assert_eq!(
        derived_artifacts(&workspace),
        replacement.into_inner().unwrap().unwrap()
    );
    assert_eq!(enhance_workspace_neural(&workspace, &hash).unwrap(), 1);
    assert_current_vector_keys(&workspace, true);
}

#[test]
#[serial]
fn enhancement_without_an_index_is_a_noop() {
    let home = tempdir().unwrap();
    let root = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let workspace = Workspace::resolve(root.path()).unwrap();
    assert!(!workspace.index_dir.exists());
    let hash = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
    assert_eq!(enhance_workspace_hash(&workspace, &hash).unwrap(), 0);
    assert_eq!(enhance_workspace_neural(&workspace, &hash).unwrap(), 0);
    assert!(!workspace.index_dir.exists());
}

#[test]
#[serial]
fn enhancement_job_cannot_switch_incarnation_between_stages() {
    let home = tempdir().unwrap();
    let root = tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };
    let source = root.path().join("lib.rs");
    fs::write(&source, "pub fn first_job_incarnation() {}\n").unwrap();
    let workspace = Workspace::resolve(root.path()).unwrap();
    let hash = HashEmbeddingModel::new(crate::EMBEDDING_DIMENSIONS);
    index_workspace(&workspace, &hash).unwrap();
    let (job, setup_lock) = EnhancementSnapshot::begin(&workspace).unwrap();
    drop(setup_lock);
    enhance_workspace_hash_for_job(&workspace, &hash, &job).unwrap();
    // The worker can spend a long time loading a model or waiting for another
    // enhancer between stages. A replacement must cancel the whole old job.
    remove_workspace_index(&workspace).unwrap();
    fs::write(&source, "pub fn second_job_incarnation() {}\n").unwrap();
    index_workspace(&workspace, &hash).unwrap();
    let vector_before = fs::read(workspace.vector_path()).unwrap();
    for result in [
        enhance_workspace_hash_for_job(&workspace, &hash, &job),
        enhance_workspace_neural_for_job(&workspace, &hash, &job),
    ] {
        assert!(result.unwrap_err().is::<EnhancementSuperseded>());
    }
    assert_eq!(fs::read(workspace.vector_path()).unwrap(), vector_before);
    assert!(!workspace.vector_neural_path().exists());
    assert!(!workspace.neural_profile_path().exists());
    assert!(!workspace.enhancing_phase_path().exists());
    assert!(!workspace.enhancing_progress_path().exists());
    assert!(
        job.lock_current(&workspace).is_err(),
        "old job cannot clean up its replacement"
    );
}
