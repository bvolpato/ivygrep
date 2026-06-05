use criterion::{BatchSize, Criterion, SamplingMode, criterion_group, criterion_main};
use ivygrep::EMBEDDING_DIMENSIONS;
use ivygrep::chunking::chunk_source;
use ivygrep::embedding::{EmbeddingModel, HashEmbeddingModel};
use ivygrep::indexer::{enhance_workspace_hash, index_workspace};
use ivygrep::merkle::MerkleSnapshot;
use ivygrep::search::{SearchOptions, hybrid_search, literal_search};
use ivygrep::vector_store::{ScalarKind, VectorStore};
use ivygrep::workspace::Workspace;
use std::cell::OnceCell;
use std::fmt::Write as _;
use std::fs;
use std::hint::black_box;
use std::path::Path;
use std::time::{Duration, Instant};

// Criterion reports per-operation latency. These repetitions keep actual timed
// sample windows long enough for stable measurements without changing units.
const CHUNK_REPETITIONS: u32 = 8;
const SMALL_FILE_CHUNK_REPETITIONS: u32 = 256;
const HASH_EMBED_SINGLE_REPETITIONS: u32 = 1_024;
const HASH_EMBED_BATCH_REPETITIONS: u32 = 32;
const INCREMENTAL_REINDEX_REPETITIONS: u32 = 4;
const SEARCH_200_REPETITIONS: u32 = 8;
const HYBRID_SEARCH_REPETITIONS: u32 = 4;
const SIMPLE_SEARCH_REPETITIONS: u32 = 16;
const VECTOR_SEARCH_REPETITIONS: u32 = 128;
const HASH_VECTOR_BUILD_COUNT: usize = 5_000;

fn configure_benchmark_env(home: &Path) {
    unsafe {
        std::env::set_var("IVYGREP_HOME", home);
        std::env::set_var("IVYGREP_ENHANCE_MAX_LOAD_RATIO", "0");
        std::env::set_var("IVYGREP_NO_AUTOSPAWN", "1");
    }
}

fn repeated_per_op<F>(iters: u64, repetitions: u32, mut f: F) -> Duration
where
    F: FnMut(u32),
{
    let start = Instant::now();
    for _ in 0..iters {
        for rep in 0..repetitions {
            f(rep);
        }
    }
    start.elapsed() / repetitions
}

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
    configure_benchmark_env(home.path());

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
    let summary = index_workspace(&workspace, &model).unwrap();
    let enhanced = enhance_workspace_hash(&workspace, &model).unwrap();
    assert_eq!(enhanced, summary.total_chunks);
    let store = VectorStore::open_readonly(
        &workspace.vector_path(),
        EMBEDDING_DIMENSIONS,
        ScalarKind::F16,
    )
    .unwrap();
    assert_eq!(store.size(), summary.total_chunks);
    (staging, home, workspace, model)
}

fn setup_bulk_workspace(
    files: usize,
    functions_per_file: usize,
) -> (
    tempfile::TempDir,
    tempfile::TempDir,
    Workspace,
    HashEmbeddingModel,
) {
    let staging = tempfile::tempdir().unwrap();
    let ws_path = staging.path().join("workspace");
    fs::create_dir_all(&ws_path).unwrap();

    for file_idx in 0..files {
        let mut source = String::with_capacity(functions_per_file * 96);
        for fn_idx in 0..functions_per_file {
            writeln!(
                source,
                "pub fn calculate_bulk_{file_idx}_{fn_idx}(amount: f64) -> f64 {{"
            )
            .unwrap();
            writeln!(source, "    amount + {fn_idx}.0").unwrap();
            writeln!(source, "}}\n").unwrap();
        }
        fs::write(ws_path.join(format!("bulk_{file_idx}.rs")), source).unwrap();
    }

    let home = tempfile::tempdir().unwrap();
    configure_benchmark_env(home.path());

    let workspace = Workspace::resolve(&ws_path).unwrap();
    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    (staging, home, workspace, model)
}

fn setup_bulk_indexed_workspace(
    files: usize,
    functions_per_file: usize,
) -> (
    tempfile::TempDir,
    tempfile::TempDir,
    Workspace,
    HashEmbeddingModel,
) {
    let (staging, home, workspace, model) = setup_bulk_workspace(files, functions_per_file);
    let summary = index_workspace(&workspace, &model).unwrap();
    let enhanced = enhance_workspace_hash(&workspace, &model).unwrap();
    assert_eq!(enhanced, summary.total_chunks);
    let store = VectorStore::open_readonly(
        &workspace.vector_path(),
        EMBEDDING_DIMENSIONS,
        ScalarKind::F16,
    )
    .unwrap();
    assert_eq!(store.size(), summary.total_chunks);
    (staging, home, workspace, model)
}

