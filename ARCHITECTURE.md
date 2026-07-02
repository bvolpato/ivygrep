# Architecture

> ivygrep is local hybrid code search: lexical index first, vector enrichment
> later, daemon-backed hot search, no source-code upload.

## System Shape

ivygrep has four layers:

- **CLI**: resolves workspace, starts daemon when useful, prints search/status
  results.
- **Indexer**: walks files, chunks source, writes SQLite/Tantivy, then schedules
  vector enrichment.
- **Search engine**: merges literal, BM25, path, symbol, hash-vector, and
  learned-vector candidates.
- **Daemon**: keeps watchers, search contexts, query cache, and embedding model
  state warm across CLI calls.

Design target: return useful BM25/literal results as soon as lexical indexing
commits, then improve semantic quality in background without blocking search.

## Index Lifecycle

Indexing path:

```text
resolve workspace -> scan Merkle diff -> chunk changed files
-> write lexical stores -> promote fresh index -> enrich vectors
```

1. **Workspace resolution**
   - Workspace id is `xxh3_128(canonical_root_path)`.
   - Git worktrees can use a base index plus thin overlay stores.
   - Worktree overlays record `base_ref.json` with base index generation; stale
     overlays are rebuilt before search.

2. **Change detection**
   - `ignore` walker respects `.gitignore` unless caller opts out.
   - `merkle_snapshot.json` stores metadata fingerprints, not file contents.
   - Fingerprint input is path map plus file size, mtime, and Unix/macOS ctime.
   - Live daemon watcher lets foreground search skip full Merkle scans.

3. **Chunking**
   - Tree-sitter structural chunking covers 24 AST language families:
     Rust, Python, Go, JavaScript/JSX, TypeScript/TSX, Java, C#, PHP, Ruby,
     Swift, C, C++, Scala, Kotlin, Elixir, Zig, shell, Haskell, OCaml, Lua,
     Dart, Objective-C, Perl, and Starlark.
   - Registry heuristics cover 40+ additional languages and formats.
   - Large generated Rust sources skip Tree-sitter when signature heuristics are
     cheaper and safer.
   - Rust doc includes are tracked through `included_file_dependencies` so
     dependent doc chunks refresh incrementally.

4. **Fresh index staging**
   - Full rebuilds write to `.fresh-index-staging-*`.
   - SQLite uses fast staging pragmas: `journal_mode=OFF`,
     `synchronous=OFF`, exclusive locking, larger cache, memory temp store.
   - Secondary SQLite indexes are deferred until data load finishes.
   - Finished staging stores are promoted into place; old vector file gets a
     recoverable `.usearch.bak` during replacement.
   - Incremental writes use WAL and normal sync.

5. **Vector enrichment**
   - Lexical commit publishes usable search first.
   - Background `--enhance-internal` builds `vectors.usearch` hash vectors,
     then `vectors_neural.usearch` learned vectors.
   - Tombstone journals (`.hash_tombstones`, `.neural_tombstones`) remove stale
     vector ids after foreground edits.
   - `.hash_enhanced_generation` and `.neural_enhanced_generation` record which
     lexical generation each vector store covers.

## Storage

Default index root:

```text
~/.local/share/ivygrep/
+-- daemon.log
+-- daemon.sock / daemon.port
+-- indexes/
    +-- <workspace-id>/
        +-- workspace.json
        +-- merkle_snapshot.json
        +-- index_format_version
        +-- job.json / job.lock
        +-- metadata.sqlite3
        +-- tantivy/
        +-- vectors.usearch
        +-- vectors_neural.usearch
        +-- neural_model.json
        +-- neural_profile
        +-- neural_backend
        +-- base_ref.json
        +-- overlay.sqlite3
        +-- overlay_tantivy/
        +-- overlay_vectors.usearch
        +-- .hash_tombstones
        +-- .hash_tombstones.processing
        +-- .hash_enhanced_generation
        +-- .neural_tombstones
        +-- .neural_tombstones.processing
        +-- .neural_enhanced_generation
        +-- .indexing.pid / .indexing.progress
        +-- .enhancing.pid / .enhancing.phase / .enhancing.progress
        +-- .watcher.pid
```

Model assets live outside index storage under `$HF_HOME` or
`~/.cache/huggingface`.

### SQLite

SQLite is metadata and source-text source of truth.

