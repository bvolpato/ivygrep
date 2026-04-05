use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use ivygrep::EMBEDDING_DIMENSIONS;
use ivygrep::chunking::chunk_source;
use ivygrep::embedding::{EmbeddingModel, HashEmbeddingModel};
use ivygrep::indexer::index_workspace;
use ivygrep::merkle::MerkleSnapshot;
use ivygrep::search::{SearchOptions, hybrid_search, literal_search};
use ivygrep::workspace::Workspace;
use std::fs;
use std::path::Path;

/// Create a temp workspace with `n` small Rust files and return handles.
fn setup_workspace(
    n: usize,
) -> (
    tempfile::TempDir,
    tempfile::TempDir,
    Workspace,
    HashEmbeddingModel,
) {
    let staging = tempfile::tempdir().unwrap();
    let ws_path = staging.path().join("workspace");
    fs::create_dir_all(&ws_path).unwrap();

    for i in 0..n {
        fs::write(
            ws_path.join(format!("file_{}.rs", i)),
            format!(
                "/// Module {i} handles tax calculations\n\
                 pub fn calculate_tax_{i}(amount: f64) -> f64 {{\n\
                     amount * 0.{i}\n\
                 }}\n\n\
                 pub fn process_payment_{i}(total: f64) -> bool {{\n\
                     total > 0.0\n\
                 }}\n"
            ),
        )
        .unwrap();
    }

    let home = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("IVYGREP_HOME", home.path()) };

    let workspace = Workspace::resolve(&ws_path).unwrap();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    (staging, home, workspace, model)
}

/// Create and index a workspace, returning it ready for searching.
fn setup_indexed_workspace(
    n: usize,
) -> (
    tempfile::TempDir,
    tempfile::TempDir,
    Workspace,
    HashEmbeddingModel,
) {
    let (staging, home, workspace, model) = setup_workspace(n);
    index_workspace(&workspace, &model).unwrap();
    (staging, home, workspace, model)
}

fn bench_indexer(c: &mut Criterion) {
    let mut group = c.benchmark_group("indexer");
    group.sample_size(10);

    group.bench_function("index_small_workspace", |b| {
        b.iter_batched(
            || setup_workspace(500),
            |(_staging, _home, workspace, model)| {
                index_workspace(&workspace, &model).unwrap();
            },
            BatchSize::LargeInput,
        )
    });

    group.bench_function("incremental_reindex_no_change", |b| {
        b.iter_batched(
            || setup_indexed_workspace(200),
            |(_staging, _home, workspace, model)| {
                let summary = index_workspace(&workspace, &model).unwrap();
                assert_eq!(summary.indexed_files, 0);
            },
            BatchSize::LargeInput,
        )
    });

    group.finish();
}

fn bench_chunking(c: &mut Criterion) {
    let mut group = c.benchmark_group("chunking");
    group.sample_size(20);

    let rust_source = (0..100)
        .map(|i| {
            format!(
                "pub fn handler_{i}(req: Request) -> Response {{\n\
                     let data = req.parse();\n\
                     Response::ok(data)\n\
                 }}\n\n"
            )
        })
        .collect::<String>();

    let py_source = (0..100)
        .map(|i| {
            format!(
                "def process_{i}(items):\n\
                     return [x * 2 for x in items]\n\n"
            )
        })
        .collect::<String>();

    group.bench_function("chunk_rust_100_fns", |b| {
        b.iter(|| chunk_source(Path::new("bench.rs"), &rust_source))
    });

    group.bench_function("chunk_python_100_fns", |b| {
        b.iter(|| chunk_source(Path::new("bench.py"), &py_source))
    });

    group.finish();
}

fn bench_merkle(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle");
    group.sample_size(10);

    group.bench_function("scan_500_files", |b| {
        b.iter_batched(
            || {
                let dir = tempfile::tempdir().unwrap();
                for i in 0..500 {
                    fs::write(
                        dir.path().join(format!("file_{}.rs", i)),
                        format!("fn f_{i}() {{}}\n"),
                    )
                    .unwrap();
                }
                dir
            },
            |dir| {
                MerkleSnapshot::build(dir.path()).unwrap();
            },
            BatchSize::LargeInput,
        )
    });

    group.bench_function("diff_500_files_no_change", |b| {
        b.iter_batched(
            || {
                let dir = tempfile::tempdir().unwrap();
                for i in 0..500 {
                    fs::write(
                        dir.path().join(format!("file_{}.rs", i)),
                        format!("fn f_{i}() {{}}\n"),
                    )
                    .unwrap();
                }
                let snap = MerkleSnapshot::build(dir.path()).unwrap();
                (dir, snap)
            },
            |(dir, old)| {
                let new = MerkleSnapshot::build(dir.path()).unwrap();
                let diff = old.diff(&new);
                assert!(diff.added_or_modified.is_empty());
            },
            BatchSize::LargeInput,
        )
    });

    group.finish();
}

fn bench_embedding(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedding");

    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    let texts: Vec<&str> = vec![
        "pub fn calculate_tax(amount: f64) -> f64 { amount * 0.2 }",
        "def process_payment(total): return total > 0",
        "function handleRequest(req) { return req.body; }",
        "public class UserService { public User getUser(int id) { return null; } }",
    ];

    group.bench_function("hash_embed_single", |b| b.iter(|| model.embed(texts[0])));

    group.bench_function("hash_embed_batch_100", |b| {
        let batch: Vec<&str> = texts.iter().cycle().take(100).copied().collect();
        b.iter(|| model.embed_batch(&batch))
    });

    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");
    group.sample_size(10);

    group.bench_function("hybrid_search_200_files", |b| {
        b.iter_batched(
            || setup_indexed_workspace(200),
            |(_staging, _home, workspace, model)| {
                let hits = hybrid_search(
                    &workspace,
                    "calculate tax",
                    Some(&model as &dyn ivygrep::embedding::EmbeddingModel),
                    &SearchOptions::default(),
                )
                .unwrap();
                assert!(!hits.is_empty());
            },
            BatchSize::LargeInput,
        )
    });

    group.bench_function("literal_search_200_files", |b| {
        b.iter_batched(
            || setup_indexed_workspace(200),
            |(_staging, _home, workspace, _model)| {
                let hits =
                    literal_search(&workspace, "calculate_tax", &SearchOptions::default()).unwrap();
                assert!(!hits.is_empty());
            },
            BatchSize::LargeInput,
        )
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_indexer,
    bench_chunking,
    bench_merkle,
    bench_embedding,
    bench_search,
);
criterion_main!(benches);
