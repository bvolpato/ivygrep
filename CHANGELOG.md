# Changelog

All notable changes to ivygrep are documented in this file.

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