- `chunks`: path, line range, language, kind, compressed text, vector key,
  modified timestamp, ignored flag.
- `_stats`: cached `chunk_count`, `file_count`, and `vector_key_count`.
- `symbols`: `WITHOUT ROWID`, primary key `(normalized_name, chunk_key)`.
- `included_file_dependencies`: Rust doc include dependency edges.
- Overlay indexes add tombstones for base files hidden by worktree changes.

Chunk text is zstd-compressed when it is at least 512 bytes. Search fetches
metadata and source text separately so BM25/path/symbol candidates avoid
decompressing text until exact verification, semantic hydration, or preview
rendering needs it.

Read-only search connections use mmap, larger page cache, and memory temp
store. Write connections use WAL except fresh staging, where durability is
provided by final atomic promote.

### Tantivy

Tantivy is lexical candidate retrieval. Current on-disk format is
`INDEX_FORMAT_VERSION = 17`.

Schema:

| Field | Use |
|---|---|
| `vector_key` | Stored id shared with SQLite and vector stores |
| `file_path` | Exact string field, stored, language/path filters |
| `start_line`, `end_line` | Stored result bounds |
| `language`, `kind` | Stored filters and ranking signals |
| `text` | Code-tokenized BM25 body, indexed with frequencies, not stored |
| `is_ignored` | Stored ignored-file flag |
| `file_path_text` | Tokenized path BM25F field |
| `signature` | Tokenized definition signature BM25F field |

Main text postings store frequencies without positions. Exact phrase/literal
verification reads SQLite chunk text instead of relying on Tantivy positions.

### Vector Stores

USearch stores ANN vectors in single files:

- `vectors.usearch`: 256-dimensional hash embeddings, F16 quantized.
- `vectors_neural.usearch`: learned embeddings, profile-dependent dimensions,
  F16 quantized.
- `overlay_vectors.usearch`: worktree-only divergent hash vectors.

Vector keys are stable and include path, chunk bounds, and content hash. This
prevents identical boilerplate in different files from sharing one vector id.

Unix search opens vector stores with mmap-style read-only views. Windows keeps
Rust-owned immutable buffers so Unicode paths and active replacement work
reliably.

## Embeddings

Always available:

- **HashEmbeddingModel**: 256-dimensional lexical/identifier hash embedding.
  No model download, used for fallback and first vector enrichment.

Default learned profile with `neural` feature:

- **`static-retrieval-v1`**: 256-dimensional static retrieval model from
  `sentence-transformers/static-retrieval-mrl-en-v1`. Assets download once via
  `hf-hub`; source text and queries are never uploaded.

Optional profiles via `IVYGREP_MODEL_PROFILE`:

| Profile | Dimensions | Backend |
|---|---:|---|
| `static-retrieval-v1` | 256 | Static token mean in Rust |
| `potion-code-16m-v1` | 256 | Model2Vec weighted token mean in Rust |
| `general` | 384 | Candle BERT, AllMiniLM-L6-v2 |
| `code-minilm-l6-v1` | 384 | Candle BERT, CodeSearchNet-tuned |
| `code-minilm-l12-v1` | 384 | Candle BERT, higher-quality CodeSearchNet-tuned |

`neural_model.json` persists profile, model id, revision, dimensions, pooling,
license, parameter count, asset bytes, and weights hash. Identity mismatch
forces neural vector rebuild before those vectors are trusted.

### Acceleration

Static profiles run in Rust with Rayon. Candle transformer profiles can use
hardware acceleration:

- `accelerate`: macOS Candle CPU through Accelerate.
- `metal`: source builds can run Candle transformers on Metal when available.
- `cuda`: source builds can run Candle transformers on CUDA; cuDNN is not
  required.

Backend falls back to local CPU if accelerator initialization fails. v1.0.1
also forwards `candle_embed/metal` to `candle_nn/metal`, fixing Metal source
builds that used Candle NN layers.

Resource controls:

| Env var | Effect |
|---|---|
| `IVYGREP_NEURAL_THREADS` | Neural worker count, default 25% CPU in background, up to 8 |
| `IVYGREP_NEURAL_MEMORY_MB` | Soft memory budget for transformer worker pool |
| `IVYGREP_NEURAL_ACCELERATOR_HANDLES` | Shared accelerator handles, capped at 8 |
| `IVYGREP_NEURAL_FOREGROUND_ACCELERATOR` | Disable foreground accelerator use when set false |
| `IVYGREP_NEURAL_BATCH_SIZE` | Override background learned-vector batch size, capped at 4096 |
| `IVYGREP_ENHANCE_MAX_LOAD_RATIO` | Load throttle for background enhancement |