fn bench_indexer(c: &mut Criterion) {
    let mut group = c.benchmark_group("indexer");
    group
        .sample_size(30)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(5))
        .measurement_time(Duration::from_secs(20));

    group.bench_function("index_small_workspace", |b| {
        b.iter_batched(
            || setup_workspace(500),
            |(_staging, _home, workspace, model)| {
                index_workspace(&workspace, &model).unwrap();
            },
            BatchSize::LargeInput,
        )
    });

    let incremental_fixture = OnceCell::new();
    group.bench_function("incremental_reindex_no_change", |b| {
        let (_staging, home, workspace, model) =
            incremental_fixture.get_or_init(|| setup_indexed_workspace(200));
        b.iter_custom(|iters| {
            configure_benchmark_env(home.path());
            repeated_per_op(iters, INCREMENTAL_REINDEX_REPETITIONS, |_| {
                let summary = index_workspace(workspace, model).unwrap();
                assert_eq!(summary.indexed_files, 0);
            })
        })
    });

    group.finish();
}

fn bench_bulk_indexer(c: &mut Criterion) {
    let mut group = c.benchmark_group("indexer_bulk");
    group
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(10));

    group.bench_function("fresh_index_30k_chunks", |b| {
        b.iter_batched(
            || setup_bulk_workspace(300, 100),
            |(_staging, _home, workspace, model)| {
                let summary = index_workspace(&workspace, &model).unwrap();
                assert_eq!(summary.deleted_files, 0);
                assert!(summary.total_chunks >= 25_000);
            },
            BatchSize::LargeInput,
        )
    });

    group.finish();
}

fn bench_chunking(c: &mut Criterion) {
    let mut group = c.benchmark_group("chunking");
    group
        .sample_size(30)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10));

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
    let rust_small_source = "pub fn calculate_tax(amount: f64) -> f64 {\n    amount * 0.2\n}\n";

    group.bench_function("chunk_rust_small_file", |b| {
        b.iter_custom(|iters| {
            repeated_per_op(iters, SMALL_FILE_CHUNK_REPETITIONS, |_| {
                black_box(chunk_source(
                    black_box(Path::new("small.rs")),
                    black_box(rust_small_source),
                ));
            })
        })
    });

    group.bench_function("chunk_rust_100_fns", |b| {
        b.iter_custom(|iters| {
            repeated_per_op(iters, CHUNK_REPETITIONS, |_| {
                black_box(chunk_source(
                    black_box(Path::new("bench.rs")),
                    black_box(&rust_source),
                ));
            })
        })
    });

    group.bench_function("chunk_python_100_fns", |b| {
        b.iter_custom(|iters| {
            repeated_per_op(iters, CHUNK_REPETITIONS, |_| {
                black_box(chunk_source(
                    black_box(Path::new("bench.py")),
                    black_box(&py_source),
                ));
            })
        })
    });

    group.finish();
}

