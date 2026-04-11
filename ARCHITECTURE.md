# Architecture

> How ivygrep turns natural-language queries into instant, relevant code
> results — entirely offline, entirely local.

---

## What It Is

ivygrep is a **local-first semantic code search engine** built in Rust. You ask
a question in plain English — *"where is tax calculated"* — and it returns the
exact lines of code across your entire codebase. No cloud, no API keys, no
telemetry.

Under the hood it fuses two fundamentally different search strategies into a
single ranked result set:

- **Lexical search** (BM25) — finds exact and near-exact term matches
- **Semantic search** (vector similarity) — finds conceptually related code

Results from both are merged via Reciprocal Rank Fusion, scored, and filtered
in a single pass. The whole thing runs behind a background daemon that keeps
indexes warm and watches for file changes.

---

## Technology Stack

Every dependency exists for a specific reason. There are no framework
batteries — only purpose-selected engines.

### Tantivy — Full-Text Search Index

[Tantivy](https://github.com/quickwit-oss/tantivy) is the lexical search
backbone. It is a Rust-native full-text search engine (think Lucene, but
embeddable in a single process with no JVM).

**What we use it for:**

- **BM25 ranked search** — every code chunk is tokenized and indexed. Query
  terms are parsed via `QueryParser` against the `text` and `file_path` fields
  (with `file_path` boosted 2×) to produce relevance-ranked results.
- **Literal search** — the `--literal` fast path uses Tantivy to narrow the
  search space from all files to only chunks containing the query terms, then
  scans just those chunks for exact substring matches. This is O(matched_chunks)
  instead of O(all_files).
- **Language pushdown** — `--type rust` is compiled into a Tantivy
  `BooleanQuery` that combines the parsed text query with a `TermQuery` on the
  `language` field. This happens at the index level, not post-filter.
- **Schema** — each chunk is a Tantivy document with fields: `chunk_id`,
  `file_path` (STRING + STORED), `start_line`, `end_line`, `language`, `kind`,
  `text` (TEXT + STORED), and `content_hash`.

**Why Tantivy and not ripgrep/grep:** grep scans every file on every query.
Tantivy builds an inverted index once and answers term queries in milliseconds.
On a 92K-file repo, a Tantivy lookup returns in ~17ms vs seconds for a full
grep.

### USearch — Vector Similarity Index

[USearch](https://github.com/unum-cloud/usearch) is a compact, embeddable
approximate nearest-neighbor (ANN) search library. It implements HNSW
(Hierarchical Navigable Small World) graphs for fast cosine similarity search.

**What we use it for:**

- **Semantic search** — query text is embedded into a vector, then USearch finds
  the closest code chunk vectors by cosine distance. This is how
  *"retry logic for payments"* finds `fn handle_payment_retry()` even though
  the terms don't overlap.
- **Two-tier vector stores:**
  - `vectors.usearch` — 256-dimensional hash embeddings, built instantly during
    indexing. Always present.
  - `vectors_neural.usearch` — 384-dimensional ONNX neural embeddings
    (AllMiniLM-L6-v2), built asynchronously by a background subprocess. Higher
    quality, used when available.
- **Memory-mapped reads** — search opens the vector index with `view()` (mmap)
  instead of `load()`. On large indices (e.g. 1.5M vectors for the Linux
  kernel), this reduces open time from ~450ms to <1ms.
- **Atomic writes** — vector saves write to a `.tmp` file then `rename()` to
  prevent corrupted reads by concurrent search processes.

**Why USearch and not FAISS/Qdrant:** USearch is a single embeddable C++ library
with Rust bindings. No server process, no Python, no external dependencies.
The entire index is a single file.

### SQLite — Chunk Metadata Store

[SQLite](https://www.sqlite.org/) via `rusqlite` (bundled, no system dependency).

**What we use it for:**

- **Source of truth for chunk data** — every indexed code chunk is stored as a
  row: `chunk_id`, `file_path`, line range, `language`, `kind`, `text`,
  `content_hash`, and `vector_key`. All search results are resolved back to
  SQLite to get the full chunk metadata.
- **Vector key → chunk resolution** — after USearch returns the top vector
  matches (as numeric keys), SQLite translates them back to file paths, line
  numbers, and source text. This is the bridge between the vector index and
  human-readable results.
- **Filtered chunk collection** — when `--include '*.yaml'` or `--type rust` is
  used, the search engine queries SQLite directly
  (`SELECT ... WHERE language = ?`) to collect matching chunk vector keys, then
  scores only those against the query vector. This turns a full-corpus vector
  scan into a targeted lookup.
- **Stats cache** — `chunk_count` and `file_count` are cached in a `_stats`
  table, updated at commit time, so `--status` queries are O(1).
- **WAL mode** — `PRAGMA journal_mode = WAL` allows concurrent reads during
  writes. Indexing batches all inserts in a single transaction for 10-50×
  speedup.

**Why SQLite and not Postgres/RocksDB:** single-file, zero-config, bundled in
the binary. A code search tool should not require a database server.

### fastembed + ort — Neural Embedding Model

[fastembed](https://github.com/Anush008/fastembed-rs) provides high-level model
loading. [ort](https://github.com/pykeio/ort) is the Rust binding for ONNX
Runtime.

**What we use them for:**

- **AllMiniLM-L6-v2 (quantized INT8)** — the neural embedding model. Converts
  code chunks and search queries into 384-dimensional dense vectors that capture
  semantic meaning. Downloaded once (~23 MB) on first use, cached in
  `~/.local/share/ivygrep/models/`.
- **Batch embedding** — `embed_batch()` sends multiple chunks through the model
  in a single ONNX inference call, dramatically faster than one-at-a-time during
  the background enhancement pass.
- **CoreML acceleration** — on macOS, `ort` is compiled with the CoreML
  execution provider, offloading inference to Apple's Neural Engine / GPU.
  Registered automatically at startup via `ort::init().with_execution_providers()`.
- **Background thread budget** — when running as a background enhancement
  subprocess, `ORT_NUM_THREADS` is set to half the CPU count (min 2) so the
  system stays responsive.
- **Graceful fallback** — if the neural model fails to load (missing download,
  corrupt cache, unsupported platform), the system silently falls back to hash
  embeddings. No search ever fails because of a model problem.

**Why fastembed and not sentence-transformers:** fastembed is pure Rust/ONNX with
no Python dependency. The model runs in the same process as the search engine.

### Tree-sitter — AST-Aware Chunking For Core Languages

[Tree-sitter](https://tree-sitter.github.io/tree-sitter/) is an incremental
parsing library that produces concrete syntax trees.

**What we use it for:**

- **Precise function/class boundaries** — today, Tree-sitter is enabled for 5
  core languages (Rust, Python, Go, JavaScript, TypeScript). It parses the
  full AST and extracts structural node ranges using S-expression queries like:
  ```
  (function_item) @fn (impl_item) @class (trait_item) @class
  ```
  Each matched node becomes a chunk with exact start/end line numbers.
- **Quality over heuristics** — Tree-sitter gives perfect boundaries for nested
  functions, multi-line signatures, and trait impls. The regex-based fallback
  (used for the rest of the supported language set) sometimes splits
  mid-function.

**Why Tree-sitter and not regex-only:** regex can't reliably parse code. A line
like `if (function_call()) {` looks like a function definition to a regex
heuristic. Tree-sitter knows it's a control flow statement because it has the
full parse tree. For languages without an AST grammar wired in yet, we fall
back to the data-driven structural heuristic registry in `LANGUAGES`.

### notify — Filesystem Watcher

[notify](https://github.com/notify-rs/notify) is a cross-platform filesystem
event library.

**What we use it for:**

- **Live index updates** — the daemon registers a `RecommendedWatcher` (FSEvents
  on macOS, inotify on Linux) on each indexed workspace directory with
  `RecursiveMode::Recursive`. Any file change event triggers an incremental
  re-index.
- **Eliminating Merkle scans** — when a watcher is alive (verified via PID
  file), the indexer skips the expensive full-filesystem Merkle diff. On a
  92K-file repo, this saves ~2 seconds per query.
- **Debounced re-indexing** — file change events are sent through a
  `tokio::sync::mpsc` channel to a dedicated indexing task, which batches and
  processes them asynchronously.

**Why notify and not polling:** FSEvents/inotify are kernel-level and instant.
Polling would add latency and CPU overhead proportional to repo size.

### rayon — Parallel Processing

[rayon](https://github.com/rayon-rs/rayon) is a data-parallelism library for
Rust.

**What we use it for:**

- **Parallel file processing** — during indexing, files are chunked across all
  CPU cores using `par_iter()`. Each file is read, parsed (Tree-sitter or
  regex), and split into chunks in parallel. The results are collected and then
  sequentially written to storage.
- **Parallel Merkle scanning** — the full-filesystem fingerprint scan
  (`MerkleSnapshot::build`) uses `par_iter()` to stat and hash files across all
  cores. On a 92K-file repo this takes ~2 seconds instead of ~8.
- **Parallel hash embedding** — the `HashEmbeddingModel::embed_batch()`
  implementation uses `par_iter()` to compute embeddings across all cores.

**Why rayon and not manual threading:** rayon's work-stealing scheduler
automatically balances load across cores. No thread pool sizing, no manual
synchronization.

### xxhash — SIMD-Accelerated Hashing

[xxhash-rust](https://github.com/DoumanAski/xxhash-rust) provides the xxh3
family of hash functions, specifically the 128-bit variant.

**What we use it for:**

- **Merkle fingerprints** — each file is fingerprinted as
  `xxh3_128(path + file_size + mtime)`. The concatenation of all file
  fingerprints produces the workspace root hash. Comparing root hashes is an
  O(1) check for "has anything changed?"
- **Content hashing** — each chunk's content is hashed with xxh3 to produce a
  `content_hash` used for deduplication and change detection across re-indexes.
- **Vector key derivation** — the `vector_key` (USearch's numeric ID for each
  vector) is derived by hashing the `content_hash` with xxh3 and truncating to
  63 bits. This gives deterministic, collision-resistant keys without
  maintaining a separate sequence.
- **Workspace ID** — each workspace is identified by
  `hex(xxh3_128(canonical_root_path))`, ensuring stable IDs without path
  separator or symlink issues.

**Why xxh3 and not SHA-256:** we need speed, not cryptographic security. xxh3
runs at memory bandwidth on modern CPUs (~30 GB/s with SIMD). SHA-256 would be
10-50× slower for the same job.

### Merkle Tree — Incremental Change Detection

The Merkle tree is how ivygrep avoids re-indexing unchanged files. Without it,
every search on a cold daemon would require re-reading, re-chunking, and
re-embedding every file in the workspace — minutes of work on a large repo. With
it, re-indexing an unchanged 92K-file workspace takes ~10ms.

**The data structure:**

A `MerkleSnapshot` is a flat map of relative file paths to per-file fingerprints,
plus a single root hash derived from all of them:

```
MerkleSnapshot {
    root_hash: "a8b3...",         // xxh3_128 over all (path, hash) pairs
    files: {
        "src/main.rs":   "f1c2...",   // xxh3_128(path + file_size + mtime)
        "src/lib.rs":    "d4e5...",
        "Cargo.toml":    "7a8b...",
        ...
    }
}
```

Each file fingerprint is computed from metadata only — **no file contents are
read**. The inputs are:

1. **Relative path** (byte representation)
2. **File size** (8 bytes, little-endian)
3. **Modification time** (16 bytes, nanoseconds since epoch)

These three values are concatenated and hashed with `xxh3_128`. This means
detecting whether 92K files have changed requires only 92K `stat()` calls and
92K hashes — no disk reads. On a modern SSD, this completes in ~2 seconds
(parallelized via rayon).

The **root hash** is computed by concatenating all `(path, file_hash)` pairs in
sorted order (BTreeMap ensures deterministic ordering) and hashing the result
with `xxh3_128`. This single 128-bit value represents the entire workspace state.

**How the diff works:**

When the indexer runs, it builds a fresh snapshot and compares it against the
previously saved one:

```
1. Compare root hashes
   ├── Equal?     → Nothing changed, skip everything (O(1))
   └── Different? → Walk both file maps:
       ├── In new but not old         → added
       ├── In both, hash differs      → modified
       └── In old but not new         → deleted
```

The diff produces a `MerkleDiff { added_or_modified, deleted }`. Only the files
in `added_or_modified` are re-read, re-chunked, and re-embedded. Chunks for
`deleted` files are removed from all three stores (SQLite, Tantivy, USearch).

**Three-tier skip hierarchy:**

The indexer has three levels of shortcuts, each faster than the next:

| Check | Cost | When it triggers |
|-------|------|-----------------|
| **Watcher alive** | O(1) — read PID file | Daemon is watching this workspace; filesystem events handle updates. Skip the entire Merkle scan. |
| **Root hash match** | O(n) stat + hash | Merkle scan ran, but root hashes are identical. No files changed. |
| **Per-file hash diff** | O(changed) | Root hashes differ, but only 3 of 92K files changed. Re-index just those 3. |

The daemon's `notify` watcher makes the first tier the common case. When the
daemon is alive, the Merkle scan is skipped entirely — `is_watcher_alive()`
checks for a PID file and verifies the process exists. The watcher handles
incremental re-indexing via filesystem events. The Merkle scan only runs on cold
starts (first search after a reboot, or when the daemon was killed).

**Why "Merkle" and not just timestamps:**

Comparing file paths + sizes + mtimes via hash rather than storing raw triples
has two advantages:

1. **O(1) workspace-level check** — a single root hash comparison short-circuits
   the entire diff when nothing changed, without walking the file list.
2. **Deterministic serialization** — the snapshot is a JSON file
   (`merkle_snapshot.json`) with sorted keys. It can be compared, diffed, and
   debugged with standard tools.

The tradeoff is that mtime-based fingerprinting can produce false positives
(e.g., `touch` changes mtime without changing content). A false positive triggers
an unnecessary re-chunk and re-embed for that file, but the chunk's
`content_hash` is based on actual content, so the storage layer handles
deduplication correctly — the old chunk is removed and an identical one is
re-inserted at the same vector key.

---

## Core Data Flow

### 1. Indexing Pipeline

When a workspace is indexed (first search, `--add`, or file watcher trigger),
the pipeline processes files through four stages:

```
① Scan  →  ② Chunk  →  ③ Embed  →  ④ Store
```

1. **Scan** — the `ignore`-crate walker traverses the workspace respecting
   `.gitignore` rules. A Merkle snapshot (xxh3 fingerprint per file) is
   compared against the previous snapshot to identify added, modified, and
   deleted files.

2. **Chunk** — changed files are split into semantic code chunks:
   - Tree-sitter AST parsing for Rust, Python, Go, JS, TS
   - Regex-based signature detection for 35+ other languages
   - Fixed-window fallback for text/config/markup

3. **Embed** — each chunk's text is embedded:
   - Hash embeddings (256-dim) are computed inline, instantly
   - Neural embeddings (384-dim, AllMiniLM-L6-v2) are computed later by a
     background subprocess (`--enhance-internal`)

4. **Store** — chunks are written to three storage backends in a single
   transaction: SQLite (metadata), Tantivy (full-text index), USearch (vectors).

### 2. Search Pipeline

Every query runs through a hybrid fusion pipeline:

1. **Lexical** — Tantivy BM25 search with tokenized, singularized,
   and compacted query variants
2. **Semantic** — USearch ANN search using the query's embedding vector
3. **Fusion** — Reciprocal Rank Fusion (k=60) merges both ranked lists
4. **Boosting** — literal match bonus, term coverage, path segment matching,
   normalized identifier matching
5. **Filtering** — adaptive score threshold based on result distribution
6. **Context** — focus line detection + ±N context lines from source

### 3. Daemon Architecture

The daemon (`ig --daemon`) is a Tokio-based async server on a Unix domain
socket. It provides:

- **Shared model loading** — the ONNX model loads once in a background thread
  (`OnceLock`). All CLI invocations share it.
- **File watching** — `notify` watchers per workspace, triggering incremental
  re-index on file changes.
- **Version-gated restart** — each response includes `BUILD_VERSION`. On
  mismatch, the CLI sends `Restart` and auto-spawns the new binary.
- **Connection resilience** — 2-second timeouts on connect/write, stale socket
  cleanup, automatic local fallback.

---

## Storage Layout

```
~/.local/share/ivygrep/
├── daemon.log                          # Daemon stderr output
├── daemon.sock                         # Unix domain socket (IPC)
├── models/                             # ONNX model cache (~23 MB)
│   └── AllMiniLML6V2Q/
└── indexes/
    └── <workspace-id>/                 # hex(xxh3(canonical_path))
        ├── workspace.json              # Workspace metadata
        ├── merkle.json                 # File fingerprint snapshot
        ├── metadata.sqlite3            # SQLite — chunk text + metadata
        ├── tantivy/                    # Tantivy BM25 index segments
        ├── vectors.usearch             # Hash embeddings (256-dim)
        ├── vectors_neural.usearch      # Neural ONNX embeddings (384-dim)
        ├── .enhancing.pid              # PID of neural enhancement subprocess
        └── .watcher.pid                # PID of daemon watcher
```

---

## Build Variants

| Feature | Default | Effect |
|---------|---------|--------|
| `neural` | ✅ | Enables ONNX neural embeddings (fastembed + ort). Adds ~23MB model download. |
| *(none)* | — | Hash-only mode. Smaller binary, no ONNX, lower search quality. |

```bash
# Full build (default — includes ONNX neural embeddings)
cargo build --release

# Minimal build (hash embeddings only, no model download)
cargo build --release --no-default-features
```

On macOS, the `neural` feature automatically links CoreML for GPU/ANE
acceleration. On Linux, ONNX runs on CPU.
