use criterion::{criterion_group, criterion_main, Criterion, BatchSize};
use ivygrep::indexer::index_workspace;
use ivygrep::workspace::Workspace;
use ivygrep::embedding::HashEmbeddingModel;
use ivygrep::EMBEDDING_DIMENSIONS;
use std::fs;

fn bench_indexer(c: &mut Criterion) {
    let mut group = c.benchmark_group("indexer");
    // Reduce sample size so the benchmarks don't take forever
    group.sample_size(10);

    group.bench_function("index_small_workspace", |b| {
        b.iter_batched(
            || {
                let staging = tempfile::tempdir().unwrap();
                let ws_path = staging.path().join("workspace");
                fs::create_dir_all(&ws_path).unwrap();
                
                // Create a few files
                for i in 0..500 {
                    fs::write(ws_path.join(format!("file_{}.rs", i)), format!("pub fn target_{}() {{}}", i)).unwrap();
                }

                // Set up isolated HOME
                let home = tempfile::tempdir().unwrap();
                unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

                (staging, home, Workspace::resolve(&ws_path).unwrap(), HashEmbeddingModel::new(EMBEDDING_DIMENSIONS))
            },
            |(_staging, _home, workspace, model)| {
                index_workspace(&workspace, &model).unwrap();
            },
            BatchSize::LargeInput,
        )
    });

    group.finish();
}

criterion_group!(benches, bench_indexer);
criterion_main!(benches);