fn bench_merkle(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle");
    group
        .sample_size(20)
        .warm_up_time(Duration::from_secs(5))
        .measurement_time(Duration::from_secs(15));

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
                MerkleSnapshot::build(dir.path(), false).unwrap();
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
                let snap = MerkleSnapshot::build(dir.path(), false).unwrap();
                (dir, snap)
            },
            |(dir, old)| {
                let new = MerkleSnapshot::build(dir.path(), false).unwrap();
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
    group
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10));

    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    let texts: Vec<&str> = vec![
        "pub fn calculate_tax(amount: f64) -> f64 { amount * 0.2 }",
        "def process_payment(total): return total > 0",
        "function handleRequest(req) { return req.body; }",
        "public class UserService { public User getUser(int id) { return null; } }",
    ];

    group.bench_function("hash_embed_single", |b| {
        b.iter_custom(|iters| {
            repeated_per_op(iters, HASH_EMBED_SINGLE_REPETITIONS, |rep| {
                let text = texts[rep as usize % texts.len()];
                black_box(model.embed(black_box(text)));
            })
        })
    });

    group.bench_function("hash_embed_batch_100", |b| {
        let batch: Vec<&str> = texts.iter().cycle().take(100).copied().collect();
        b.iter_custom(|iters| {
            repeated_per_op(iters, HASH_EMBED_BATCH_REPETITIONS, |_| {
                black_box(model.embed_batch(black_box(batch.as_slice())));
            })
        })
    });

    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");
    group
        .sample_size(20)
        .warm_up_time(Duration::from_secs(5))
        .measurement_time(Duration::from_secs(15));

    let fixture = OnceCell::new();
    let options = SearchOptions::default();

    group.bench_function("hybrid_search_200_files", |b| {
        let (_staging, _home, workspace, model) =
            fixture.get_or_init(|| setup_indexed_workspace(200));
        b.iter_custom(|iters| {
            repeated_per_op(iters, SEARCH_200_REPETITIONS, |_| {
                let hits = hybrid_search(
                    black_box(workspace),
                    black_box("calculate tax"),
                    Some(model as &dyn ivygrep::embedding::EmbeddingModel),
                    black_box(&options),
                )
                .unwrap();
                assert!(!hits.is_empty());
                assert!(
                    hits.iter()
                        .any(|hit| hit.sources.iter().any(|s| s == "semantic"))
                );
                black_box(hits);
            })
        })
    });

    group.bench_function("literal_search_200_files", |b| {
        let (_staging, _home, workspace, _model) =
            fixture.get_or_init(|| setup_indexed_workspace(200));
        b.iter_custom(|iters| {
            repeated_per_op(iters, SEARCH_200_REPETITIONS, |_| {
                let hits = literal_search(
                    black_box(workspace),
                    black_box("calculate_tax"),
                    black_box(&options),
                )
                .unwrap();
                assert!(!hits.is_empty());
                black_box(hits);
            })
        })
    });

    group.finish();
}

fn bench_regex_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("regex_search");
    group
        .sample_size(20)
        .warm_up_time(Duration::from_secs(5))
        .measurement_time(Duration::from_secs(15));

    let fixture = OnceCell::new();
    group.bench_function("regex_200_files", |b| {
        let (_staging, _home, workspace, _model) =
            fixture.get_or_init(|| setup_indexed_workspace(200));
        b.iter_custom(|iters| {
            repeated_per_op(iters, SEARCH_200_REPETITIONS, |_| {
                let hits = ivygrep::regex_search::regex_search(
                    black_box(workspace),
                    black_box(r"calculate_tax"),
                    Some(50),
                    None,
                    &[],
                    &[],
                    false,
                )
                .unwrap();
                assert!(!hits.is_empty());
                black_box(hits);
            })
        })
    });

    group.finish();
}

fn bench_base_search_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("base_search_patterns");
    group
        .sample_size(20)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(5))
        .measurement_time(Duration::from_secs(15));

    let fixture = OnceCell::new();
    let options = SearchOptions {
        limit: Some(20),
        ..SearchOptions::default()
    };

    group.bench_function("hybrid_simple_symbol_1000_files", |b| {
        let (_staging, _home, workspace, model) =
            fixture.get_or_init(|| setup_bulk_indexed_workspace(1_000, 8));
        b.iter_custom(|iters| {
            repeated_per_op(iters, HYBRID_SEARCH_REPETITIONS, |_| {
                let hits = hybrid_search(
                    black_box(workspace),
                    black_box("calculate_bulk_250_3"),
                    Some(model as &dyn ivygrep::embedding::EmbeddingModel),
                    black_box(&options),
                )
                .unwrap();
                assert!(!hits.is_empty());
                black_box(hits);
            })
        })
    });

    group.bench_function("hybrid_complex_phrase_1000_files", |b| {
        let (_staging, _home, workspace, model) =
            fixture.get_or_init(|| setup_bulk_indexed_workspace(1_000, 8));
        b.iter_custom(|iters| {
            repeated_per_op(iters, HYBRID_SEARCH_REPETITIONS, |_| {
                let hits = hybrid_search(
                    black_box(workspace),
                    black_box("calculate bulk amount"),
                    Some(model as &dyn ivygrep::embedding::EmbeddingModel),
                    black_box(&options),
                )
                .unwrap();
                assert!(!hits.is_empty());
                black_box(hits);
            })
        })
    });

    group.bench_function("literal_simple_symbol_1000_files", |b| {
        let (_staging, _home, workspace, _model) =
            fixture.get_or_init(|| setup_bulk_indexed_workspace(1_000, 8));
        b.iter_custom(|iters| {
            repeated_per_op(iters, SIMPLE_SEARCH_REPETITIONS, |_| {
                let hits = literal_search(
                    black_box(workspace),
                    black_box("calculate_bulk_250_3"),
                    black_box(&options),
                )
                .unwrap();
                assert!(!hits.is_empty());
                black_box(hits);
            })
        })
    });

    group.bench_function("regex_symbol_1000_files", |b| {
        let (_staging, _home, workspace, _model) =
            fixture.get_or_init(|| setup_bulk_indexed_workspace(1_000, 8));
        b.iter_custom(|iters| {
            repeated_per_op(iters, SIMPLE_SEARCH_REPETITIONS, |_| {
                let hits = ivygrep::regex_search::regex_search(
                    black_box(workspace),
                    black_box(r"calculate_bulk_250_\d+"),
                    Some(20),
                    None,
                    &[],
                    &[],
                    false,
                )
                .unwrap();
                assert!(!hits.is_empty());
                black_box(hits);
            })
        })
    });

    group.finish();
}

