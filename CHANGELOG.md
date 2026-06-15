# Changelog

All notable changes to ivygrep are documented in this file.

## [Unreleased]

## [0.10.1] - 2026-06-15

### Fixed
- **Linux x86 releases run on baseline x86-64 CPUs.** The static musl artifact no longer requires x86-64-v3 instructions, which caused `v0.10.0` to terminate with `Illegal instruction` on older valid x86_64 hosts.

### Testing
- **Release portability flags are regression-tested.** The workflow test rejects native and elevated x86-64 microarchitecture requirements in distributed Linux artifacts.

## [0.10.0] - 2026-06-15

### Added
- **Public code-retrieval evaluation.** A BEIR/CoIR-style runner reports nDCG@10, MRR@10, precision@5, recall@20, indexing cost, index size, and cold/warm latency with an offline multilingual CI fixture.
- **Persisted symbol and call graph.** `ig --symbol`, `ig --refs`, and `ig --callers` query exact definitions and callers across normal indexes and worktree overlays.
- **Portable Windows vector backend.** Native hash-only Windows builds, E2E coverage, release archives, and checksums no longer depend on USearch/SimSIMD compiling under MSVC.
- **Opt-in code embedding profile.** `IVYGREP_MODEL_PROFILE=code` selects a pinned compact CodeSearchNet-trained MiniLM checkpoint, with profile identity persisted beside neural vectors.

### Changed
- **Neural vectors now use F16 storage.** Index format v10 rebuilds incompatible stores, materially reduces neural payload size, and exposes per-component index sizes.
- **Ranking work is bounded.** Coverage-aware hash/neural fusion preserves partial-index recall, exact symbol evidence feeds hybrid ranking, and expensive deterministic boosts examine at most `IVYGREP_RERANK_LIMIT` candidates (100 by default).
- **Large-repository status is metadata-backed.** Normal status and doctor reads use cached counts; `ig --doctor --deep` is the explicit full integrity scan. Indexed no-watch workspaces also reuse the daemon unless auto-spawn is disabled.
- **Release artifacts are stripped and size-gated.** Release binaries and archives enforce an 80 MiB budget.

### Testing
- Added multilingual symbol CRUD/worktree coverage, portable vector persistence and recovery tests, F16/F32 recall and size checks, model-profile compatibility tests, Windows CI/E2E lanes, and public retrieval metric tests.

## [0.9.7] - 2026-06-14

### Fixed
- **Duplicate stable vector keys no longer break enhancement.** Indexing and enhancement deduplicate vector keys before graph insertion, while health checks compare vector stores against distinct keys instead of raw chunk rows.
- **Successful explicit hash enhancement clears stale status safely.** Completed runs remove obsolete error and progress markers without overwriting a concurrently active enhancement job.

### Performance
- **Large-index vector enhancement scales with sequential reads and fewer graph rewrites.** Hash and neural passes scan SQLite in storage order, suppress duplicate keys per batch, checkpoint less often, and skip no-op hash saves.

### Testing
- **Duplicate-key and benchmark workspace regressions are covered.** The suite exercises repeated stable keys, isolates temporary benchmark repositories from ancestor Git directories, and keeps regex benchmarks lexical-only.
- **macOS Intel E2E jobs have enough time for the full release matrix.** The timeout now reflects the slower native Intel runner.
- **Release-note extraction is portable across `awk` implementations.** Tag builds match changelog version headers without relying on environment-specific backslash handling.

## [0.9.6] - 2026-06-12

### Fixed
- **Natural-language ranking no longer penalizes large implementation files for having many relevant chunks.** File-level evidence now aggregates query coverage before per-file result diversity is applied.
- **Stopwords are removed before singularization.** Words such as `does` no longer become bogus search terms such as `doe`.
- **Code-search query expansion covers common implementation vocabulary.** Portable aliases connect intent terms such as choosing, validation, queues, allocation, receiving, and tracking to common code identifiers without repository-specific paths.

### Performance
- **Large-repository relevance improved without material daemon regression.** On a 93,502-file, 4.42-million-chunk Linux checkout, the portable intent-query score improved from `6.33` to `41.20`; MRR@10 improved from `0.059` to `0.490`, nDCG@10 from `0.055` to `0.416`, and recall@20 from `0.205` to `0.603`. Fresh shared x86 validation measured `79 ms` warm daemon cache-replay p95 and `137 ms` process-cold p95.

