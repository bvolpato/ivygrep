# Architecture

> How ivygrep turns natural-language queries into instant, relevant code
> results -- with local inference and no source-code upload.

---

## What It Is

ivygrep is a **local-first semantic code search engine** built in Rust. You ask
a question in plain English — *"where is tax calculated"* — and it returns the
exact lines of code across your entire codebase. No hosted inference, no API
keys, no telemetry. Neural mode downloads model assets once through `hf-hub`;
source text and queries are never sent to that service.

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
On a 93K-file Linux checkout, indexed candidate lookup stays in milliseconds
instead of scanning every file.

### Vector Backends

[USearch](https://github.com/unum-cloud/usearch) is the optimized Linux/macOS
approximate nearest-neighbor backend. Windows hash-only builds use a
dependency-light pure-Rust backend with the same persistence/search contract
and deterministic linear cosine ranking.

Windows daemon IPC uses loopback TCP with a fresh per-daemon authentication
token. Unix keeps the owner-only socket and peer-uid check.

**What we use it for:**

- **Semantic search** — query text is embedded into a vector, then USearch finds
  the closest code chunk vectors by cosine distance. This is how
  *"retry logic for payments"* finds `fn handle_payment_retry()` even though
  the terms don't overlap.
- **Two-tier vector stores:**
  - `vectors.usearch` — 256-dimensional hash embeddings, built asynchronously
    after lexical indexing. The store exists immediately but can be empty while
    BM25/literal search is already queryable.
  - `vectors_neural.usearch` — 384-dimensional F16 Candle neural embeddings,
    built asynchronously by a background subprocess. Higher quality, used when
    available.
- **Memory-mapped reads** — search opens the vector index with `view()` (mmap)
  instead of `load()`. On large indices (e.g. 4.66M vectors/chunks for the
  Linux kernel), this keeps vector-store open time out of the hot path.
- **Crash-safe writes** — vector saves write to a temporary file and replace
  the active store. The portable backend keeps a recoverable backup during the
  Windows replacement sequence.

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
- **Stats cache** — `chunk_count`, `file_count`, and `vector_key_count` are
  cached in `_stats` at commit time. Normal status/doctor never falls back to
  full-table scans; `--doctor --deep` performs live integrity counts.
- **Symbol graph** — exact definitions and lightweight call/reference edges
  are indexed by normalized symbol name for `--symbol`, `--refs`, and
  `--callers`. Worktree tombstones prevent shadowed base symbols from leaking
  into overlay results.
- **WAL mode** — `PRAGMA journal_mode = WAL` allows concurrent reads during
  writes. Indexing batches all inserts in a single transaction for 10-50×
  speedup.

**Why SQLite and not Postgres/RocksDB:** single-file, zero-config, bundled in
the binary. A code search tool should not require a database server.

### candle_embed + Candle -- Neural Embedding Model

[`candle_embed`](https://crates.io/crates/candle_embed) provides model loading
and embedding over [`candle-core`](https://github.com/huggingface/candle).

**What we use them for:**

- **AllMiniLM-L6-v2** -- the default neural embedding model. Converts
  code chunks and search queries into 384-dimensional dense vectors that capture
  semantic meaning. Downloaded on first neural use through `hf-hub`, cached in
  `$HF_HOME` or `~/.cache/huggingface`.
- **Code MiniLM profile** -- `IVYGREP_MODEL_PROFILE=code` selects a pinned
  CodeSearchNet-trained 384-dimensional MiniLM checkpoint. The profile identity
  is stored with the vector index; a mismatch forces re-embedding.
- **Parallel background embedding** -- `embed_batch()` distributes slices over
  a bounded pool of Candle embedders in OS threads. The query path keeps one
  model instance.
- **Current acceleration** -- macOS release builds enable Accelerate-backed
  Candle CPU math; portable Linux release builds use Candle CPU execution.
  Source builds can opt into local Metal or CUDA inference with `--features
  metal` or `--features cuda`. CUDA builds do not require cuDNN. `build.sh`
  and `test.sh` infer `CUDA_COMPUTE_CAP=120` for RTX 50/Blackwell hosts when
  `nvidia-smi` cannot report compute capability. Metal is exercised in Apple
  Silicon CI but is not a release default: its current single-stream
  enhancement path uses less memory but is slower than the CPU worker pool.
- **Background thread budget** -- CPU neural enhancement uses at most 25% of
  CPU cores, capped at eight worker/model instances. Metal currently uses one
  model instance to avoid multiplying local GPU/unified-memory residency.
- **Graceful fallback** -- if neural enhancement fails (missing download,
  corrupt cache, unsupported platform), lexical/hash search stays available and
  status reports the failed background job.

**Why Candle and not a hosted embedding API:** model inference runs in-process
with no Python service and no source-code upload.

### Model Evaluation Roadmap

Keep `AllMiniLM-L6-v2` as the portable default until the opt-in code profile or
another replacement wins public relevance and laptop-throughput gates on macOS,
Linux, and Windows-compatible hash fallback.

| Candidate | Fit | Required work before experiment |
|---|---|---|
| [`BAAI/bge-small-en-v1.5`](https://huggingface.co/BAAI/bge-small-en-v1.5) | Closest low-cost candidate: 384 dimensions and 512-token sequence length | Add BGE pooling and query-prefix behavior; compare CPU, Metal, and CUDA throughput |
| [`jinaai/jina-embeddings-v2-base-code`](https://huggingface.co/jinaai/jina-embeddings-v2-base-code) | Code-aware 768-dimensional model with 8192-token context | Add JinaBERT adapter; measure larger vector-store memory and ANN cost |
| [`nomic-ai/nomic-embed-text-v1.5`](https://huggingface.co/nomic-ai/nomic-embed-text-v1.5) | 768-dimensional model with 8192-token context and Matryoshka dimensions | Add custom model adapter and task prefixes; evaluate reduced dimensions before increasing index footprint |

### Tree-sitter — AST-Aware Chunking For 10 Core Languages

[Tree-sitter](https://tree-sitter.github.io/tree-sitter/) is an incremental
parsing library that produces concrete syntax trees.

**What we use it for:**

- **Precise function/class boundaries** — today, Tree-sitter is enabled for 10
  core languages (Rust, Python, Go, JavaScript, TypeScript, Java, C#, PHP,
  Ruby, Swift). It parses the
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
  file), the indexer skips the expensive full-filesystem Merkle diff. On large
  repositories, this avoids thousands of metadata reads per query.
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
  cores. On large repositories this keeps cold validation bounded by parallel
  metadata reads instead of serial tree walks.
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
it, re-indexing an unchanged large workspace can complete in milliseconds.

**The data structure:**

A `MerkleSnapshot` is a flat map of relative file paths to per-file fingerprints,
plus a single root hash derived from all of them:

```
MerkleSnapshot {
    root_hash: "a8b3...",         // xxh3_128 over all (path, hash) pairs
    files: {
        "src/main.rs":   "f1c2...",   // xxh3_128(size + mtime + ctime)
        "src/lib.rs":    "d4e5...",
        "Cargo.toml":    "7a8b...",
        ...
    }
}
```

Each file fingerprint is computed from metadata only — **no file contents are
read**. The inputs are:

1. **File size** (8 bytes, little-endian)
2. **Modification time** (16 bytes, nanoseconds since epoch)
3. **Change time on macOS/Linux** (`ctime`, seconds and nanoseconds)

These values are packed into a fixed-size stack buffer and hashed with
`xxh3_128`. The relative path is already the snapshot map key and is included
in the root hash, so hashing it again per file would be redundant work. `ctime` advances
when contents or inode metadata change, including writes followed by mtime
restoration (`touch -r` or `cp -p`). This closes stale-index misses without
reading repository content during verification. This means
detecting whether 93K files have changed requires only 93K `stat()` calls and
93K hashes — no disk reads. On a modern SSD, this is parallelized via rayon.

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
| **Per-file hash diff** | O(changed) | Root hashes differ, but only 3 of 93K files changed. Re-index just those 3. |

The daemon's `notify` watcher makes the first tier the common case. When the
daemon is alive, the Merkle scan is skipped entirely — `is_watcher_alive()`
checks for a PID file and verifies the process exists. The watcher handles
incremental re-indexing via filesystem events. The Merkle scan only runs on cold
starts (first search after a reboot, or when the daemon was killed).

**Why "Merkle" and not just timestamps:**

Comparing file paths + sizes + timestamps via hash rather than storing raw values
has two advantages:

1. **O(1) workspace-level check** — a single root hash comparison short-circuits
   the entire diff when nothing changed, without walking the file list.
2. **Deterministic serialization** — the snapshot is a JSON file
   (`merkle_snapshot.json`) with sorted keys. It can be compared, diffed, and
   debugged with standard tools.

The tradeoff is that metadata-based fingerprinting can produce false positives
(e.g., `touch` changes mtime without changing content). A false positive triggers
an unnecessary re-chunk and re-embed for that file, but the chunk's
`content_hash` is based on actual content, so the storage layer handles
deduplication correctly — the old chunk is removed and an identical one is
re-inserted at the same vector key.

---

## Core Data Flow

### 1. Indexing Pipeline

When a workspace is indexed (first search, `--add`, or file watcher trigger),
the pipeline commits a lexical index first, then enriches vectors in background:

```
① Scan  →  ② Chunk  →  ③ Store lexical index  →  ④ Enrich vectors
```

1. **Scan** — the `ignore`-crate walker traverses the workspace respecting
   `.gitignore` rules. A Merkle snapshot (xxh3 fingerprint per file) is
   compared against the previous snapshot to identify added, modified, and
   deleted files.

2. **Chunk** — changed files are split into semantic code chunks:
   - Tree-sitter AST parsing for Rust, Python, Go, JS, TS
   - Regex-based signature detection for 35+ other languages
   - Fixed-window fallback for text/config/markup

3. **Store lexical index** — chunks commit to SQLite and Tantivy. Queries can
   return BM25/literal results as soon as this commit completes.

4. **Enrich vectors** — a niced, load-aware background subprocess
   (`--enhance-internal`) builds resumable 256-dim hash ANN vectors, then
   384-dim AllMiniLM-L6-v2 neural vectors.

### 2. Search Pipeline

Every query runs through a hybrid fusion pipeline:

1. **Lexical** — Tantivy BM25 search with tokenized, singularized,
   compacted, and repository-neutral alias query variants
2. **Semantic** — USearch ANN search using the raw query plus
   identifier-normalized terms, without lexical alias injection
3. **Fusion** — Reciprocal Rank Fusion (k=60) merges both ranked lists
4. **Boosting** — literal match bonus, term coverage, path segment matching,
   normalized identifier matching, file authority, and per-file diversity
5. **Filtering** — adaptive score threshold plus quality gates for
   low-authority files and low-confidence semantic-only neighbors
6. **Context** — focus line detection + ±N context lines from source

Relevance changes are guarded by `tests/relevance_quality.rs`, a labeled corpus
that measures top-result relevance, MRR@10, nDCG@5, recommendation precision@3,
forbidden low-authority leakage, and unrelated-query suppression.

### 3. Daemon Architecture

The daemon (`ig --daemon`) is a Tokio-based async server on a Unix domain
socket. It provides:

- **Shared model loading** — the Candle model loads once in a background thread
  (`OnceLock`). All CLI invocations share it.
- **File watching** — `notify` watchers per workspace, triggering incremental
  re-index on file changes.
- **Version-gated restart** — each status response includes `BUILD_VERSION`. On
  mismatch, the CLI sends `Restart` and auto-spawns the new binary.
- **Bounded protocol framing** — each request carries an explicit protocol
  version and is capped at 1 MiB. Malformed, oversized, or incompatible
  requests receive a structured JSON error.
- **Connection resilience** — 2-second timeouts on connect/write, stale socket
  cleanup, automatic local fallback.

---

## Storage Layout

```
~/.local/share/ivygrep/
├── daemon.log                          # Daemon stderr output
├── daemon.sock                         # Unix domain socket (IPC)
└── indexes/
    └── <workspace-id>/                 # hex(xxh3(canonical_path))
        ├── workspace.json              # Workspace metadata
        ├── merkle_snapshot.json        # File fingerprint snapshot
        ├── metadata.sqlite3            # SQLite — chunk text + metadata
        ├── tantivy/                    # Tantivy BM25 index segments
        ├── vectors.usearch             # Async hash embeddings (256-dim)
        ├── vectors_neural.usearch      # Neural Candle embeddings (384-dim)
        ├── .hash_tombstones            # Stale hash keys queued after foreground edits
        ├── .hash_enhanced_generation   # Last lexical generation covered by hash vectors
        ├── .neural_tombstones          # Stale neural keys queued after foreground edits
        ├── .enhancing.pid              # PID of hash + neural enrichment process
        ├── .enhancing.phase            # Current hash or neural enrichment phase
        └── .watcher.pid                # PID of daemon watcher
```

Neural model assets are cached outside this tree by `hf-hub`, under `$HF_HOME`
or `~/.cache/huggingface`.

---

## Build Variants

| Feature | Default | Effect |
|---------|---------|--------|
| `neural` | default | Enables Candle neural embeddings. Downloads model assets on first neural use. |
| `accelerate` | opt-in | Uses Apple's Accelerate framework for Candle CPU math on macOS. |
| `metal` | opt-in | Executes Candle neural inference through Metal when a local Metal device is available; falls back locally to CPU. |
| `cuda` | opt-in | Executes Candle neural inference through CUDA when built on a compatible CUDA host; falls back locally to CPU. Does not require cuDNN. |
| *(none)* | - | Hash-only mode. Smaller binary, no model download, lower search quality. |

```bash
# Full build (default -- includes Candle neural embeddings)
cargo build --release

# Minimal build (hash embeddings only, no model download)
cargo build --release --no-default-features

# macOS opt-in Metal build
cargo build --release --features accelerate,metal

# Linux CUDA build (requires a compatible CUDA installation)
./build.sh --features cuda
```

Release binaries for macOS are built with `accelerate`. A separate Apple
Silicon CI lane builds `accelerate,metal` and requires actual `Candle Metal`
reporting, but Metal remains opt-in until enhancement throughput improves.
Portable Linux release binaries currently use Candle CPU execution.
