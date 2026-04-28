# Changelog

All notable changes to ivygrep are documented in this file.

## [0.6.12] — 2026-04-27

### Performance
- **8× faster hybrid search on large repos:** Replaced O(N) individual SQLite lookups with batched `WHERE vector_key IN (...)` queries, reducing hundreds of B-tree traversals to 1-2 round-trips. Hash hybrid search dropped from ~4s to ~0.5s on a 290K-file, 3.8M-chunk repository.
- **Read-path SQLite PRAGMAs:** Added `mmap_size` (2 GB), `cache_size` (64 MB), and `temp_store = MEMORY` to read-only connections. Cold-start search dropped from ~5.4s to ~3.5s on multi-GB indexes.
- **Prepared statement caching:** `fetch_chunk_by_vector_key` now uses `prepare_cached()` to reuse compiled SQL across hundreds of calls per search.

### Added
- **`--type` accepts file extensions and aliases:** You can now use `ig --type rs`, `ig --type py`, `ig --type c++`, or `ig --type bash` instead of the full language name. Common aliases like `js` → JavaScript, `ts` → TypeScript, and `yml` → YAML are supported.

## [0.6.11] — 2026-04-27

### Performance
- **60× faster regex search on large repos:** Regex patterns like `func.*DDSQLizer` on a 2GB+ monorepo (289K files, 3.8M chunks) dropped from 12s to ~0.2s. Extracts literal fragments from regex patterns and uses the Tantivy inverted index to pre-filter to only files that could match, then scans candidates in parallel with rayon.
- **Removed unnecessary 10ms sleep in neural enhancer:** Background embedding now runs at full speed when the system is not constrained.

### Added
- **Path-based score boosting:** Files whose path contains the query term (e.g., searching for "my-service" surfaces `apps/my-service/` at the top) now receive a significant ranking boost, ensuring directory/filename matches outrank generic code hits.

### Fixed
- Dependency bumps: `openssl` 0.10.78, `rand` 0.8.6, `rustls-webpki` 0.103.13.

## [0.6.10] — 2026-04-26

### Performance
- **17× faster search on large repos:** Search on a 7GB+ monorepo (289K files, 3.8M chunks) dropped from 20s to ~1s by replacing runaway 1M candidate limits with proportional budgets.
- **Candidate limits scale with `--limit`:** Lexical (10×N), literal (5×N), and semantic (1×N) candidates now grow proportionally when `--limit` is increased, with sensible caps.

### Added
- **Ctrl+C cancels in-flight search:** In the TUI, pressing Ctrl+C or Esc now cancels a running search instead of quitting. Three-tier behavior: cancel search → clear input → quit.
- **Cooperative cancellation:** A shared `cancel_token` (AtomicBool) is threaded through the search pipeline, checked between literal, BM25, semantic, and RRF phases for instant abort.
- **Auto-cancel on keystroke:** Typing a new query while a search is in flight automatically cancels the stale search before starting the debounce timer.

## [0.6.9] — 2026-04-25

### Fixed
- **TUI phantom text rendering:** Fixed an issue where resizing panels or rendering shorter snippets left phantom artifacts ("ghost text") from previous renders.
- **Live formatting progress:** The TUI now displays an active progress bar with precise chunks/percent estimates in the status bar while indexing or enhancing in the background.

## [0.6.7] — 2026-04-25

### Fixed
- **TUI pre-filled query hang:** Fixes an issue where running `ig --ui <query>` would hang with a blinking cursor before rendering the TUI, because the search blocked the initial draw. The TUI now renders immediately with a "Searching…" status.

## [0.6.6] — 2026-04-25

### Added
- **Mouse support:** Click to focus panels (search input, file list, snippet panel). Scroll wheel navigates file/snippet lists or scrolls the file view.
- **Draggable panel separator:** Click and drag the border between the file list and snippet panel to resize (15%–70% range).
- **Tab / Shift+Tab cycling:** Tab cycles focus forward (Search → FileList → SnippetList → Search), Shift+Tab cycles backward.

### Changed
- Status-bar hints updated to reflect Tab and mouse shortcuts.

### Tests
- 11 new unit tests for rect hit-testing, split percent clamping, drag state, and Tab cycling logic. Total test count: 211.

## [0.6.5] — 2026-04-25

### Changed
- **TUI: "Searching…" indicator appears before blocking search** — the status bar now renders the pending state before the search query blocks the main thread, so the UI no longer appears frozen during slower queries.
- **TUI: FileView rendering cached** — syntax-highlighted file views are cached as pre-rendered line vectors, eliminating per-frame re-highlighting lag on large files.
- **TUI: Enter key transition fixed** — pressing Enter in Search mode now properly triggers the search and transitions to FileList only after results arrive.

