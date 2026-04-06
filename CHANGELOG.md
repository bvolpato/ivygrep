# Changelog

All notable changes to ivygrep are documented in this file.

## [0.5.11] — 2026-04-06

### Added
- Optional hardware acceleration for Linux users with CUDA/GPU installed (significantly speeds up neural embedding generation)

### Performance
- **Faster initial indexing:** Eliminated redundant per-file SQLite lookups and Tantivy deletes on fresh indexes (pure INSERT vs INSERT OR REPLACE)
- **Parallel filesystem scanning:** Switched Merkle snapshot from sequential walk + parallel hash to fully parallel walker, improving scan throughput on large repos
- **SQLite tuning:** Enabled WAL mode, larger page cache, and in-memory temp storage for bulk writes
- **Tantivy heap:** Increased writer heap from 50MB to 200MB, reducing forced commit frequency
- **Reduced I/O noise:** Lowered progress file writes from every 500 to every 2000 files, compact (non-pretty) Merkle JSON
- **Batched timestamps:** Single syscall per file batch instead of per chunk (eliminates 1M+ syscalls on Linux kernel)

## [0.5.10] — 2026-04-05

### Fixed
- **Neural Error Observability:** Added explicit error messages to `ig --status` when background neural embedding operations fail (e.g. out of memory, network failure). The status no longer silently reverts to "run a query to trigger neural upgrade".
- **Benchmark Fidelity:** Converted CI performance metrics to display in microseconds (µs) for improved readability in PR comments, and wired up the full suite of criterion benchmarks.
- Eliminated internal compilation warnings and Clippy suggestions.

## [0.5.7] — 2026-04-05

### Fixed
- **RAII PID file cleanup:** `.indexing.pid` and `.enhancing.pid` lockfiles are now guaranteed to be removed via RAII guards, even when indexer or enhancer threads panic. Prevents stale PID files from blocking subsequent daemon runs.

### Performance
- **Batched SQLite transaction commits:** The indexer now dynamically batches SQLite transaction commits by chunk count instead of per-file, significantly improving indexing throughput on Linux.

### CI
- Added `github-action-benchmark` with Criterion tracking to monitor indexer performance across commits with automatic PR comments.

## [0.5.6] — 2026-04-05

### Fixed
- Re-enabled cross-architecture smoke testing for static aarch64 payloads using GitHub Actions Qemu setup.

## [0.5.5] — 2026-04-05

Fully **statically linked** Linux binaries — zero shared library dependencies.