### Security
- **Release and CI dependencies are immutable.** GitHub Actions now use pinned commit SHAs, checkout credentials are disabled by default, and workflow permissions follow least privilege.
- **Security checks run continuously.** CI audits Rust dependencies, scans the current tree for leaked secrets, and validates workflows with actionlint and zizmor.
- **Dependency updates are automated without PR floods.** Dependabot monitors Cargo and GitHub Actions dependencies weekly and groups each ecosystem into one update.
- **The lockfile no longer uses a yanked crate release.** `fastrand` is locked to the supported `2.3.0` release.

### Testing
- **Build, test, benchmark, and release jobs enforce `Cargo.lock`.** Cargo commands now use `--locked` so local validation and published binaries resolve the same dependency graph.
- **Git-backed tests are isolated from developer signing settings.** Synthetic fixture commits explicitly disable signing, keeping the suite hermetic on Linux and macOS.

## [0.9.5] — 2026-06-07

### Fixed
- **Query repair still recovers corrupt stores after faster preflight.** Local hybrid, literal, regex, and daemon-fallback searches now retry after rebuilding an unhealthy index when a corrupt store causes an error or empty result, preserving hash search while neural vectors are unavailable.

### Performance
- **Local query startup skips full index audits.** Query preflight now uses quick health checks instead of opening and cross-validating SQLite, Tantivy, and vector stores on every invocation. On the Linux kernel index, complex hot query p95 dropped from roughly 300 ms to about 53 ms while result counts and relevance stayed stable.

### Testing
- **Corrupt Tantivy repair is covered end to end.** CLI snapshot coverage now corrupts Tantivy metadata, verifies full health detects the issue, and confirms the next query repairs the index and returns expected results.

## [0.9.4] — 2026-06-05

### Fixed
- **Daemon IPC rejects malformed, oversized, and incompatible requests clearly.** Requests now carry an explicit protocol version, the daemon caps request lines at 1 MiB, and malformed or mismatched input receives a structured JSON error instead of dropping the connection.
- **Default query expansion is repository-neutral.** Removed ripgrep-specific aliases and stopped injecting lexical aliases into semantic query text; the deterministic relevance suite improved MRR from 0.620 to 0.627 and recall@5 from 0.761 to 0.804.

### Performance
- **Hybrid search moves candidates through the hot path without full-chunk clones.** ANN collection, RRF fusion, score filtering, per-file grouping, and hit materialization now transfer owned chunks and source lists instead of cloning decompressed text per stage.
- **Foreground indexing reuses one path string per file.** SQLite and Tantivy ingestion share the same borrowed path, and Tantivy documents copy fields directly instead of cloning them before insertion.

### Testing
- **Real-repository stress relevance remains intact.** All ignored stress tests pass, including all 10 ripgrep deep-relevance queries after removing corpus-specific aliases.
- **IPC tests cover missing/mismatched protocol versions and oversized/malformed requests.**

## [0.9.3] — 2026-06-04

### Fixed
- **Doctor detects cross-store index drift before it becomes silent search loss.** Full health checks now compare live SQLite and Tantivy chunk counts, verify completed hash-vector cardinality, validate optional neural stores, and flag SQLite paths missing from the Merkle snapshot.
- **Missing Merkle snapshots force a rebuild.** Quick health checks no longer treat an index without its incremental-diff snapshot as healthy, preventing stale chunks from surviving deleted source files.
- **Malformed vector stores fail safely before native loading.** Serialized dimensions, length, and header magic are validated before USearch mmap/load calls, avoiding native crashes on truncated or malformed-header index files.
- **Benchmark guards preserve developer checkouts.** Regression comparisons reject dirty worktrees and restore the original branch or detached commit after measuring both revisions.

### Performance
- **Hybrid benchmarks now measure semantic search.** Indexed Criterion fixtures complete hash-vector enhancement and assert semantic evidence reaches representative phrase-query results instead of accidentally measuring empty-vector lexical-only queries.
- **Hot ANN latency has dedicated 50K-vector coverage.** Critical-journey benchmarks now separate mmap/open overhead from repeated in-process vector search.