### Fixed
- **Clippy compliance:** resolved `type_complexity` (new `FileViewCache` type alias) and two `collapsible_match` lints by folding conditions into match guards.

### Added
- **27 new TUI unit tests** covering file/snippet navigation wrapping, mode transitions, rendering pipelines (dividers, scores, highlights), flash messages, reset state, path resolution, and hit grouping. Total test count: 200.



### Changed
- **TUI Redesign — Hierarchical Code Browser:** The interactive TUI (`ig --interactive`) has been completely rebuilt with a four-mode navigation model: **Search → FileList → SnippetList → FileView**. Files are now deduplicated in the left panel with hit counts; the right panel shows syntax-highlighted snippet previews that become individually navigable on Enter. Pressing Enter again expands the full file with line numbers, gutter highlighting on matched regions, and scrolling.
- **Editor integration via `e` key:** Press `e` at any level to open `$EDITOR` at the matched line. Enter no longer opens an external editor — it navigates deeper into the result hierarchy.
- **Clipboard copy via `y` key:** Copy `file:line` to the system clipboard using `arboard`.
- **Esc/Ctrl+C clear-then-quit:** In the search box, Esc clears the query first; pressing Esc again (or when empty) exits the TUI.
- **Status bar with mode-dependent hints:** Every mode shows context-sensitive key bindings.
- **Visual polish:** Proper `────` divider lines between snippets, higher-contrast color scheme, stronger selection highlights, mode indicator in the title bar.

### Fixed
- **README roadmap:** Removed TUI from the future roadmap (shipped in 0.6.2). Added `--interactive` and `--literal` to the CLI reference.


## [0.6.2] — 2026-04-20

### Added
- **Killer TUI Mode!** You can now launch an interactive `ratatui`-powered Terminal User Interface by running `ig -i` or `ig --interactive`. It supports real-time substring/semantic search as you type, and previews source files with `syntect` syntax highlighting natively within the terminal.


## [0.6.1] — 2026-04-20

### Improved
- **Documentation and Branding:** Complete visual and content overhaul of the documentation site, highlighting the new MCP server architecture with interactive animations and setup guides.
- **MCP Server Capabilities:** Enhanced E2E integration covering full lifecycle queries (`tools/list`, `tools/call`, `ig_status`, `ig_search`).
- **Daemon Resilience:** Better recovery logic handling stale UNIX domain socket binding collisions natively across restarts.
- **CI Modernization:** Removed minor checkout version skew across parallel workflows.

## [0.5.54] — 2026-04-13

### Fixed
- **Watcher registration TOCTOU race:** Concurrent requests to watch the same workspace could both pass the `contains_key` check and create duplicate watchers, silently leaking the first watcher's tokio task and file descriptor. The lock is now held across check+build+insert.


## [0.5.53] — 2026-04-13

### Fixed
- **Semantic scope leakage:** directory-scoped semantic searches now escape SQLite `LIKE` wildcards in scope paths, so `_` and `%` in real directory names no longer leak hits from similarly named siblings.
- **Hybrid recall under scoped search:** semantic candidate collection now re-checks `scope_matches()` before scoring and truncation, preventing out-of-scope chunks from stealing top-K slots.

## [0.5.52] — 2026-04-13

### Added
- **E2E verification:** Added a full E2E CLI test preventing regressions in worktree overlay invalidation.

## [0.5.51] — 2026-04-13

### Fixed
- **Worktree overlay staleness:** Track base index generation so worktree overlays can rebuild automatically instead of returning stale results when the base index updates.

## [0.5.50] — 2026-04-13