fn bench_vector_store(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_store");
    group
        .sample_size(30)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(5))
        .measurement_time(Duration::from_secs(15));

    group.bench_function("upsert_1000_vectors", |b| {
        b.iter_batched(
            || {
                let dir = tempfile::tempdir().unwrap();
                let vectors: Vec<(u64, Vec<f32>)> = (0..1000)
                    .map(|i| {
                        let mut v = vec![0.0f32; EMBEDDING_DIMENSIONS];
                        v[i % EMBEDDING_DIMENSIONS] = 1.0;
                        (i as u64, v)
                    })
                    .collect();
                (dir, vectors)
            },
            |(dir, vectors)| {
                let mut store = ivygrep::vector_store::VectorStore::open(
                    &dir.path().join("bench.usearch"),
                    EMBEDDING_DIMENSIONS,
                    ivygrep::vector_store::ScalarKind::F32,
                )
                .unwrap();
                for (key, vec) in vectors {
                    store.upsert(key, vec).unwrap();
                }
                store.save().unwrap();
            },
            BatchSize::LargeInput,
        )
    });

    let search_fixture = OnceCell::new();
    group.bench_function("search_in_1000_vectors", |b| {
        let (_search_dir, search_path, query) = search_fixture.get_or_init(|| {
            let search_dir = tempfile::tempdir().unwrap();
            let mut search_store = ivygrep::vector_store::VectorStore::open(
                &search_dir.path().join("bench.usearch"),
                EMBEDDING_DIMENSIONS,
                ivygrep::vector_store::ScalarKind::F32,
            )
            .unwrap();
            for i in 0..1000u64 {
                let mut v = vec![0.0f32; EMBEDDING_DIMENSIONS];
                v[(i as usize) % EMBEDDING_DIMENSIONS] = 1.0;
                search_store.upsert(i, v).unwrap();
            }
            search_store.save().unwrap();
            drop(search_store);
            let search_path = search_dir.path().join("bench.usearch");
            let mut query = vec![0.0f32; EMBEDDING_DIMENSIONS];
            query[0] = 1.0;
            (search_dir, search_path, query)
        });

        b.iter_custom(|iters| {
            repeated_per_op(iters, VECTOR_SEARCH_REPETITIONS, |_| {
                let store = ivygrep::vector_store::VectorStore::open_readonly(
                    black_box(search_path),
                    EMBEDDING_DIMENSIONS,
                    ivygrep::vector_store::ScalarKind::F32,
                )
                .unwrap();
                let results = store.search(black_box(query), 10);
                assert!(!results.is_empty());
                black_box(results);
            })
        })
    });

    group.finish();
}

fn bench_hash_vector_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_vector_build");
    group
        .sample_size(10)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(5));

    let model = HashEmbeddingModel::new(EMBEDDING_DIMENSIONS);
    let vectors = (0..HASH_VECTOR_BUILD_COUNT)
        .map(|i| {
            let text = format!(
                "pub fn handle_request_{i}() {{ retry request with timeout and error recovery }}",
            );
            (i as u64, model.embed(&text))
        })
        .collect::<Vec<_>>();

    group.bench_function("ingest_5k_hash_vectors", |b| {
        b.iter_batched(
            || (tempfile::tempdir().unwrap(), vectors.clone()),
            |(dir, vectors)| {
                let mut store = ivygrep::vector_store::VectorStore::open(
                    &dir.path().join("hash.usearch"),
                    EMBEDDING_DIMENSIONS,
                    ivygrep::vector_store::ScalarKind::F16,
                )
                .unwrap();
                store.reserve_additional(vectors.len()).unwrap();
                for (key, vector) in vectors {
                    store.add_unchecked(key, vector).unwrap();
                }
                assert_eq!(store.size(), HASH_VECTOR_BUILD_COUNT);
                black_box(store);
            },
            BatchSize::LargeInput,
        )
    });

    group.finish();
}