### Testing
- **Daemon/local search equivalence runs in CI and release builds.** Representative hybrid, literal, regex, type, glob, scope, and multi-workspace queries must return equivalent results through both execution paths.
- **Python harness tests run in local full validation and CI.** Benchmark and conversion helper regressions are no longer outside the default gate.
- **Documented E2E procedures cover regex search.** Release and cross-platform CLI smoke tests now validate the regex path alongside literal, scoped, filtered, status, doctor, add, and remove workflows.
- **Test and benchmark harnesses disable production load throttling.** Explicit enhancement runs finish deterministically on busy development and CI hosts.

## [0.9.2] — 2026-06-04

### Fixed
- **Benchmark regression guards survive force-pushed baselines.** Benchmark CI now fetches the push or pull-request baseline commit when it is missing locally, so valid history rewrites no longer fail before the comparison runs.

### Testing
- **Cross-platform stress coverage is runnable and recurring.** Linux x86_64 E2E bootstraps its required stress fixtures before ignored stress tests and runs that coverage on scheduled and tagged workflows, while manual dispatch can still opt into stress explicitly.

## [0.9.1] — 2026-05-31

### Fixed
- **Linux CUDA source builds work on current Candle.** Updated the Candle dependency stack and CUDA forward-call integration so `--features cuda` builds and runs on Linux without requiring cuDNN.
- **RTX 50/Blackwell CUDA builds infer compute capability when NVML is unavailable.** `build.sh` and `test.sh` now set `CUDA_COMPUTE_CAP=120` on detected RTX 50/Blackwell hosts when `nvidia-smi` cannot report compute capability, while still honoring an explicit `CUDA_COMPUTE_CAP`.
- **Accelerator backends self-test before use.** Metal or CUDA initialization now falls back to local CPU inference if the loaded backend returns invalid validation embeddings, avoiding broken persisted neural vectors on backend/runtime regressions.
- **Stress fixture bootstrap repairs corrupt shallow clones.** `scripts/bootstrap_stress_fixtures.sh` now reclones unhealthy repo fixtures instead of reusing a corrupt `.git/index`.

### Testing
- **Linux CUDA backend smoke is documented and verified.** README and architecture docs now include `./build.sh --features cuda` plus `e2e_neural_backend.sh --expect-backend "Candle CUDA"`.
- **Metal opt-in smoke accepts safe CPU fallback.** CI now treats `Candle CPU (Accelerate)` as valid when the Metal self-test rejects the accelerator backend.
- **Large text stress queries avoid low-confidence phrase filtering.** Shakespeare and Alice stress checks use deterministic indexed terms, and multi-workspace stress no longer mutates `IVYGREP_HOME` from worker threads.

## [0.9.0] — 2026-05-31

### Changed
- **Index format bumped to v8 -- upgrading triggers a one-time full reindex.** Foreground indexing commits SQLite and Tantivy before ANN construction. Hash ANN and neural tiers now enrich asynchronously with stable vector keys, generation tracking, and tombstone journals.

### Performance
- **Fresh indexing becomes queryable before ANN construction.** Hash graph mutation moved out of foreground indexing into resumable background enhancement, so first lexical results no longer wait for HNSW ingest or persistence.
- **Watcher updates use targeted Merkle deltas.** Safe changed-file updates avoid full-tree snapshot walks while preserving full-rebuild fallbacks for broad or ambiguous filesystem events.
- **Focused searches filter before candidate caps.** Workspace scope and include/exclude globs now reduce lexical, semantic, and regex candidate pools before truncation, improving speed and recall for narrow searches.

### Fixed
- **Regex scope searches fall back after truncated prefilter results.** Regex search no longer misses valid scoped matches when Tantivy prefilter caps omit later files.
- **Background vector enrichment remains correct across edits.** Hash and neural tombstone journals remove stale keys after lexical updates, while generation markers schedule repair when indexing races enrichment.
- **Provisional hash evidence no longer overpowers lexical and literal matches.** Hash fusion weights now keep the first-tier ANN signal useful without displacing stronger direct evidence.