### Fixed
- **Critical: prevent silent data loss on crash.** Merkle snapshot was saved before index stores were committed — a crash (SIGKILL/OOM/power loss) between snapshot save and final commit left the snapshot claiming files were indexed while stores were empty/partial. On next run, the diff was empty and missing files were silently never re-indexed. The snapshot is now saved after all store commits, making it a true high-water mark of persisted state
- **Crash detection safety net:** `index_health_with_options` now detects a stale `.indexing.pid` file (left behind when SIGKILL bypasses the IndexingGuard's Drop) and marks the index as Unhealthy, forcing a rebuild on the next run
- **Atomic Merkle snapshot writes:** `MerkleSnapshot::save()` now uses write-to-tmp + `fs::rename()` instead of bare `fs::write()`, preventing truncated JSON on crash during save
- **Test-path false positives:** `is_test_path()` used bare `.contains("test")` which penalized files like `attestation.rs`, `contest.rs`, `inspect.py` as test files. Replaced with boundary-aware matching using directory segments (`tests/`, `__tests__/`) and filename conventions (`_test.`, `.test.`, `test_`)

## [0.5.49] — 2026-04-12

### Fixed
- **CI daemon recovery coverage:** the end-to-end watcher recovery test now explicitly opts back into daemon autospawn, so it exercises the real recovery path even under CI’s `IVYGREP_NO_AUTOSPAWN=1` guard

## [0.5.48] — 2026-04-12

### Fixed
- **Watcher daemon recovery:** `ig --add` now autospawns the daemon when watch mode is enabled, so newly indexed workspaces do not get stuck as “configured” without a live watcher
- **Daemon startup recovery:** restarting `ig --daemon` now restores filesystem watchers for already indexed workspaces that were previously configured with watch mode
- **Query-path recovery:** a normal query now revives an offline watcher for watch-configured workspaces instead of leaving status permanently degraded
- **Status clarity:** `ig --status` now reports `watcher offline` instead of the vaguer `daemon stale`, which better matches the actual failure mode

## [0.5.47] — 2026-04-12

### Fixed
- **Stale legacy runtime PID cleanup:** `ig doctor --fix` now removes dead legacy watcher, indexing, and enhancement PID files instead of only reporting them
- **Query-path self-healing:** normal CLI and MCP searches now clean stale legacy runtime PID files before searching, so old runtime markers stop lingering until a manual repair
- **False stale warnings:** doctor now checks whether legacy PID files still point to a live process before flagging them as stale

## [0.5.46] — 2026-04-12

### Improved
- **No-op reindex hot path:** restored incremental `index_workspace()` performance by using a cheap health check on the clean fast path and deferring full storage verification until an actual write is needed
- **Resilient self-healing without benchmark tax:** suspicious or corrupt index storage still rebuilds automatically, but healthy indexes no longer pay the full doctor-grade verification cost on every no-change reindex
- **Linux job liveness checks:** PID start-time verification now reads `/proc/<pid>/stat` instead of spawning `ps`, reducing background bookkeeping overhead on the common path

### Added
- **Critical benchmark guard:** the benchmark workflow now compares `indexer/incremental_reindex_no_change` against the base ref on the same runner and fails fast on regressions above the configured threshold

## [0.5.45] — 2026-04-12

### Added
- **Persistent job ledgers:** each workspace now tracks watcher, indexing, and enhancement jobs in `job.json` with generation, heartbeat, phase, attempt count, PID identity, and last error details
- Recovery-focused tests covering stalled job detection, watcher event storms, parser-backed language retrieval, and watcher-triggered reindexing
- Tree-sitter AST chunking for **Java, C#, PHP, Ruby, and Swift**

### Improved
- **Watcher stability:** background file watching now uses per-workspace coalescing (`dirty` + `indexing` + rerun-once semantics) instead of an unbounded event queue, eliminating redundant full reindexes during save storms
- **Status accuracy:** `ig --status` now distinguishes “configured to watch” from “watcher alive”, and reports stalled indexing / enhancement jobs instead of showing them as indefinitely active
- **Doctor coverage:** `ig doctor` now flags stale legacy PID files, stale job heartbeats, long-paused neural enhancement, and watcher queue saturation symptoms
- **Watcher reindex correctness:** daemon-triggered updates now bypass the watcher short-circuit and actually process filesystem mutations
- **Configuration fidelity:** indexing now preserves the workspace’s requested watch mode instead of silently forcing `watch_enabled = true`

## [0.5.44] — 2026-04-11

### Added
- **`ig doctor` / `ig doctor --fix`:** new index-health inspection and repair flow for stale, partial, or corrupted local indexes
- Relevance regressions for natural-language implementation queries and source-file lookup
- Workspace health classification covering `not_indexed`, `healthy`, `healthy_empty`, and `unhealthy`

### Improved
- **Self-healing index detection:** `workspace_is_indexed()` now refuses zero-chunk indexes when the workspace still has indexable files, so broken indexes rebuild automatically instead of returning empty results
- **Natural-language query understanding:** stopword filtering, light intent normalization, file-stem boosts, and location-intent ranking make plain-English queries less likely to drift into tests or unrelated helpers
- **Semantic resilience:** hash vectors remain available immediately, neural vectors are used as an upgrade when present, and small repositories can complete neural enhancement before the first search returns
- Documentation now distinguishes Tree-sitter AST chunking for core languages from heuristic structural chunking for the broader 44-language registry

## [0.5.43] — 2026-04-11

### Added
- **Code-aware tokenizer:** Custom BM25 tokenizer splits camelCase, snake_case, dots, colons, and path separators so that natural-language queries like "handle error" natively match `handleError`, `handle_error`, and `HandleError` at the BM25 scoring level
- **BM25F multi-field scoring:** New `file_path_text` (5× boost) and `signature` (10× boost) fields bring Sourcegraph/Zoekt-style field-level relevance — function definitions and filename matches rank significantly higher than body text
- **Literal variant expansion:** The literal pass now tries snake_case, camelCase, and compact variants of the query, so "hybrid search" also matches `hybrid_search` and `hybridSearch` as exact substrings
- **Definition-kind boost:** 2× post-BM25 multiplier for Function, Class, Struct, Trait, Interface, Impl, Enum, and Module chunks counteracts BM25's document-length normalization penalty on large definitions
- Tests for code-aware tokenizer covering camelCase, snake_case, path separators, function signatures, and natural-language queries
- BM25F relevance test proving definition-site ranking via signature boost

### Improved
- Lexical search now uses code-aware tokenization instead of Tantivy's default `SimpleTokenizer`, eliminating the reliance on post-hoc query expansion for identifier matching
- Both literal and lexical search passes search across all BM25F fields for broader candidate recall
- Increased default candidate limit from 100 to 500 to ensure BM25 retrieves definition chunks even for high-frequency terms
- Softened file density normalization from 1/√n to 1/n^0.3 to preserve definition-site signal in files with many matching chunks

## [0.5.42] — 2026-04-11

### Fixed
- **Literal search recall:** Top-level code (imports, constants, type aliases) outside functions/classes was silently dropped by the tree-sitter chunker, causing `ig gquota` to miss matches that `rg` would find
- **Gap-filling chunks:** Tree-sitter chunker now emits `Module`-kind chunks for any source lines not covered by function/class AST nodes, ensuring full index coverage

### Added
- CLI e2e tests for literal and hybrid search against top-level string constants
- Unit tests for chunker gap-filling and literal search recall

## [0.5.41] — 2026-04-10

### Improved
- **Search relevance overhaul:** Rebalanced hybrid RRF scoring weights for significantly better result quality
- **Definition-site ranking:** New `definition_name_boost` signal strongly prefers function/class definitions over usage sites
- **Query expansion:** Automatically generates `snake_case` and `camelCase` variants (e.g., "error handling" → `error_handling`, `errorHandling`)
- **Density-aware literal scoring:** Exact-match pass now scores by occurrence count instead of flat 1.0
- **Stronger semantic-only penalty:** Chunks found only by semantic search (no lexical/literal confirmation) are more aggressively demoted
- **Zero-coverage noise filter:** Chunks with no query term overlap get an additional penalty
- **Path-segment boost increased:** File path matching (e.g., "search" → `search.rs`) is now 2.5× more influential

### Added
- 5 new relevance-focused integration tests: snake_case matching, camelCase matching, definition-site ranking, file-path boosting, semantic-only penalty verification

## [0.5.39] — 2026-04-09

### Added
- **MCP server `ig_status` tool:** Added MCP tool to list indexed projects and check index status.

## [0.5.13] — 2026-04-07

### Performance
- **32x larger enhancement batches:** Increased ONNX inference batch size from 16 to 512 chunks, dramatically reducing session overhead during background neural enhancement
- **Skip decompression for completed keys:** Enhancement loop now checks vector store before decompressing text, avoiding ~1M redundant zstd decompressions on resume
- **CPU affinity limiting (Linux):** Background enhancement now uses `sched_setaffinity` to pin ONNX threads to 25% of available cores (capped at 4), keeping the system responsive during long-running enhancement
- **Instant initial indexing:** `ig --add` now always uses the lightweight hash model for initial indexing; neural enhancement runs exclusively in the background daemon

### Fixed
- **Backward compatibility for `is_ignored` field:** Tantivy field is now optional, allowing v0.5.13 to seamlessly read indexes created by older versions without crashing
- **Honest CUDA detection:** Added cuDNN probe to verify CUDA is actually functional before reporting GPU acceleration in `ig --status`

## [0.5.12] — 2026-04-06

### Performance
- Bounded ONNX/GPU allocations by enforcing maximum chunk counts for embeddings, capping VRAM well below 8GB during massive batches
- Fixed a bug where initial indexing incorrectly instantiated the background neural model even when `--hash` was passed

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