Background learned-vector batch defaults:

- CPU or static profile: 64.
- CUDA: 8, with live backoff based on free VRAM and utilization.
- Metal: 256.

## Search Pipeline

Hybrid search runs these candidate passes:

1. **Literal**
   - Builds exact-ish variants of query text.
   - Uses Tantivy to narrow candidate chunks by token terms.
   - Verifies exact substring/regex matches against SQLite text.

2. **BM25F**
   - Searches `text`, `file_path`, tokenized `file_path_text`, and `signature`.
   - Boosts exact path and definition-signature matches.
   - Pushes `--type` and simple `--include '*.ext'` filters into Tantivy when
     possible.

3. **Symbols**
   - Looks up exact, qualified-leaf, inferred natural-language, and alias symbol
     names in SQLite.
   - Promotes canonical definitions without depending only on body-text BM25.

4. **Path recall**
   - Handles path-shaped and natural-language path queries through
     `file_path_text`.
   - Adds file-level candidates that body text alone would miss.

5. **Semantic**
   - Uses neural vectors when requested, present, and model identity matches.
   - Keeps hash-vector recall while neural coverage is partial.
   - Skips hash search only when neural vectors cover enough direct candidates.

6. **Fusion and filtering**
   - Reciprocal Rank Fusion merges literal, BM25, path, symbol, hash, and
     neural ranked lists.
   - Boosts term coverage, literal matches, identifier compaction,
     definition names, path segments, file authority, and file coherence.
   - Applies secondary-source gates and semantic-only confidence filters.

Search hydrates text late. BM25 candidates are truncated before SQLite text
fetch. Semantic candidates batch-fetch metadata/text with prepared cached
statements. Preview rendering reads live file contents through a bounded cache
that tracks file length and mtime.

## Daemon

Daemon protocol:

- Unix: owner-only socket with peer uid check.
- Windows: loopback TCP plus per-daemon auth token.
- Requests include protocol version and are capped at 1 MiB.
- CLI restarts daemon on `BUILD_VERSION` mismatch.

Runtime caches:

- Resolved workspace cache: 128 exact absolute roots.
- Search context pools: 32 workspace/dimension keys.
- Idle contexts: 4 per key, 32 global, each with read-only SQLite/Tantivy/vector
  views.
- Query result cache: 128 entries, skips result sets above 2,000 hits.
- Neural readiness cache: invalidated by vector/model file stamps.
- Embedding model: lazy `OnceLock`; hash model is used while learned model loads.

Daemon watchers debounce filesystem events over a 2-second quiet period and
30-second max debounce. Heavy search/index work is bounded by CPU permits.

## Build Variants

| Feature | Default | Effect |
|---|---|---|
| `neural` | yes | Enables learned profiles and first-use asset download |
| `accelerate` | no | Enables Accelerate-backed Candle CPU on macOS |
| `metal` | no | Enables Candle Metal transformer inference on macOS source builds |
| `cuda` | no | Enables Candle CUDA transformer inference on compatible Linux hosts |
| none | no | Hash-only binary, no learned model assets |

Commands:

```bash
# Default learned retrieval profile
cargo build --release

# Hash-only build
cargo build --release --no-default-features

# macOS source build with Metal transformer support
cargo build --release --features accelerate,metal

# Linux CUDA transformer build
./build.sh --features cuda
```

Release binaries use conservative defaults: macOS gets Accelerate CPU support;
Linux and Windows use portable CPU execution. Metal and CUDA remain source-build
features so release binaries do not require local GPU toolchains.

## Correctness Gates

Relevance is treated as product behavior, not only performance.

- `tests/relevance_quality.rs` tracks top-result relevance, MRR@10, nDCG@5,
  recommendation precision@3, low-authority leakage, and unrelated-query
  suppression.
- Search tests cover exact literals, path recall, symbol promotion, phrase to
  identifier matching, filters, overlays, and daemon cache behavior.
- Index tests cover staging, deferred indexes, vector tombstones, generation
  stamps, compression, generated-source fallback, and accelerator batch sizing.

Performance changes should be kept only when A/B data shows index/search speed
improves without relevance regression.