### Build: Portable Linux Binaries
- **musl static linking:** Linux release binaries now target `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, producing fully self-contained executables with no glibc dependency. This eliminates the `libmvec.so.1` / `libstdc++.so.6` errors on older or minimal Linux distributions.
- **cross-compilation:** Release workflow uses [`cross`](https://github.com/cross-rs/cross) for Linux builds, providing a proper musl-native C++ toolchain for the `usearch` dependency.
- **usearch simsimd disabled:** The `simsimd` feature is disabled to ensure compatibility with musl cross-toolchains. The `fp16lib` feature is retained for half-precision float support.

## [0.5.4] — 2026-04-04

A milestone architecture release introducing **Worktree-Aware Zero-Copy Overlay Indexing**.

### Feature: Shared Base + Thin Overlays
- **Worktree Indexing:** When indexing a `git worktree`, `ivygrep` no longer copies the enormous parent repository. Instead, it reads the `.cache/ivygrep/{base}` index and dynamically constructs a lightning-fast "overlay index" (`metadata.sqlite3`, `vectors...`) containing exclusively the chunks that were added, modified, or deleted in the worktree.
- **Microsecond Tombstoning:** If a file is deleted or modified in your worktree, ivygrep registers robust SQLite tombstones in the overlay. The `SearchContext` seamlessly merges base and overlay indices mid-query, ensuring ultra-accurate search results.
- **Base Auto-Indexing Cascade:** If you attempt to index a worktree before your `ivygrep` daemon has naturally indexed the base checkout, ivygrep gracefully intercepts the request, recursively locks and builds the full base index, and rapidly evaluates your overlay delta afterwards.
- **Background Upgrade Cascading:** Background neural enhancement operations automatically cascade into parent base indices when triggered from a dependent worktree.
- **UI Tracking Hierarchy:** `ig --status` has been revamped to visualize base repositories alongside a dedicated, indented visual tree representing its corresponding worktree overlays. Index file footprints precisely isolate the delta byte counts compared to the main checkout.

## [0.5.3] — 2026-04-03

Minor patch addressing Clippy CI constraints.
- Resolved `clippy::collapsible_if` nested block rules originating from integration test additions.

## [0.5.2] — 2026-04-03

- **CoreML Thermal/Cache Tuning:** Reduced the ONNX background execution batch size from 64 down to 16. While 64 scaled optimally on pure high-VRAM GPU setups, it caused severe thermal throttling and L2 cache thrashing on Apple Silicon / CoreML execution providers, slowing down the background indexer. The new limit still benefits from 2× batch throughput over v0.5.0 but maintains crisp desktop responsiveness.

## [0.5.1] — 2026-04-03

- **ONNX Throughput Boost:** Increased the background neural enhancement batch size by 8× (from 8 to 64). To strictly prevent out-of-memory CoreML/ONNX Tensor attention matrix expansion bloat, chunk text is now deterministically bounded and truncated at ~1024 bytes directly before tokenization.

## [0.5.0] — 2026-04-03

A massive storage efficiency and stability release. The index-to-source ratio has been reduced from **~6.5× to ~2.3×**.

> [!WARNING]  
> **Breaking Change:** Due to the migration of neural and hash vectors to FP16 quantization, and the addition of `zstd` compression for SQLite, existing indices are incompatible. Please wipe your local `~/.local/share/ivygrep/` directory or run `ig --add . --force` before performing new searches to avoid mismatched chunks.

### Storage & Performance
- **F16 Vector Quantization:** `USearch` indices are now quantized down to `ScalarKind::F16` for hash embeddings, strictly halving the footprint of `.usearch` stores.
- **SQLite zstd Compression:** Reduced `chunks.text` storage massively by compressing raw text chunks using `zstd`. Legacy uncompressed rows are auto-detected and correctly decoded.
- **Tantivy Store Truncation:** Extracted `STORED` flag from Tantivy's text index. Full lexical matches now rely seamlessly on SQLite, removing ~500MB+ per index.

### Stability & Indexing Pipeline
- **Tree-sitter Timeout Engine:** Refactored tree-sitter bindings to invoke modern `ParseOptions` with `progress_callback`, imposing a mandatory 100ms parser completion limit. This entirely eliminates deadlocking on obfuscated, heavily-minified JavaScript or deeply nested data.
- **Robust Enhancement Trigger:** Fixed a bug where indexer interruption permanently halted neural enhancement background processing. Background tasks now correctly calculate differential completion metrics to resume reliably via `.needs_neural_enhancement()`.
- **First-run Spinner Resolution:** Initial daemon chunking progress now writes and parses `.indexing.progress`. "Stuck at 0 chunks" spinners are now perfectly responsive.

## [0.4.7] — 2026-04-03

Introducing the new fast literal search path. This completes the performance push by optimizing the final bottleneck: exact string match queries.

### Performance
- **Index-Backed Literal Search (`--literal` / `-l`):** 5.6× faster than the old `--regex` mode on massive repos. Bypasses BM25 and neural enhancement entirely, utilizing Tantivy phrase queries to rapidly isolate relevant chunks before performing an exact case-insensitive scan.
- **Daemon-Routed Exact Matches:** The new literal fast-path runs through the daemon by default (`DaemonRequest::LiteralSearch`), meaning if the daemon hasn't finished loading the 134MB neural model, exact text searches still complete in milliseconds.
- **MCP Literal Parameter:** `ig_search` now supports `literal: true` directly to provide agents with a high-speed search alternative when semantic search isn't needed.

### Changed
- Hide the slow `--regex` flag from `--help` (still works, but users are steered to `--literal` or `rg` for pure regex).

## [0.4.6] — 2026-04-03

A state-of-the-art query latency release that makes ivygrep as fast as traditional string matchers like `grep` and `ripgrep` while maintaining intelligent retrieval. Un-cached searches of 90,000+ files take around ~15-40ms.

### Performance
- **Identifier Fast-Path:** Queries consisting of single word identifiers (like "kfree" or "malloc") bypass the ONNX memory-mapped vector semantic step entirely, searching strictly via BM25 SQL. Speed increased by over 10x (`~40ms` query latency on Linux).
- **No-Rescan Penalty:** Local `ig` searches heavily bypass duplicate workspace Merkle re-indexes. If the workspace is already indexed, the CLI relies heavily on the background daemon and triggers instant search mode to save ~2 seconds of latency.
- **Daemon Speedups:** Fixed IPC RPC errors caused by old daemon sockets surviving binary restarts and enhanced search options.
- **Lazy Models:** Reduced memory usage by making Embedding models dynamically lazy.

## [0.4.1] — 2026-04-02

A performance-focused release that makes ivygrep viable on massive monorepos
(tested on a 269K-file, 2.3M-chunk, 17 GB production codebase). Indexing is up to 35%
faster, `ig --status` dropped from 20 s to 24 ms, and filtered queries now
bypass full-corpus vector scans entirely.

### Added

- **`--wait-for-enhancement` flag** — block until neural embeddings reach 100%
  before returning results (`02b2d60`).
- **`ig --status` dashboard** — rich workspace health view showing index age,
  watcher state, neural enhancement progress, and CoreML acceleration
  (`3c5834d`).
- **Dynamic terminal progress** — real-time `[n/total] chunking…` counter
  during first-run indexing and neural enhancement (`dc71324`, `0de7f60`).
- **CLI spinner** — `⠋ searching…` feedback while the daemon processes queries
  so large repos never appear frozen (`037323e`).
- **MCP `ig_search` tool** — full-featured Model Context Protocol server for
  AI coding agents (Claude Code, Cursor, Codex, OpenCode) with auto-indexing,
  scoping, and `.gitignore` support.

### Performance

- **Instant indexing → background neural** — two-tier pipeline: hash embeddings
  index in ~0.0 s, ONNX neural embeddings compute silently in the background
  (`5640a3f`).
- **xxh3 SIMD hashing** — replaced SHA-256 with 128-bit `xxh3` for Merkle
  fingerprints and vector keys; ~4× faster hashing (`e310a02`).
- **Parallel Merkle scan** — `rayon` parallel stat + hash across all cores;
  cold index −24%, warm scan −35% on Linux kernel (`0787e13`).
- **MPSC streaming pipeline** — decoupled file I/O from SQLite writes via
  async channels, capping memory at 4096-file batches (`a64b5f3`).
- **SQLite WAL + single-transaction batching** — all INSERTs in one
  `tx.commit()`, all Tantivy docs in one `writer.commit()` (`0787e13`).
- **SQLite pre-filtering for globs** — `--include '*.yaml'` pushes language
  filter into SQLite index lookup, turning 2.3M-row scans into a few thousand
  rows (`207743e`).
- **Tantivy language pushdown** — `BooleanQuery(Must, query, Must, lang)` skips
  irrelevant Tantivy segments at query time (`207743e`).
- **Cached `_stats` table** — `ig --status` reads O(1) pre-computed counts
  instead of `COUNT(*)` on 2.3M rows; 20 s → 24 ms (`4d4b5a9`).
- **Watcher-alive shortcut** — when a live daemon watcher is confirmed, skip
  the full Merkle rebuild entirely (`0787e13`).
- **OOM prevention** — bounded vector allocations prevent memory spikes during
  massive indexing runs (`3c94545`).
- **Apple CoreML acceleration** — ONNX embedding model offloads to the Neural
  Engine / GPU on macOS automatically (`8332268`).

### Fixed

- **Daemon zombie reaping** — defunct child processes no longer block lockfile
  cleanup (`4d24094`).
- **Atomic subprocess locks** — prevent duplicate enhancement processes from
  spawning concurrently (`dd3e0d0`, `29bfeca`).
- **Request-aware daemon timeouts** — prevent double-indexing when the daemon
  receives concurrent requests (`a9771f6`).
- **Neural segfault** — fixed background neural enhancement crash and daemon
  vector corruption (`22c18fb`).
- **Memory spikes** — resolved embedding model memory spikes by streaming
  batches with bounded allocation (`ddc258a`, `33efe8a`).
- **Linux portable binaries** — switched from glibc to musl for fully portable
  Linux releases (`2a6edf1`).
- **False model-download message** — eliminated spurious "Downloading embedding
  model" log on every run (`00f1644`).

### Changed

- Upgraded Tantivy to 0.26.0 (`d8a9577`).
- Deduplicated `FileSearchResult` / `group_hits_by_file` / walker config
  (`62a0c8d`).
- Extracted shared text helpers (camelCase splitting, singularization) into
  `text` module (`d8c444c`).

## [0.3.2] — 2026-03-24

Patch release with bug fixes and stability improvements.

## [0.3.1] — 2026-03-23

Minor improvements and documentation updates.

## [0.3.0] — 2026-03-22

Initial public release with hybrid BM25 + semantic search, tree-sitter AST
chunking, incremental Merkle indexing, and daemon-based file watching.