/// Critical-journey performance: incremental update cost on a large index, and
/// ANN search at a more realistic vector-store scale.
fn bench_critical_journeys(c: &mut Criterion) {
    let mut group = c.benchmark_group("critical_journeys");
    group
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(10));

    // Incremental update latency: a single changed file in a ~10K-chunk index
    // should cost ~one file's work, not a function of total index size.
    let incremental_fixture = OnceCell::new();
    let mut counter = 0u64;
    group.bench_function("incremental_one_file_change_10k_chunks", |b| {
        let (_staging, _home, workspace, model) =
            incremental_fixture.get_or_init(|| setup_bulk_indexed_workspace(200, 50));
        let target = workspace.root.join("bulk_0.rs");
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _ in 0..iters {
                counter += 1;
                fs::write(
                    &target,
                    format!("pub fn changed_{counter}() -> u64 {{ {counter} }}\n"),
                )
                .unwrap();
                let summary = index_workspace(workspace, model).unwrap();
                assert_eq!(summary.indexed_files, 1);
                black_box(summary);
            }
            start.elapsed()
        })
    });

    // ANN search at scale: 50K pseudo-random vectors (vs the 1K micro-bench),
    // enough to exercise usearch HNSW behaviour rather than a trivial set.
    let ann_fixture = OnceCell::new();
    group.bench_function("vector_search_in_50k", |b| {
        let (_ann_dir, ann_path, query) = ann_fixture.get_or_init(|| {
            let ann_dir = tempfile::tempdir().unwrap();
            let ann_path = ann_dir.path().join("ann.usearch");
            {
                let mut store = ivygrep::vector_store::VectorStore::open(
                    &ann_path,
                    EMBEDDING_DIMENSIONS,
                    ivygrep::vector_store::ScalarKind::F32,
                )
                .unwrap();
                for i in 0..50_000u64 {
                    let v: Vec<f32> = (0..EMBEDDING_DIMENSIONS)
                        .map(|j| (((i as usize * 31 + j * 17) % 97) as f32) / 97.0)
                        .collect();
                    store.upsert(i, v).unwrap();
                }
                store.save().unwrap();
            }
            let query: Vec<f32> = (0..EMBEDDING_DIMENSIONS)
                .map(|j| ((j * 13 % 97) as f32) / 97.0)
                .collect();
            (ann_dir, ann_path, query)
        });

        b.iter_custom(|iters| {
            let start = Instant::now();
            for _ in 0..iters {
                let store = ivygrep::vector_store::VectorStore::open_readonly(
                    black_box(ann_path),
                    EMBEDDING_DIMENSIONS,
                    ivygrep::vector_store::ScalarKind::F32,
                )
                .unwrap();
                let results = store.search(black_box(query), 50);
                assert!(!results.is_empty());
                black_box(results);
            }
            start.elapsed()
        })
    });

    group.bench_function("vector_search_in_50k_hot", |b| {
        let (_ann_dir, ann_path, query) = ann_fixture.get_or_init(|| {
            let ann_dir = tempfile::tempdir().unwrap();
            let ann_path = ann_dir.path().join("ann.usearch");
            {
                let mut store =
                    VectorStore::open(&ann_path, EMBEDDING_DIMENSIONS, ScalarKind::F32).unwrap();
                for i in 0..50_000u64 {
                    let v: Vec<f32> = (0..EMBEDDING_DIMENSIONS)
                        .map(|j| (((i as usize * 31 + j * 17) % 97) as f32) / 97.0)
                        .collect();
                    store.upsert(i, v).unwrap();
                }
                store.save().unwrap();
            }
            let query: Vec<f32> = (0..EMBEDDING_DIMENSIONS)
                .map(|j| ((j * 13 % 97) as f32) / 97.0)
                .collect();
            (ann_dir, ann_path, query)
        });
        let store =
            VectorStore::open_readonly(ann_path, EMBEDDING_DIMENSIONS, ScalarKind::F32).unwrap();

        b.iter_custom(|iters| {
            repeated_per_op(iters, VECTOR_SEARCH_REPETITIONS, |_| {
                let results = store.search(black_box(query), 50);
                assert!(!results.is_empty());
                black_box(results);
            })
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_indexer,
    bench_bulk_indexer,
    bench_chunking,
    bench_merkle,
    bench_embedding,
    bench_search,
    bench_regex_search,
    bench_base_search_patterns,
    bench_vector_store,
    bench_hash_vector_build,
    bench_critical_journeys,
);
criterion_main!(benches);