### Tooling
- **Daemon benchmarks distinguish cache replay from distinct warm misses.** Hot-query measurement now reports both cases explicitly.
- **Relevance evaluation reports foreground and hash-enriched tiers separately.** Explicit enrichment checks bypass production background throttling so CI and local gates complete deterministically.

## [0.8.1] — 2026-05-30

### Fixed
- **Dependency security updates.** Updated the TUI dependency stack to remove vulnerable `lru@0.12.5` and updated `rand` to `0.9.3`; release artifacts now contain the patched resolutions.

### Maintenance
- **Release workflows use supported GitHub Actions runtimes.** Updated QEMU and release-publishing actions to their Node 24 generations, removing Node 20 deprecation annotations from release and cross-platform E2E workflows.

## [0.8.0] — 2026-05-30

### Changed
- **Index format bumped to v6 -- upgrading triggers a one-time full reindex.** Bazel/Starlark files now store first-class language metadata and Starlark macro AST chunks, very large BUILD-like sources split top-level target calls into AST chunks, and `.tsx` files use the TSX grammar rather than the plain TypeScript grammar; existing stored chunks must be rebuilt to pick up these behaviors.
- **Index format bumped to v4 -- upgrading triggers a one-time full reindex.** Unix Merkle fingerprints now include inode change time (`ctime`) in addition to size and mtime, so a same-size edit followed by restored mtime cannot leave the index stale. The path remains part of the root hash through the snapshot map key. Verification stays metadata-only: no full-repository content reads were added.
- **Index format bumped to v3 — upgrading triggers a one-time full reindex.** A definition's leading doc-comment/attribute lines are now folded into the following function/class chunk instead of being emitted as standalone single-line `Module` chunks. On a representative tree this removed ~40% of chunks (less to embed, store, and search), and search now returns the documented definition rather than a bare comment line.

### Added
- **Starlark/Bazel coverage.** `BUILD`, `BUILD.bazel`, `WORKSPACE`, `MODULE.bazel`, `.bzl`, `.bazel`, and `.star` files are first-class `starlark` sources; Tree-sitter splits macro definitions in `.bzl`/`.star` files and target calls in very large BUILD-like sources into retrievable units while ordinary BUILD files retain bounded text chunks.
- **Skip minified bundles / single-line blobs when indexing.** A file with a 50 KB+ run and no line break (minified JS/CSS, packed data) is skipped during indexing — it would otherwise become one enormous, low-value chunk that dilutes relevance on large monorepos. Complements the existing 16 MB file-size cap, catching minified files that fall under it. Large hand-written docs (normal line lengths) are unaffected.

### Fixed
- **Cancelled Tree-sitter parses no longer persist partial chunks.** When structural parsing exceeds its time budget, indexing follows the normal fallback chunking path instead of using an incomplete partial syntax tree.
- **`.tsx` source uses Tree-sitter's TSX grammar.** TypeScript React files were registered as TypeScript but parsed with the non-JSX grammar, allowing parse errors to degrade structural chunks.
- **MCP no longer neural-embeds the whole repo inline on the first query (#56).** The MCP auto-index now uses the fast hash model and defers neural embeddings to a background subprocess — mirroring the daemon — so the first query returns quickly even on very large repos. Previously the inline neural pass could block the first query for many minutes and, run by several MCP clients at once, saturate the host.
- **MCP caches its neural query model per process (#57).** It was reconstructed on every `ig_search` (and even for `literal`/`regex` modes that never embed a query); it is now loaded once via `OnceLock`, only in the hybrid path.
- **Background neural enhancement no longer stalls on busy machines (#62).** It paused whenever the 1-minute load average exceeded ~0.75–0.8× the CPU count, so on a routinely-busy host (a dev box mid-build, a shared machine) neural vectors were never built and search stayed on the lower-quality hash path. The subprocess is already `nice(10)` and capped at ~25% of cores, so the threshold is now a more lenient 2.0× and configurable via `IVYGREP_ENHANCE_MAX_LOAD_RATIO` (≤ 0 disables the load check). Battery, thermal, and low-memory pauses are unchanged.
- **Background neural enhancement no longer deadlocks (#69).** `embed_batch` fanned out across texts with rayon on the *global* thread pool, but candle's CPU kernels also parallelize their matmuls on that same global pool — so each embedding's nested rayon work could never be scheduled (every worker parked in `collect()`/`Sleep::sleep`) and enhancement hung at ~0% CPU after an initial burst, never finishing the neural vectors. This is the likely cause of parallel MCP enhancement saturating a host. The fan-out now uses dedicated OS threads (`std::thread`), leaving candle's global-pool work schedulable; a ~5.7k-chunk repo that previously hung now enhances in ~60s. Affects every neural model.
- **Watcher-triggered indexing is now bounded by the daemon CPU semaphore (#72).** File-watch re-indexing ran its `spawn_blocking` work *outside* the #58 concurrency limit, so a change dirtying many watched workspaces at once (a multi-repo branch switch or build) could spawn unbounded parallel indexing — saturating the rayon chunking pool and the Tokio blocking pool exactly the way the #58 client-burst limit was meant to prevent, just on the watcher axis. Watcher indexing now acquires a CPU permit like client requests.
- **Privacy and neural-backend behavior now matches reported runtime (#75).** Search and embedding inputs stay local; neural mode fetches model artifacts on first use through `hf-hub`. Current releases remain local CPU builds (Accelerate-backed on macOS). Opt-in Metal and CUDA builds execute local accelerator paths when available, and `ig --status` reports the recorded backend that generated neural vectors.

### Added
- **Daemon concurrency limit (#58).** Heavy hybrid/literal/regex search and index work is gated behind a `Semaphore` sized to the CPU count, providing backpressure instead of spawning unbounded blocking tasks (Tokio's blocking pool defaults to 512 threads) under a burst of clients.
- **Relevance eval gate wired into CI (#20).** A new opt-in `Relevance` workflow runs `scripts/eval_relevance.py` as a deterministic hash-path gate on PRs touching ranking/indexing/eval code, catching regressions in fusion/boosting/chunking. The harness also now reliably measures the **neural** path locally (`--neural`): it previously set `IVYGREP_NO_AUTOSPAWN` and never built neural vectors, so it silently scored the hash fallback while labeling it "neural"; it now builds neural vectors up front. The labeled query set grew from 15 to 23 intent-style queries over ivygrep's own tree.

### Performance
- **Background neural embedding now uses the full background core budget.** `candle_embed`'s `embed_batch` runs single-text forward passes sequentially behind one mutex, so embedding pinned ~1 core no matter the thread budget — building neural vectors for a repo with millions of chunks took many hours. The embedder is now a small pool (one instance per background worker thread) and forwards run in parallel across worker threads, so the enhancement pass uses its allotted ~25%-core budget (measured ~3–4× CPU utilization, i.e. proportionally faster wall time when cores are free) **without capping which chunks get neural vectors**. The foreground query model stays a single instance.
- **Tree-sitter chunking reuses parsers and compiled queries (#29).** Initial indexing previously allocated a parser and compiled the grammar query for every source file. Parsers are now reused per indexing worker and immutable queries are cached per grammar. The existing synthetic Rust chunk benchmark dropped from 11.371 ms to 1.271 ms median (~8.9× faster).
- **Hash first-tier HNSW builds use a lower-cost graph profile (#30).** The provisional 256-dim `F16` vector store now uses `connectivity=8` and `expansion_add=32` while retaining `expansion_search=64`; neural `F32` stores keep usearch quality defaults. The new synthetic hash-ingest benchmark drops from 1.616 s to 261 ms median for 5,000 vectors (~6.2× faster), while the deterministic hash relevance gate remains at MRR 0.579 / recall@5 0.761.
- **Metal neural execution remains opt-in after measured validation.** On the checked-in 10,787-chunk `tantivy` stress fixture, Accelerate CPU enhancement completes in 135s at 704 MB peak RSS; the current single-stream Metal path completes in 402s at 383 MB. Metal reduces memory but is not a faster background default until it gains safe batching or concurrency.

### Testing
- Added regression coverage: MCP auto-index builds 256-dim hash vectors (not 384-dim neural inline), MCP query-model caching, daemon CPU-concurrency bound, and leading-comment chunk folding.
- **Large-repo stress harness (`scripts/stress_large_repo.sh`).** Drives index → neural-enhance → query on a target repo and reports per-phase wall time, peak RSS, chunk/file counts, and query latency, with a per-phase watchdog that flags hangs (e.g. the enhancement deadlock class). Emits metrics only — never file paths or contents — so it is safe to run on private repositories.
- **Filtered Criterion checks no longer initialize unrelated fixtures (#76).** Targeted performance guards lazily construct only selected benchmark data while full-suite runs still share and execute their selected fixtures.
- **Parser reuse is covered across grammar switches.** A regression test parses Go, TypeScript, and Rust successively on one thread, and a small-file Criterion case tracks per-file chunking setup overhead.
- **Hash-only scale measurement stays hash-only (#78).** `scripts/stress_large_repo.sh --skip-enhance` now passes `--hash` during query measurement, so it cannot load neural model artifacts or report neural startup as hash-query latency.
- **Hash-vector construction has a dedicated Criterion case.** `hash_vector_build/ingest_5k_hash_vectors` isolates provisional `F16` graph construction, and unit coverage asserts the faster profile is not applied to neural stores.
- **Neural accelerator execution has a dedicated smoke procedure.** `scripts/e2e_neural_backend.sh` runs neural enhancement on a throwaway fixture and verifies persisted backend reporting; a macOS ARM opt-in CI lane requires real `Candle Metal` execution.

## [0.7.0] — 2026-05-23

### Changed
- **Index format bumped to v2 — upgrading triggers a one-time full reindex.** An `index_format_version` sentinel is recorded on each successful index; an older-format index is detected as unhealthy and rebuilt before serving queries. Worktree overlays also migrate an outdated base index before referencing it.

### Fixed
- **Semantic similarity is no longer discarded.** Vector search returned `-distance` (range `[-2, 0]`), which downstream normalization clamped to `0`, so the semantic magnitude signal was lost and search degenerated toward lexical-only. Search now returns the true cosine similarity.
- **Neural results are no longer diluted by hash embeddings.** When neural vectors exist they are used as the primary semantic signal; the low-quality 256-bucket hash store is kept only as a low-weight fallback so chunks not yet neural-embedded (partial/incremental enhancement) still get a semantic candidate.
- **Path matches no longer override content relevance.** The fake high-BM25 sentinel injected into the lexical pool was removed; path matches now form their own bounded rank-fusion list, deduplicated by chunk id keeping the highest score.
- **Ranking boosts no longer dominate the fused score.** Coverage/path/file-stem/definition boosts are bounded relative to the RRF base so they perturb rather than replace the fused ranking.
- **usearch vectors are keyed by the unique chunk id, not the content hash.** Identical-content chunks (license headers, boilerplate) no longer collide onto one vector key, so deleting one file can no longer drop a still-live chunk's vector from another file.
- **Phrase-alias expansion requires a contiguous token window**, eliminating spurious expansions when alias terms merely co-occur.

### Added
- **Daemon single-instance lock.** An exclusive lock is acquired before binding the IPC socket, preventing a restart/auto-spawn race from leaving two daemons fighting over the socket (and a zombie holding file watchers).
- **Self-contained relevance evaluation harness** (`scripts/eval_relevance.py`) with a labeled query→file dataset over ivygrep's own tree, reporting precision@k, recall@k, MRR, and nDCG with a `--check` gate.

### Testing
- Added regression coverage for cosine-magnitude scoring, the hash fallback under partial neural coverage, vector-key collisions, the index-format migration/rebuild, and worktree base-index migration.

## [0.6.20] — 2026-05-18

### Testing
- **Benchmark smoke runs no longer compare against stale local Criterion baselines.** `./bench.sh` now uses a temporary smoke baseline by default, with `--keep-baseline` available for intentional local history comparisons.
- **Fast benchmark cases now collect longer timed samples.** Short per-operation results are repeated inside Criterion so the measured sample windows stay stable while reports still show per-operation latency.
- **Benchmark docs now explain per-operation timing.** README, testing notes, and `./bench.sh --help` clarify why sub-100ms reported results can still be backed by multi-second timed samples.

## [0.6.19] — 2026-05-18

### Changed
- **Generic relevance scoring shipped from data-driven aliases.** Query expansion now comes from the generated alias dictionary instead of hardcoded codebase-specific rules.
- **Docs and site now match the source language registry.** Language/file-type claims are aligned with the 44 registered handlers, including Thrift, Markdown, XML, config, JSON, and text buckets.
- **Onboarding scripts are the documented default path.** README and testing docs now point contributors to `./build.sh`, `./test.sh`, `./bench.sh`, and the documented procedure E2E smoke.

### Testing
- **Documented CLI procedures now run end-to-end in CI, release, and cross-platform E2E.** The shared smoke covers help/version/status, first-run auto-indexing, explicit add/remove, scoped search, include/exclude filters, literal search, compact output modes, and doctor checks against an isolated fixture project.
- **Workflow smoke tests now use `ig --status`, not a search query for `status`.** This closes a false-positive E2E gap in CI, release, and cross-platform workflows.
- **Benchmark CI now fails on benchmark execution errors and malformed/empty Criterion JSON.** Critical benchmark regressions remain enforced by the dedicated 15% guard, while dashboard alerts still comment on broader benchmark movement.
- **Release workflow now refuses empty release notes.** Tags without a matching changelog section fail before publishing a weak release.

## [0.6.18] — 2026-05-14

### Performance
- **92.9× faster daemon hot queries on the Linux kernel benchmark.** The daemon now keeps search context and bounded query-result caches warm, avoiding repeated SQLite/Tantivy/vector-store setup on repeated hot queries.
- **Fresh-index vector persistence deferred to final commit.** Bulk ingest avoids rewriting the whole vector file for every batch while preserving bounded in-memory staging.
- **Hot-query status preflight trimmed for static benchmark/no-autospawn runs.** Normal daemon and watcher health handling remains on the full safety path.

### Fixed
- **Daemon cache keys distinguish `--all` from workspace-scoped searches.** Prevents cross-mode query cache reuse when a prior single-workspace query warms the daemon.
- **Corrupt index repair still uses full health verification.** Normal query repair detection opens the stores before deciding to skip rebuilds.
- **Literal and regex searches fall back locally after stale daemon sockets.** `IVYGREP_NO_AUTOSPAWN` plus a stale socket no longer returns empty results for `--literal` or `--regex`.
- **GitHub Actions no longer restores cached Cargo binaries.** CI, release, benchmark, and E2E workflows keep dependency/target caching but avoid poisoned `~/.cargo/bin` restores on macOS ARM.

### Testing
- Added daemon/local equivalence checks for hybrid, literal, regex, filters, scoped queries, and `--all` cache warmup.
- Added stale-socket fallback coverage for literal and regex modes.
- Added Linux hot-query benchmark harness and published charts under `docs/benchmarks/`.

## [0.6.17] — 2026-05-04

### Fixed — Linux Stability
- **OOM: vector store capacity doubling was unbounded.** `usearch` grew capacity by 2× with no cap, causing a single 512 MB allocation on large repos (500K+ chunks). Now caps growth increments to 256K entries (~128 MB max per reallocation).
- **OOM: Tantivy writer heap reduced from 200 MB to 50 MB.** Combined with usearch and SQLite, the old 200 MB heap pushed total working memory past 400 MB on the indexer alone, triggering the OOM killer on 8 GB Linux machines.
- **OOM: SQLite cache reduced from 64 MB to 16 MB.** The WAL cache contributed to the combined memory pressure during bulk indexing.
- **OOM: pre-indexing memory guard on Linux.** The indexer now checks `/proc/meminfo` before starting and refuses to index when `MemAvailable < 512 MiB`, preventing the OOM killer from crashing the entire machine.
- **WAL bloat: SQLite WAL checkpoint after indexing.** Added `PRAGMA wal_checkpoint(TRUNCATE)` after index completion to reclaim WAL disk space immediately.
- **inotify limit detection.** When the recursive watcher fails with `ENOSPC` on Linux (inotify watch limit exhausted), the daemon now logs actionable guidance (`echo fs.inotify.max_user_watches=524288 | sudo tee -a /etc/sysctl.conf`) instead of silently failing.

## [0.6.16] — 2026-05-03

### Performance
- **Daemon-wide hash model cache:** `HashEmbeddingModel` was rebuilt from scratch on every watcher filesystem event, every `Index` request, and every fallback search (when ONNX is still loading). Now cached in a process-wide `static OnceLock` so the alias hash map is constructed exactly once for the daemon's lifetime.
- **Watcher debounce (300ms):** The file watcher previously triggered re-indexing immediately on the first `notify` event. Added a 300ms debounce delay so burst saves (e.g., `cargo fmt` touching 50 files in rapid succession) coalesce into a single indexing pass.

### Fixed
- **README CLI examples:** Removed escaped backslash-quotes (`\"`) in code blocks that rendered incorrectly.

## [0.6.15] — 2026-05-02

### Performance
- **Background neural enhancement no longer pegs the CPU:** The background `--enhance-internal` subprocess was running at full CPU (400%+ on multi-core machines) because the `_is_background` thread-limiting flag was dead code. The rayon global thread pool is now capped to 25% of cores (min 1) when running in background mode, and the subprocess is launched with `nice(10)` so the OS scheduler deprioritizes it below interactive work.
- **Hash embedding uses bounded thread pool:** `HashEmbeddingModel::embed_batch` now uses a cached `OnceLock<ThreadPool>` with half the available cores instead of the unbounded global pool.
- **Search scoring loop eliminates redundant lowercasing:** Introduced `ChunkBoostContext` that precomputes `text_lower`, `path_lower`, `path_segments`, `file_stem`, `first_line`, `text_compact`, and `path_compact` once per candidate. All 7 boost functions and `file_authority_score` now use the precomputed context (~10 redundant string allocations per candidate eliminated).
- **`build_lexical_queries` deduplication:** Was called 3× with the same input per hybrid search (literal pass, lexical pass, path-match pass). Now computed once and shared across all three passes.
- **`HashEmbeddingModel` cached per-process:** The hash model was recreated on every search query. Now cached in a `static OnceLock` so the alias map is built once for the process lifetime.
- **RRF accumulation consolidated:** Three separate `HashMap`s (scores, chunks, sources) consolidated into a single `RrfEntry` struct map, eliminating 6 redundant `chunk_id.clone()` calls per candidate across the accumulation passes.
- **`summarize_reason` hoists lowercase:** `focus.to_ascii_lowercase()` was called once for the contains-check and again per token in the loop — now computed once.
- **`to_hit` avoids cloning file content:** Uses `Cow<str>` so pre-read content is borrowed instead of cloned into a new `String`.

### Testing
- Added 26 new tests: 8 ChunkBoostContext correctness tests, 10 boost function unit tests, 3 embed_batch thread pool consistency tests, 2 E2E hybrid search integration tests, and 3 file_authority_score tests. Total suite: **252 tests**.
## [0.6.14] — 2026-04-29

### Performance
- **Regex matcher hoisted out of hot loop:** `regex_search_parallel` was rebuilding the compiled regex matcher for every file in the `par_iter` loop. The matcher is now built once and shared across all threads, eliminating redundant compilation overhead on every file.

### Tests
- **8 new `filter_meaningful_scores` unit tests:** The adaptive score filtering function — which determines which search results users actually see — now has dedicated unit tests covering single-result passthrough, empty input, uniform distributions, outlier dropping, literal-source bypass, never-empty guarantee, tight cluster preservation, and wide-spread filtering. Total test count: 234.

## [0.6.13] — 2026-04-28

### Changed
- **`ig doctor` → `ig --doctor`:** The `doctor` subcommand is now a `--doctor` flag. This frees the word "doctor" for use as a search query — previously `ig doctor` was silently intercepted before clap parsing, making it impossible to search for the word "doctor". `--fix` is now a standalone flag that requires `--doctor`.

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
- **Stale legacy runtime PID cleanup:** `ig --doctor --fix` now removes dead legacy watcher, indexing, and enhancement PID files instead of only reporting them
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
- **Doctor coverage:** `ig --doctor` now flags stale legacy PID files, stale job heartbeats, long-paused neural enhancement, and watcher queue saturation symptoms
- **Watcher reindex correctness:** daemon-triggered updates now bypass the watcher short-circuit and actually process filesystem mutations
- **Configuration fidelity:** indexing now preserves the workspace’s requested watch mode instead of silently forcing `watch_enabled = true`

## [0.5.44] — 2026-04-11

### Added
- **`ig --doctor` / `ig --doctor --fix`:** new index-health inspection and repair flow for stale, partial, or corrupted local indexes
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
