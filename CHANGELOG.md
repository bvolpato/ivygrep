# Changelog

All notable changes to ivygrep are documented in this file.

## [Unreleased]

### Performance
- Fresh indexes skip the serial dependent-discovery pass, and incremental runs read only `Cargo.toml`/`go.mod`/`pubspec.yaml` for manifest resolution signatures instead of every changed file.

## [1.2.12] - 2026-08-20

### Security
- Upgraded the Hugging Face HTTP dependency chain to eliminate `RUSTSEC-2026-0258` from the inherited `h2` dependency.

### Fixed
- CLI search, task context, and MCP now preserve brace-alternative globs, mixed path filters, character classes, and escaped commas.
- Semantic filtering retains include-glob alternatives and filename-based language overrides without incorrectly narrowing complex patterns.
- Linked Git worktrees complete background neural enhancement and safely share base-index search leases.
- Neural query caches invalidate when model identity changes, watcher failures are retried, regex candidate coverage is complete, and index storage stays excluded from indexed workspaces.

### Performance
- Reuse embeddings when unchanged source chunks shift and keep AST chunk generation bounded.
- Seek directory-scoped and selective glob searches through indexed SQLite ranges instead of scanning candidate paths.
- Retain hot query, neural, readiness, workspace, and search-context entries with bounded LRU caches.
- Adapt Tantivy writer parallelism and memory to changed source size, reducing measured fresh-index latency by 22% at 30,001 chunks and 33% at 100,001 chunks.
- Speed up all-workspace discovery and avoid redundant generated-corpus benchmark compilation.

### Testing
- Required CI now enforces lexical/hash and verified neural retrieval relevance, including nDCG, MRR, candidate recall, and zero-hit thresholds.
- Fresh-index profiling reports individual SQLite, Tantivy, vector-counting, merge, and publication phases.
- CI workflows cancel superseded pull-request runs and distinguish every platform/backend matrix job.

## [1.2.11] - 2026-08-14

### Security
- Updated Tantivy to its patched `lru` dependency, resolving `RUSTSEC-2026-0253` across search and TUI dependency paths.

### Maintenance
- Updated Cargo, Web, and GitHub Actions dependencies.
- Added confirmation runs for noisy million-scale benchmark regressions.

## [1.2.10] - 2026-08-05

### Fixed
- `--skip-gitignore` now reconciles healthy daemon indexes and watcher filters when ignore mode changes.
- Cancelling a queued search now exits workspace-mode waits without delaying daemon cancellation.
- Web and MCP type aliases such as `rs` and `py` now retain semantic search candidates.

### Documentation
- Source installation now creates `~/.local/bin` before copying the release binary.

## [1.2.9] - 2026-08-04

### Maintenance
- Updated release attestations to `actions/attest` v4.2.1 and regenerated evidence dashboard provenance for the pinned workflow.

## [1.2.8] - 2026-08-04

### Fixed
- Multi-workspace search now returns valid hits with explicit warnings when one workspace fails, and returns an error when every selected workspace fails.
- Corrupt, truncated, or oversized compressed chunk text now returns a contextual error instead of being rendered as source.
- Index repair now holds the workspace lock, preserves bare-worktree scope, aborts snapshot publication after ingestion failures, and removes stale vectors and tombstones when an index becomes empty.
- Watchers now reconcile repository exclude changes, retain tracked files under ignored directory names, and surface backend failures.
- Search options now preserve all-index scope from subdirectories, apply type and context filters to regex results, reject overflowing limits, and honor local ignore-policy resets.
- External URL and editor launches now preserve arguments without shell interpretation on every platform.
- Daemon recovery now bounds every connection attempt and handles incompatible responses without waiting for the full recovery timeout.
- Learned reranker previews now truncate long Unicode text only at valid UTF-8 boundaries.
- TUI search cancellation now propagates through daemon queues and active retrieval work, prevents stale result publication, and drains superseded searches without holding the terminal.

### Testing
- Local E2E now covers CLI procedures, daemon equivalence, MCP errors, and a Playwright search-to-file-viewer flow. Neural and cached-model acceptance force neural retrieval and require `neural_executed: true`.
- Release acceptance checks exact installer inputs and exercises CUDA archives through their neural backend or an explicit CPU fallback when no GPU is present.

### Security
- Patched PostCSS and nanoid in the Web lockfile and added weekly npm dependency monitoring.

### Maintenance
- Search execution, fusion, routing, presentation, and workspace aggregation now have focused modules. Index compression, Git state, resource policy, staging, and storage are separated behind the existing indexer API; Web API, rendering, viewer, and UI helpers are split from the application entry point.

### Documentation
- Added a source-oriented architecture guide covering index commit order, storage, retrieval, context packs, worktree overlays, protocols, security boundaries, and module ownership.
- Public benchmark pages now pin latest-measured-release binary provenance, distinguish historical retrieval matrices, and state synthetic-corpus and dataset-license limits. WinGet v1.2.6 is live; v1.2.7 and crates.io publication remain unavailable.

## [1.2.7] - 2026-07-26

### Added
- Notes and memories are now a first-class retrieval use case. Default daemon-backed search blends semantic and lexical retrieval across CLI, MCP, Web, and TUI, then adds two bounded local memory probes when initial results are overwhelmingly note-like.
- Public MemoryQuest evidence covers 535 implicit questions over 3,878 preindexed sessions. Default search retrieves 74.9% of required memories in the top 20. The final-default A/B median was 86.05 ms warm p95; the published v1.2.7 artifact rerun is 87.63 ms.

### Performance
- Incremental indexing now maintains exact distinct-vector counts from up to 512 changed keys, falls back to a full count for larger transactions, streams Merkle root hashing without a corpus-sized buffer, and fetches only missing text blobs when neural enhancement is at least 75% complete. A one-million-entry Merkle A/B fell from 45.01 ms and 128 MB temporary memory to 14.85 ms and constant extra memory; a 90%-complete 200,000-row neural resume improved 33.6%.
- Daemon query caches now invalidate only affected workspaces, survive no-op indexing, and use a 250 ms single-event or 750 ms burst watcher debounce. Measured single-event watcher latency fell from 2.009 s to 251.4 ms.
- Context packs retrieve tests through direct graph ownership and bounded anchor queries, then allow 14 items after required roles are covered. Frozen-task recall rose from `.75000` to `.81944` and test recall from `.000` to `1.000`. A paired three-task screen recovered all eight expected files versus seven with native discovery and reduced aggregate agent input 61.0% (`645,778` to `251,827`); this is one benchmark signal, not a universal token-savings claim.
- Context token estimates are calibrated against current `o200k_base` and `cl100k_base` tokenizers. Thirty packs moved median estimate/actual ratios from about `1.82` to `1.09` with no underestimates.
- Context packs use 34.1% fewer tokens on the frozen 12-task benchmark with unchanged `.7500` recall. A 400-token snippet cap and role-aware 12-item target preserve required relationship roles.
- Automatic neural retrieval now follows lexical confidence and retains complementary hash candidates. On the 1,000-query public-core benchmark, neural executions fell 52.4% while nDCG@10 rose 2.10%, MRR@10 rose 3.21%, and recall@20 rose from `.469` to `.472`.
- Repeated clean Git indexing reuses an exact repository-state marker. A 100,000-file no-op improved from 616 ms to 494 ms and reduced peak RSS by about 56%, with ignore, index, sparse-checkout, and worktree changes forcing full reconciliation.

### Testing
- Context relevance CI runs frozen tasks in clean pre-change worktrees and gates recall, primary recall, test-file recall, zero-recall rate, relationship-role coverage, and recall per 1,000 tokens. Curated fixtures can label relevant unchanged paths against pinned base trees; independent reports record retained and rejected experiments.
- Current-release million-chunk evidence publishes three complete v1.2.7 trials and checks package version plus every reported median.

### Documentation
- README and site lead with code, notes, memory retrieval, and bounded context examples, explicit WinGet and crates.io status, a functional workflow hero, canonical integration pages, and current-release benchmark evidence.

## [1.2.6] - 2026-07-19

### Added
- Codex and Claude Code marketplaces now install one ivygrep plugin containing MCP configuration and a focused task-context skill.
- Release archives now include a cross-platform MCPB package and MCP Registry metadata. GitHub OIDC publishes each tagged release to registry.
- Dedicated integration guides cover Codex, Claude Code, Cursor, Gemini CLI, OpenCode, and generic MCP clients.

### Distribution
- ivygrep and four behavior-critical dependency forks have publishable crates.io manifests without consumer-side `[patch.crates-io]` configuration.
- Windows users gain a portable WinGet package backed by release archive checksum and nested executable validation.

### Testing
- Distribution contracts cover marketplace versions, plugin MCP configuration, crate dependency aliases, cross-platform MCPB contents and digest, WinGet metadata, agent pages, and publication workflows.

## [1.2.5] - 2026-07-18

### Performance
- Exact queries defer neural-store construction until neural retrieval is useful. On a 7,842-chunk hash-indexed workspace, this reduced wall time from `1.06 s` to `0.55 s` and peak RSS from `334,432 KiB` to `28,740 KiB`. Natural-language queries still request neural enhancement, and existing neural stores remain current.
- Fresh indexing uses observed counts and smaller producer batches. Five alternating 1,000,001-chunk runs improved median throughput by `1.62%` and reduced peak RSS by `14.55%` with unchanged file, chunk, vector, and index-size results.

### Fixed
- Neural enhancement requests now survive concurrent hash work. Exact CLI searches keep existing neural stores current after index updates, while natural-language requests arriving during hash-only work start neural enhancement afterward.

### Testing
- Million-scale checks now gate process-cold, CLI warm, daemon warm, cache replay, filtered, and concurrent p95 latency and recall independently.
- The public 100-query pull-request matrix covers hash, hybrid, automatic blended, and forced-neural retrieval.
- The external exact-search harness records ivygrep, ripgrep, and git-grep versions, binary digest, repository commit, match counts, and latency distributions.

## [1.2.4] - 2026-07-18

### Performance
- **Exact daemon queries no longer load the neural model unnecessarily.** Model startup now follows query routing. In an indexed neural workspace, an exact identifier query reduced daemon RSS from `68,596 KiB` to `26,352 KiB` while preserving identical results; a later natural-language query still loaded neural search on demand.

### Testing
- **Pull requests now exercise bounded neural retrieval.** A 100-query public smoke profile gates nDCG, MRR, recall, and no-hit rate alongside deterministic hash and context-pack checks.
- **Million-scale regression detection is tighter.** Statistically significant regressions now fail at `10%` instead of `15%`; a controlled three-run comparison accepted the known-good `0.9241` throughput ratio while a `7.5%` gate produced a false positive.
- **Release retrieval evidence is permanent.** Tag workflows attach public retrieval JSON and HTML directly to each GitHub release after the benchmark matrix completes.
- **Speculative candidates stayed out.** Intent-specific fusion, wider candidate pools, borrowed query-plan fields, bulk persistence variants, score-only context ranking, and a permanent external-tool gate were discarded after neutral, regressive, or non-comparable A/B results.

## [1.2.3] - 2026-07-17

### Performance
- **Exact substring and regex search use compact file-level trigram candidates.** On the 1,000-file benchmark, literal search improved from `0.923 ms` to `0.704 ms` (`23.8%`) and regex search from `9.24 ms` to `1.99 ms` (`4.65x`). A real-repository index grew `5.1%` overall while fresh 30,000-chunk indexing improved from `159.54 ms` to `157.56 ms` (`1.2%`).

### Fixed
- **Literal and regex search no longer miss substrings inside identifiers.** Queries such as `ppl` now find `applyFilter`, regex literal extraction ignores optional and alternative branches, indexed regex respects gitignore state, and overlapping structural chunks produce one literal hit per source line.
- **Failed fresh rebuilds preserve the last healthy index.** Staged artifacts are validated before promotion, live stores move to a rollback directory, and partial promotion failures restore every previous artifact without replacing the active lock inode.
- **Watcher job generations cannot overwrite newer state.** Heartbeats and completion updates carry the watcher nonce, so detached work from a stopped watcher cannot revive or modify a replacement watcher record.
- **Invalid vector dimensions cannot delete existing embeddings.** Optimized vector upserts validate dimensions before removing an existing key, and search and score APIs now match portable-backend rejection behavior.

### Testing
- **Correctness and performance candidates were independently gated.** Focused regressions cover index preservation, watcher generations, vector replacement, substring recall, optional regex groups, gitignore filtering, and literal deduplication. Wildcard-bigram candidates (`82x` slower), chunk-level trigrams (`42%` larger index), and raw scans (`3.3x` slower) were discarded.

## [1.2.2] - 2026-07-17

### Performance
- **Broad filtered semantic search uses an adaptive ANN budget.** Three alternating 100,000-chunk trials improved p50 latency from `11.93 ms` to `9.65 ms` (`19.1%`) and p95 from `13.90 ms` to `11.50 ms` (`17.3%`) with identical recall and MRR. Underfilled filtered results retain a bounded fallback.

### Fixed
- **Forced local rebuilds preserve watcher intent.** `ig --add PATH --force` rewrites workspace metadata after removing an index when no daemon handles the request, so later daemon startup resumes file watching.
- **Context relationships retain correct source ranges.** Synthetic path headers are removed before focusing graph definitions, callers, and references, preventing useful relationship evidence from being discarded as overlapping.
- **Hash readiness is visible.** Status, doctor, and JSON workspace output report hash-vector count and coverage alongside neural coverage.

### Testing
- **Performance evidence covers retrieval, readiness, and daemon stability.** Release tags run three public-retrieval repetitions; relevance checks upload historical context-pack evidence; million-scale scheduled runs include concurrent query and watcher-mutation soak results.
- **Rejected experiments remain out.** Global and low-confidence candidate widening, higher HNSW connectivity, lower search expansion, automatic vector compaction, short-token boundary changes, and always-on neural search lost quality or cost too much latency, memory, build time, or storage.

## [1.2.1] - 2026-07-16

### Fixed
- **Windows installation retries transient GitHub failures.** Release metadata, archive, and checksum requests use bounded exponential backoff instead of failing on a single service outage.

### Performance
- **Filtered search avoids redundant candidate work.** One shared semantic filter plan serves hash and neural vectors, broad indexed filters use a bounded SQLite preflight, exact and directory include globs use indexed path ranges, and repeated glob path filters use a bounded request cache. On a deterministic 100,001-chunk profile, broad filtering improved `35.40 ms` to `9.48 ms` (`3.73x`), exact paths `61.52 ms` to `1.24 ms` (`49.62x`), missing prefixes `62.99 ms` to `0.325 ms` (`194.08x`), and filtered literal search `6.71 ms` to `2.54 ms` (`2.64x`). Unfiltered search also improved `1.217 ms` to `1.085 ms`.
- **Incremental indexing updates cached chunk and file statistics transactionally.** A controlled one-file update reduced finalize latency from `266.97 ms` to `225.48 ms` (`15.5%`) while preserving exact cached counts across additions, replacements, deletions, and empty files.

### Testing
- **Search output and relevance remain unchanged.** Two hundred fifty baseline-candidate comparisons produced identical hit counts and fingerprints. Deterministic relevance held at `0.949275` MRR, `0.913043` precision@1, `0.978261` recall@5, and `0.920201` nDCG.
- **Rejected experiments remain out.** LIKE prefix filtering, per-vector statistics probes, raw vector deltas, structured neural metadata, and wider rerank limits were slower, neutral, or reduced relevance.

## [1.2.0] - 2026-07-16

### Added
- **Context packs understand active work.** `ig context --since main` combines merge-base commits, staged edits, unstaged edits, untracked files, and exact file-line references from issues or stack traces.
- **CLI, MCP, and Web share one structured pack.** MCP accepts `since`; Web adds Context pack mode, Git base, multiline task input, relationship roles, and explanations.
- **JVM and .NET test ownership is directional and incremental.** Java, Kotlin, Scala, Groovy, and C# source-test conventions work forward, reverse, and when either side appears later.
- **Context Graph v2 connects task evidence across files.** Indexing stores compact typed dependency, test, configuration, and documentation edges. `ig context` expands both directions, ranks bounded graph evidence, adds recent Git co-changes, and reports dependency and dependent coverage.
- **MCP agents can request complete context packs through `ig_search`.** `output=context_pack` and `budget_tokens` expose same scoped, filtered, token-bounded pack as CLI without adding another tool or round trip.

### Performance
- **Broad filtered semantic searches stop after proving exact scoring would exceed its 50,000-chunk bound.** A deterministic 60,001-chunk profile improved from `77.918 ms` to `69.777 ms` (`10.4%`) with byte-identical output, `12.1%` fewer instructions, and neutral narrow-filter latency.
- **Completed vector enhancement avoids reading and decompressing every stored chunk.** Exact key validation plus no-op neural persistence reduced a 60,000-chunk no-op pass from `104.951 ms` to `48.747 ms` (`53.6%`) without slowing initial or partial enhancement.
- **Skip-gitignore Merkle scans share their read-only path set across workers.** A 50,000-file scan improved from `112.38 ms` to `81.29 ms` (`27.7%`) with identical file and root hashes.
- **Index heartbeat workers stop and join when each index operation finishes.** Two hundred rapid no-op indexes improved from `823.07 ms` to `776.60 ms` (`5.6%`) while final process threads fell from `218` to `17`.

### Community
- **Contributor paths are complete.** Structured issue forms, a pull-request evidence template, stable toolchain defaults, community policies, support routing, and a task-oriented contributor guide reduce setup and review ambiguity.
- **Website and README expose contribution entry points.** Newcomers can find starter work, Discussions, issue forms, security reporting, and validation guidance without searching repository internals.

### Documentation
- **Launch surface leads with outcome.** README now shows task to context pack to agent to passing test in roughly 200 lines. Website and dedicated 1280x640 social card use same message.
- **Project documentation is smaller.** Contributor, integration, architecture, governance, support, and benchmark essentials now live in focused README, CONTRIBUTING, website, HTML reports, and machine-readable evidence instead of overlapping Markdown files.

### Testing
- **Diff packs have cross-surface E2E coverage.** Tests cover stale indexes, dirty files, stdin traces, token budgets, gitignore, Web payloads, MCP schema, and late JVM/.NET additions.
- **Context graph behavior has unit, incremental-index, CLI, MCP schema, direct-tool, and stdio session coverage.** Tests verify relationship extraction, reverse edges, stale-edge replacement, strict output shape, and budget enforcement.
- **Community-health contracts prevent onboarding drift.** Tests verify required files, issue-form routing, policy links, contributor commands, and release-version synchronization.
- **Performance candidates remain evidence-gated.** Wider reranker routing, retrained reranker weights, partial-selection fusion, larger Tantivy and indexing buffers, single-transaction persistence, universal enhancer scans, and read-only enhancement connections were discarded after relevance loss, slower cold paths, higher memory, or neutral results.

## [1.1.19] - 2026-07-13

### Improved
- **`ig context` expands and explains more task evidence.** Packs now add exact references, separate configuration and documentation roles, merge retrieval signals, reject acronym and filename false anchors, prefer production relationships over test-helper names, and report role coverage.
- **Context budgets cover rendered Markdown.** `used_tokens` now estimates complete pack output, including headings, reasons, signals, and code fences, with final trimming when needed.

### Documentation
- **Context packs are a first-class product workflow.** README, agent guide, CLI help, metadata, comparison table, and dedicated website section explain one-command task handoffs and useful budget sizes.

### Fixed
- **Cross-platform release checks tolerate transient model-host failures.** Xet CAS signed-URL failures retry without treating generic authorization errors as transient, and daemon-equivalence output is decoded as UTF-8 on Windows.
- **Neural acceptance uses Hugging Face's native Xet client.** CI and release checks cache pinned, checksum-verified model assets before testing online and offline loading.

## [1.1.18] - 2026-07-12

### Added
- **`ig hardware` selects and explains best supported build.** Diagnostics report CPU threads, NVIDIA model and compute capability, active model profile, installed backend, recommended backend, missing runtime libraries, compatibility limits, and exact reinstall command in text or JSON.

### Fixed
- **Installers avoid unusable accelerator builds.** Unix auto-selection requires complete CUDA 13 libraries and compute capability 8.0+, matching shipped `sm_80` kernels. Explicit CUDA requests fail with exact remediation, Apple Silicon Homebrew installs Metal build, and every installer states selected archive.

### Testing
- **Hardware decisions have platform and installer regression coverage.** Tests cover Linux CUDA success, incomplete runtime, unsupported NVIDIA generations, no-GPU fallback, Apple Metal, Windows portable selection, Homebrew formula output, CLI text/JSON, and diagnostics without writable application storage.

## [1.1.17] - 2026-07-12

### Fixed
- **Help works without writable application storage.** CLI parsing now handles `--help` before creating index directories, including cross-architecture and restricted-home environments.

## [1.1.16] - 2026-07-12

### Added
- **`ig context` builds task-aware context bundles.** One command gathers implementations, definitions, callers, tests, and supporting evidence within a token budget.
- **`ig agent install` configures Claude Code, Codex, and Cursor.** Existing settings are preserved, then `ig agent doctor` verifies MCP initialization, tool discovery, and one real search.

## [1.1.15] - 2026-07-11

### Added
- **MCP search supports symbol definitions, references, and callers.** `ig_search` now matches CLI symbol-navigation modes while preserving scope, type, and glob filters.
- **CoREB can be exported as a pinned external relevance suite.** The exporter covers text-to-code, code-to-code, and code-to-text retrieval, verifies source hashes, and excludes labeled hard negatives from positive judgments.

### Performance
- **Lightweight neural indexing uses larger embedding batches.** StaticEmbedding and Model2Vec enhancement improved a shared 100,000-chunk median from `7.96 s` to `7.42 s` (`6.8%`) with unchanged memory and byte-identical vector stores. Transformer CPU batching remains unchanged.

### Fixed
- **The canonical PotionCode profile name selects the requested model.** `potion-code-16m-v1` no longer falls back to the default profile.

### Testing
- **Search relevance remains unchanged.** The final 1,000-query public-core guard held nDCG@10, MRR@10, recall@20, and warm p95 within measurement noise; query-truncation candidates that improved latency but reduced held-out relevance were discarded.

## [1.1.14] - 2026-07-11

### Performance
- **General search uses multi-core hosts more effectively.** Independent lexical work runs concurrently while filtering, deduplication, and ranking remain deterministic. Single-threaded hosts keep the sequential path.
- **Repeated neural queries reuse bounded query vectors.** Changing limits, context, or filters no longer repeats identical model inference when compatible neural vectors are available.

### Testing
- **Search output and relevance remain covered by release gates.** Multi-platform tests verify hash-only, neural, daemon, CLI, and MCP paths.

## [1.1.13] - 2026-07-11

### Performance
- **Broad filtered semantic scoring is 4.45x to 7.09x faster.** Runtime-dispatched AVX2/FMA scores eight dimensions per instruction, and filters with at least 5,000 candidates use parallel local top-K heaps followed by an exact deterministic merge. On a Ryzen 9 3950X, Criterion time point estimates for 5K, 25K, and 50K candidates fell from `1.525 ms`, `7.904 ms`, and `15.808 ms` to `0.343 ms`, `1.222 ms`, and `2.230 ms`. The serial 500-candidate path also improved from `164.3 µs` to `91.9 µs` (`1.79x`). Unsupported CPUs retain the scalar path.

### Testing
- **Vector arithmetic and parallel selection have focused regression coverage.** Tests compare SIMD scores with scalar scores, exercise non-multiple-of-eight dimensions, verify missing-key handling, and force a two-thread local/global top-K merge with tied scores.
- **Rejected prototypes stay rejected.** Full-store key validation improved only `4.6%`; native hash-filter traversal made 500-candidate search `3.07x` slower; a native bitmap filter peaked at `17%`. Indexing and complex lexical-search profiles remained distributed, with no isolated hotspot supporting a credible `2x` claim.

## [1.1.12] - 2026-07-11

### Relevance
- **Natural-language subject stems receive a bounded primary-source lift.** Partial derivational matches such as `walk` to `walker.rs` and `job` to `jobs.rs` now break close fused-score ties, while exact generic stems, short queries, tests, docs, and generated files receive no multiplier. On the deterministic 23-query fixture, precision@1 improved from `0.913` to `1.000`, MRR from `0.957` to `1.000`, and nDCG@10 from `0.933` to `0.965`; recall@5 held at `0.957` and candidate recall held at `1.000`.

### Testing
- **Relevance CI now protects the measured baseline.** Minimum precision@1 rises from `0.38` to `1.00`, MRR from `0.52` to `1.00`, and recall@5 from `0.70` to `0.95`.
- **Profile-driven experiments remain evidence-gated.** Native key-addressed exact-vector scoring was discarded after slowing the 50K-vector benchmark from `15.912 ms` to `2.789 s`; profiles showed no indexing or complex-search hotspot capable of a credible isolated `2x` win.

## [1.1.11] - 2026-07-10

### Performance
- **Fresh indexing sizes Tantivy document buffers from their content.** A three-run 30K-chunk profile reduced median peak RSS from `94,780 KiB` to `74,756 KiB` (`21.1%`) and minor faults from `22,735` to `17,433` (`23.3%`) with neutral wall time.
- **Filtered exact vector scoring performs one lookup and one arithmetic pass per key.** Removing the redundant existence lookup and fusing dot-product/norm accumulation improved the 50K-vector top-50 benchmark from `24.128 ms` to `15.548 ms` (`35.6%`) without changing score bits or tie ordering.
- **Three-term disjunctive lexical queries use a flat boosted clause list.** Removing one Tantivy union layer improved the 1K-file complex phrase benchmark from `3.394 ms` to `3.179 ms` (`6.3%`); other query shapes keep the nested layout after adjacent rerank benchmarks rejected broad flattening.

### Testing
- **Filtered vector benchmarks cover 1%, 10%, 50%, and 100% candidate subsets.** CI now guards the full exact-filter path alongside fresh indexing, incremental indexing, hybrid search, and hot ANN search.

## [1.1.10] - 2026-07-10

### Added
- **MCP tools expose strict, version-negotiated contracts.** Clients can negotiate four protocol versions through `2025-11-25`; tool definitions now include titles, output schemas, side-effect annotations, bounded inputs, structured results, and standard JSON-RPC errors.
- **Web search supports authenticated non-loopback access.** Each daemon creates a process-local token, exchanges it for an HttpOnly same-site cookie, and accepts equivalent bearer authentication for API clients.

### Performance
- **Filtered exact vector search batches scoring and keeps only top candidates.** Query normalization and retrieval buffers are reused once per store, while a bounded heap replaces full-result sorting. The 50,000-vector top-50 benchmark improved from `34.960 ms` to `23.685 ms` (`1.48x`).
- **Incremental deletion journals sync once per transaction.** A 32-file reindex reduced expected hash/neural durability syncs from 64 per-file calls to 2 traced `fdatasync` calls while preserving pre-commit crash ordering.

### Fixed
- **`--add --wait-for-enhancement` waits for requested vector work.** Hash and neural workflows now start the correct background worker, surface launch or stall failures, and verify durable completion before returning.
- **CLI diagnostics honor explicit paths and terminal color settings.** `--doctor PATH` uses the requested workspace, and status output respects `NO_COLOR`.
- **Web UI rejects stale asynchronous updates.** Superseded search streams, file loads, and tree loads can no longer overwrite current state; partial workspace failures remain visible.

### Security
- **Web APIs validate request origin and bound connection work.** Host and same-origin checks protect every API route, file opening is POST-only, security headers cover embedded content, and request handling is limited to 128 connections with a five-second header deadline.

### Documentation
- **CLI, MCP, and web guidance now matches runtime behavior.** Help text documents every argument and option, runnable examples cover common workflows, and non-loopback web instructions explain token handling and plain-HTTP limits.

### Testing
- **Performance and relevance changes have paired evidence.** Filtered vector scoring improved by `32.3%`; a repeated 100-query public retrieval matrix showed no proven aggregate relevance gain, so broader semantic-only results remain guarded by score and authority thresholds.
- **New regression coverage exercises durability, protocol contracts, authenticated web access, asynchronous UI ordering, and enhancement waiting.** Full default-feature validation passed 624 Rust tests, 112 Python tests, and 3 frontend tests.

## [1.1.9] - 2026-07-09

### Added
- **Shell installs can select accelerator archives.** Linux x86_64 releases now include a CUDA build, Apple Silicon releases include a Metal build, and `install.sh` selects a compatible accelerator archive while retaining portable fallback behavior.

### Testing
- **Performance gates now use paired base/head measurements for indexing, search, and hot vector lookup.** CI publishes machine-readable guard results and keeps the cross-run shared-runner chart informational.
- **Release acceptance now covers seven archives.** Portable Linux, macOS, and Windows builds retain existing smoke coverage; accelerator builds add CUDA linkage checks and Metal backend execution.
- **CUDA release builders install NVRTC development libraries.** Tagged Linux CUDA builds now have every link-time CUDA dependency on clean GitHub runners.
- **Retrieval reports now distinguish production blended routing from forced neural retrieval.** The 600-query challenge matrix ran three times under active LLM load; blended quality held nDCG@10 `0.5955`, MRR@10 `0.5652`, and recall@20 `0.7000`.
- **Recall experiments were rejected after broader checks.** Deeper candidate pools lost `7.9%` challenge nDCG; broad and source-only hash retention also reduced challenge quality or codetrans recall. Search routing remains unchanged.

## [1.1.8] - 2026-07-07

### Relevance
- **Learned reranking now uses a deeper candidate set for natural-language queries.** Exact identifier, path, and literal/error lookups keep the old narrow rerank pool, while mixed, docs, and natural-language routes expose more candidates to the learned reranker. Deterministic reranking and `IVYGREP_RERANK_LIMIT` overrides keep their previous behavior.

### Testing
- **A/B kept only the adaptive rerank-depth winner.** On the 600-query SOTA challenge profile across 3 repetitions, nDCG@10 improved from `0.5950` to `0.5963`, MRR@10 from `0.5654` to `0.5661`, precision@5 from `0.1300` to `0.1303`, and recall@20 from `0.6967` to `0.7000`, with no task-level relevance regression. Raw rerank depths 50, 100, and 200, wider result limits, PotionCode defaults, and long-query decomposition were discarded after neutral, slower, or regressive results.

## [1.1.7] - 2026-07-06

### Fixed
- **Large UTF-8 source files no longer get skipped when the sniff sample ends mid-character.** The text gate now accepts valid UTF-8 prefixes with an incomplete trailing sequence, so large source files such as `src/chunking.rs` stay indexable and persist chunks.

### Relevance
- **Natural-language ranking now promotes exact alias-derived file stems.** Generic aliases cover code-search phrasing such as IPC channels, vector stores, protocol messages, concurrency bounds, generated-file filters, tokenization, and indexing pipelines. Exact alias stem matches receive a bounded rank lift, while generic process aliases such as daemon/service/worker do not crowd out more specific subsystem files.

### Testing
- **Relevance fixture now has no low-rank misses.** Foreground relevance improved from precision@1 `0.391`, MRR `0.578`, recall@5 `0.783`, nDCG@10 `0.620`, and candidate recall `0.783` to precision@1 `1.000`, MRR `1.000`, recall@5 `0.978`, nDCG@10 `0.973`, and candidate recall `1.000`.
- **A/B discarded neutral or regressive candidates.** Path-recall alias expansion regressed MRR/nDCG slightly, and compound file-stem boosting produced no metric gain, so both were left out.

## [1.1.6] - 2026-07-06

### Performance
- **Natural-language search skips impossible exact-symbol lookups.** Whitespace prose queries no longer probe the symbol table for a literal full-query symbol name, while single-symbol and qualified-symbol searches keep exact definition lookup. On the million-chunk corpus, warm p95 improved from `0.578 ms` to `0.500 ms`, warm median from `0.405 ms` to `0.377 ms`, and filtered p95 from `0.520 ms` to `0.430 ms`.

### Testing
- **A/B kept only the measured search-planning win.** ASCII tokenizer scanning, larger Tantivy writer heaps, daemon signature TTL caching, and unsafe no-context indexed previews were discarded after neutral, regressive, or correctness-risk results.
- **Relevance stayed unchanged.** Public million-chunk recall@20 and MRR@20 stayed at `1.0`; fixture MRR, nDCG@10, precision@1, recall@5, and no-hit count were identical in paired baseline/current runs.

## [1.1.5] - 2026-07-05

### Performance
- **Incremental reindex removes stale symbol rows without scanning the full symbol table.** File graph cleanup now recomputes symbols for changed chunks and deletes exact `(normalized_name, chunk_key)` keys. On the million-chunk corpus, one-file reindex median improved from `230.81 ms` to `169.12 ms`, and p95 improved from `242.46 ms` to `182.74 ms`.
- **Merkle diffs avoid cloned path sets.** Snapshot diff now merges ordered maps directly, cutting allocation work while preserving added, modified, deleted, and ignored-file decisions.

### Testing
- **A/B kept only measured wins.** A secondary `symbols(chunk_key)` SQLite index improved incremental cleanup but slowed fresh indexing by `5.7%`, and an ASCII tokenizer scanner was within noise with worse tails, so both were discarded.
- **Fresh indexing and search quality stayed clean.** The stacked winner improved fresh million-chunk indexing from `5234.98 ms` to `5056.88 ms`; search recall@20 and MRR@20 stayed at `1.0`, with warm p95 `0.471 ms`.

## [1.1.4] - 2026-07-05

### Performance
- **Incremental indexing avoids full status scans before nested-workspace checks.** The guard now reads workspace roots from `workspace.json` metadata instead of building full status records with SQLite, dbstat, vector, and compaction checks. On the million-chunk generated corpus, one-file reindex wall time improved from `852 ms` median / `865 ms` p90 to `259 ms` median / `263 ms` p90, while still indexing exactly one changed file.

### Testing
- **A/B kept only the measured winner.** Larger fresh-index Tantivy writer memory regressed fresh indexing from `5.20 s` to `7.86 s`, a tokenizer lowercase shortcut was neutral/slower, and a daemon search micro-hoist was noisy, so all three were discarded.
- **Search quality and latency stayed clean.** The final 2000-sample million-chunk run kept recall@20 and MRR@20 at `1.0`, with warm p95 `0.499 ms`, cache p95 `0.125 ms`, and filtered p95 `0.548 ms`.

## [1.1.3] - 2026-07-05

### Performance
- **Daemon hot searches cache workspace readiness after safe artifact checks.** Repeated daemon searches now skip redundant queryability and overlay freshness work when metadata, index format, SQLite/Tantivy/vector stamps, Merkle snapshots, PID files, and worktree base references are unchanged.

### Testing
- **Million-chunk A/B kept the measured winner.** Warm distinct daemon p95 improved from `6.377 ms` to `0.955 ms`, cache replay p95 from `5.569 ms` to `0.153 ms`, filtered p95 from `6.281 ms` to `0.491 ms`, and concurrent p95 from `10.430 ms` to `0.717 ms`; recall@20 and MRR@20 stayed at `1.0`.
- **Discarded neutral or slower candidates.** Top-level regex alternation prefiltering preserved hits but slowed local regex median from `179.6 ms` to `183.3 ms`, and extra vector/context reuse work was left out because daemon hot vector search already stays around `9.6 us`.

## [1.1.2] - 2026-07-05

### Performance
- **Daemon-backed CLI searches skip unnecessary status preflights.** Already-indexed, no-watch/static query paths now route straight to the daemon when its socket exists, avoiding a redundant runtime-status round trip before hot searches.
- **Single-literal ASCII verification avoids regex setup.** Exact literal searches use a small case-insensitive byte matcher for the common single-query ASCII path, while multi-query and specificity-ranked searches keep the regex path.

### Testing
- **Performance and relevance stayed green in A/B checks.** Literal search on the 1k fixture improved from `1.221 ms` to `0.931 ms`; 100k warm distinct CLI median improved from `9.449 ms` to `7.505 ms`; relevance stayed unchanged at MRR `0.5848`, nDCG@10 `0.6219`, precision@1 `0.3913`, and recall@5 `0.7826`.
- **Discarded noisy candidates before release.** Larger symbol insert batches, fresh-index cached stats, tokenizer scan rewrites, and multi-variant ASCII literal matching were tested independently and left out after neutral or regressive results.

## [1.1.1] - 2026-07-04

### Fixed
- **Daemon-backed web searches recover unhealthy workspace indexes before failing.** Selected-workspace searches rebuild broken metadata/index stores on demand, and all-index searches skip unrecoverable workspaces without failing the whole result stream.
- **Web UI no longer keeps recent searches.** The sidebar now focuses on workspaces, scope, explorer, results, pinned copies, and file browsing without persisting query history in local storage.

### Documentation
- **README and Pages keep `ig --web` visible in normal workflow docs.** The local web UI remains documented alongside daemon and MCP commands without adding another launch-heavy section.

## [1.1.0] - 2026-07-04

### Added
- **Web daemon mode (`ig --web`) opens local browser search.** The daemon can now bind a local HTTP UI on `127.0.0.1:4747` by default, with `--host`, `--port`, optional initial query, workspace focus, all-index search, status, result browsing, and tracked-file viewing.
- **Browser UI now includes code navigation affordances.** Result snippets and opened files use best-effort syntax highlighting, result snippets show line markers, Markdown files open in preview mode with a source toggle, selected results are visibly highlighted, known languages get compact file icons, and sidebar/results/file panes scroll independently.

### Testing
- Added web-server integration coverage for status, search, and safe tracked-file reads.
- Added pnpm frontend checks so embedded web assets stay in sync with source.

## [1.0.7] - 2026-07-03

### Performance
- **Simple lexical searches bypass QueryParser construction.** Identifier-like BM25 queries now build Tantivy term queries directly across code text, tokenized path, and signature fields, while paths, quoted syntax, and boolean parser operators still use the existing parser path. The repeated 2x160-query A/B run improved warm median latency from `0.399 ms` to `0.363 ms`, warm p95 from `0.497 ms` to `0.464 ms`, lexical phase median from `0.255 ms` to `0.228 ms`, and total phase median from `0.370 ms` to `0.341 ms`.

### Testing
- **Relevance stayed unchanged in generated-scale validation.** Recall@20 and MRR@20 stayed at `1.0` across the A/B runs and final 1M-chunk benchmark.
- **Index-side candidates were discarded.** Tokenizer ASCII scanning, symbol-row normalization, presentation allocation changes, and the stacked tokenizer/search candidate were tested independently and rejected after noisy or regressive indexing/search results.

## [1.0.6] - 2026-07-03

### Performance
- **Exact lexical routes skip unnecessary neural vector-store opens.** Default exact identifier, path, and literal/error searches now avoid loading neural vectors when routing will not execute neural retrieval, while forced neural and scoped/glob/type-filtered searches keep neural vectors available for provenance and filtered semantic search. The clean A/B run improved `hybrid_simple_symbol_1000_files` from `2.7606 ms` to `1.7812 ms`; `hybrid_complex_phrase_1000_files` stayed slightly faster at `2.5267 ms`, and bounded rerank stayed neutral at `3.0164 ms` versus `3.0122 ms`.

### Testing
- **Independent A/B trials kept only the measured winner.** Fusion map pre-sizing, single-symbol SQL fast paths, tokenizer buffer reserve changes, hash-vector expansion tuning, and F32 hash vectors were discarded after benchmark regressions, noisy wins, or artifact compatibility costs.
- **Relevance stayed unchanged.** Hash-enriched relevance held MRR `0.6087`, nDCG@10 `0.6498`, precision@1 `0.4348`, and recall@5 `0.7826`.

## [1.0.5] - 2026-07-03

### Performance
- **Hash-vector indexing uses a smaller HNSW graph for the provisional recall tier.** The hash store now builds with lower connectivity and add expansion, cutting `ingest_5k_hash_vectors` from `338.64 ms` to `85.88 ms` and hot 50k-vector search from `35.97 us` to `9.37 us` while public hash/hybrid retrieval fixture quality stayed at `1.0` nDCG@10, MRR@10, and recall@20.
- **Dense Rust function files skip unnecessary Tree-sitter parsing.** Large Rust files made only of many simple function definitions use the existing signature splitter, while mixed Rust files still use AST chunking. The 30k-chunk fresh-index benchmark improved from `170.97 ms` to `154.53 ms`.

### Testing
- **Independent A/B trials kept only the measured winners.** Semantic-source bitsets, file-coverage bitsets, tokenizer lowercase rewrites, threaded USearch reserve, `expansion_add=4`, and lexical pruning were discarded after benchmark or relevance regressions.
- **Release gates passed before tagging.** Validation covered `./test.sh`, full Rust tests, Clippy, Criterion hot-path benchmarks, hash-enriched relevance, and public retrieval fixture checks.

## [1.0.4] - 2026-07-03

### Performance
- **Symbol definition lookup batches candidate names into one ranked SQLite query.** Search now avoids repeated symbol-definition SELECT execution in hot hybrid queries while preserving per-name ranking, exact-case preference, canonical-file preference, and chunk deduplication.

### Testing
- **Independent A/B trials kept only the symbol-query winner.** The 500k generated-corpus confirm run improved warm daemon p95 from `0.689 ms` to `0.472 ms`, filtered p95 from `0.644 ms` to `0.461 ms`, lexical phase p95 from `0.650 ms` to `0.429 ms`, and concurrent QPS from `1855` to `2343`; recall@20 and MRR@20 stayed at `1.0`. Direct query compilation, tokenizer changes, and larger symbol insert batches were discarded.

## [1.0.3] - 2026-07-02

### Performance
- **Fresh indexing prebuilds Tantivy documents on worker threads.** Chunk compression, signature extraction, and Tantivy document construction now happen before the single writer loop persists batches, moving more indexing work onto the parallel producer stage.
- **ASCII code tokenization avoids extra lowercase work on common identifiers.** Lowercase and numeric tokens take a fast path while preserving existing camelCase, snake_case, path, and offset behavior.

### Testing
- **Independent A/B trials kept only the winning indexing optimizations.** The 500k generated-corpus run improved fresh-index throughput from `157k` to `202k` chunks/s while preserving recall@20 and MRR@20 at `1.0`; paired daemon query comparison found no significant search regression.

## [1.0.2] - 2026-07-02

### Performance
- **Query normalization no longer initializes the regex-backed pluralizer.** Common plural handling now uses a small deterministic singularizer, preserving recall while reducing cold search startup cost and improving daemon hot-query tails.

### Security
- **Syntax highlighting no longer pulls the vulnerable plist/XML loader path.** Syntect now uses its built-in syntax and theme dumps without the `quick-xml` transitive dependency flagged by RustSec.

### Testing
- **Paired 500k-chunk A/B validation kept relevance unchanged.** Recall@20 and MRR@20 stayed at `1.0`; fresh indexing stayed neutral, warm daemon p95 improved from `0.658 ms` to `0.600 ms`, and cold process p95 improved from `22.253 ms` to `13.768 ms`.

## [1.0.1] - 2026-07-02

### Fixed
- **Apple Metal builds now enable Candle NN's Metal backend.** The vendored embedding crate forwards its `metal` feature to both `candle-core` and `candle-nn`, so macOS release/source builds resolve the Metal kernel stack consistently.

## [1.0.0] - 2026-07-01

### Changed
- **SemVer-major release for index format v17.** Existing indexes rebuild once so the main Tantivy code-body field can drop positional postings. The rebuild is intentional cache invalidation; source data and CLI behavior are unchanged.

### Performance
- **Text postings stay frequency-only on the main code body field.** Paired Criterion runs improved `hybrid_complex_phrase_1000_files` from `4.45 ms` to `2.79 ms`, `hybrid_search_200_files` from `3.66 ms` to `3.50 ms`, `literal_search_200_files` from `2.41 ms` to `2.05 ms`, and `hybrid_simple_symbol_1000_files` from `3.19 ms` to `2.96 ms`.

### Testing
- **Release gates stayed green after the version promotion.** Local checks covered formatting, Clippy, lib and integration tests, release smoke, daemon equivalence, and relevance thresholds before tagging.

## [0.12.21] - 2026-07-01

### Performance
- **Main Tantivy code-body postings no longer store positions.** The main `text` field now keeps term frequencies without positional postings, cutting position-decoding work in lexical search while exact user-facing matches still go through literal verification. Paired Criterion runs improved `hybrid_complex_phrase_1000_files` from `4.45 ms` to `2.79 ms`, `hybrid_search_200_files` from `3.66 ms` to `3.50 ms`, `literal_search_200_files` from `2.41 ms` to `2.05 ms`, and `hybrid_simple_symbol_1000_files` from `3.19 ms` to `2.96 ms`; fresh and incremental indexing stayed neutral.

### Testing
- **Relevance and discarded candidates were checked before release.** The foreground relevance gate stayed at `MRR 0.609`, `precision@1 0.435`, and `recall@5 0.783`. Fast-field stored-doc replacement, a direct flat vector backend, and a generic search-context cache were tested independently and discarded.

## [0.12.20] - 2026-07-01

### Performance
- **Large generated Rust indexes skip unnecessary Tree-sitter parsing.** Files with generated headers and many simple Rust item signatures now use the fallback splitter while preserving each item's leading documentation. The million-chunk generated profile improved fresh index time from `6505.5 ms` to `6354.9 ms`, warm p95 from `0.655 ms` to `0.632 ms`, and kept recall@20 and MRR@20 at `1.0`.
- **Search hydration reuses SQLite prepared statements.** Repeated semantic candidate hydration now uses cached statements for batch metadata and text fetches, reducing query preparation overhead in hot search paths.

### Fixed
- **Objective-C declarations no longer fold into following method chunks.** `@interface`, `@implementation`, and `@protocol` remain structural chunks instead of being treated as generic decorator lines.

## [0.12.19] - 2026-06-30

### Fixed
- **Stress fixture bootstrap no longer fails when Project Gutenberg is unreachable.** Downloaded public-domain corpora remain preferred, but the script now writes small deterministic fallback fixtures that preserve the ignored stress-test queries when Gutenberg times out from CI runners.

## [0.12.18] - 2026-06-30

### Performance
- **Fresh indexing now uses a bounded foreground parser pool.** Indexing defaults to physical cores and exposes `IVYGREP_INDEX_THREADS` for explicit tuning. The 505k-chunk profile improved median fresh-index wall time from `3.46s` to `3.33s`, CPU time from `28.46s` to `24.05s`, and peak RSS from `251MB` to `239MB`.
- **Code tokenization reuses buffers instead of allocating segment strings.** Identifier splits now carry source byte ranges and normalize into a reused Tantivy token buffer. Heaptrack allocation samples dropped indexing allocations from `12.93M` to `9.08M`.
- **Repeated query tokenization is cached per search thread.** Hot search loops reuse bounded tokenized query forms, improving median latency from `0.556ms` to `0.533ms` and p95 from `0.704ms` to `0.680ms` while preserving result digests in A/B tests.

### Testing
- **Discarded slower batched symbol-definition SQL.** It preserved output digests but reduced search throughput from about `1700` QPS to `1640` QPS, so it was reverted before release.
- **Stress validation covers the old crash path.** `./test.sh --stress` passed all 11 downloaded-fixture stress tests, including concurrent search during reindex, query storms, regex stress, and ripgrep/tantivy scale fixtures.

## [0.12.17] - 2026-06-30

### Performance
- **Code tokenization now streams terms instead of materializing every token.** The code tokenizer emits normalized segments on demand, removing a hot allocation path during indexing. Paired million-chunk validation improved fresh-index average from `7.238s` to `7.072s`, reduced indexing CPU from `68.0s` to `61.8s`, and cut concurrent search p95 from `9.00ms` to `6.69ms` while preserving recall@20 and MRR@20 at `1.0`.
- **Small stored chunks avoid premature compression.** Tiny chunk text stays inline until `512` bytes, reducing compression overhead during indexing. Validation showed index size moving from `435MB` to `467MB`, a deliberate tradeoff for lower CPU and better tails.

### Documentation
- **README and Pages are tighter and current.** The comparison table now includes Zoekt, release/site copy matches the 24 Tree-sitter AST language set and `static-retrieval-v1` embedding model, and the GitHub Pages table shape is fixed.

## [0.12.16] - 2026-06-30

### Performance
- **Auxiliary BM25F fields no longer store token positions.** The boosted `file_path_text` and `signature` Tantivy fields now keep code-tokenized matching while using basic postings instead of full positions; the main code body field still keeps positions. Six paired million-chunk runs preserved expected recall@20 and MRR@20 at `1.0`, reduced warm distinct-query p95 to `0.917x`, reduced index size to `0.976x`, and held fresh-index throughput at `1.003x`.

### Security
- **Updated `anyhow` past the denied RustSec advisory.** The lockfile now uses `anyhow 1.0.103`, clearing the `RUSTSEC-2026-0190` unsoundness denial in `cargo audit --deny yanked --deny unsound`.

## [0.12.15] - 2026-06-29

### Performance
- **Indexing avoids per-chunk random UUID syscalls.** Transient chunk IDs now reuse the existing stable 128-bit chunk digest instead of calling `Uuid::new_v4()` for every chunk. A 101k-chunk strace dropped `getrandom` calls from `101,092` to `90`, and the generated 505k-chunk profile improved median fresh-index time from `4103.9 ms` to `3949.6 ms` while preserving recall@20 and MRR@20 at `1.0`.

### Fixed
- **Linux Clippy can validate portable accelerator feature flags.** macOS Metal/Accelerate Candle dependencies are isolated to macOS targets, so Linux CI can lint `--features accelerate,metal` without pulling Apple frameworks or requiring CUDA `nvcc`.

## [0.12.14] - 2026-06-29

### Performance
- **Fresh indexes now stage bulk writes and promote complete stores atomically.** Initial indexing writes SQLite, Tantivy, and hash-vector stores into a private staging directory with fast bulk-write pragmas, restores WAL mode, then promotes the complete stores into place. The 505k-chunk generated profile improved fresh index wall time from `4295.9 ms` to `3891.1 ms`, reduced filesystem writes from `432.3 MB` to `261.3 MB`, and preserved warm search p95 (`0.698 ms` to `0.688 ms`) across 1000 query samples.
- **Symbol definition persistence batches inserts.** Indexing now accumulates normalized symbol rows and flushes 256-row SQLite inserts instead of executing one statement per discovered definition.

### Testing
- **Longer benchmark samples guard search quality and latency noise.** The release comparison used 1000 warm distinct-query samples and retained expected recall@20 and MRR@20 at `1.0`.

## [0.12.13] - 2026-06-29

### Performance
- **Fresh SQLite indexing writes less and finishes sooner.** Fresh indexes now defer secondary metadata indexes until after bulk chunk ingestion and reuse transaction-scoped prepared statements for chunk, symbol, and dependency persistence. The generated 505k-chunk profile improved fresh index wall time from `4907 ms` to `4096 ms`, reduced filesystem writes from `640.7 MB` to `447.2 MB`, and cut the persist phase from `4745 ms` to `3000 ms`.

### Testing
- **Discarded non-winning speedup candidates before release.** DuckDB was tested as a metadata-store replacement and rejected because ivygrep's point lookups and symbol joins were slower despite smaller files. A broad lexical expansion short-circuit was also reverted after relevance dropped. CLI startup and full staging-directory swaps remain follow-up candidates rather than shipped changes.

## [0.12.12] - 2026-06-28

### Performance
- **CUDA transformer enhancement sizes batches from live GPU headroom.** Linux CUDA builds now default background MiniLM enhancement to batch size 8 only when `nvidia-smi` reports enough free VRAM and low GPU utilization; memory pressure or an already-busy GPU backs off to smaller batches instead of competing with workloads such as local LLM serving. Local RTX 5070 Ti validation completed the ripgrep fixture in `10.34s` with `4,814 / 4,814` vectors under idle conditions.
- **Foreground transformer queries default to accelerator-backed embedding when available.** CUDA and Metal builds use the local accelerator for transformer query vectors, prefetch common neural query vectors in the daemon, and skip redundant hash fallback once neural coverage is complete.
- **Neural search fusion trims candidate bookkeeping overhead.** Fusion stores compact source masks and hydrates only the bounded rerank set before boost computation, reducing hot neural search work while preserving result behavior.

### Fixed
- **Foreground accelerator builds satisfy the cross-platform lint gate.** The accelerator selection path now compiles cleanly under the full CI matrix.

## [0.12.11] - 2026-06-28

### Performance
- **Transformer enhancement batches real forward passes on accelerator backends.** CUDA and Metal builds now run contiguous MiniLM batches for background neural enhancement, choose larger accelerator batch defaults when free VRAM allows it, and keep bounded handle pools so indexing can push more work onto the GPU without oversubscribing laptops.
- **Daemon neural search warms the model before serving the first neural query.** The first post-load query no longer pays the one-time backend/tokenizer warmup cost; on the local CUDA MiniLM fixture, first neural embedding moved from `9.23 ms` to `4.22 ms` and daemon search total from `13.77 ms` to `9.00 ms`.
- **Single-query transformer embedding avoids a tokenizer clone on every request.** The hot search path now reuses each handle's configured tokenizer directly, reducing cache-bypassed daemon search averages from `24.1 ms` to `19.9 ms` on the local 15k-chunk MiniLM fixture.

### Fixed
- **macOS available-memory detection no longer underreports safe transformer capacity.** Neural worker and batch sizing now use the corrected platform calculation before applying memory caps.

## [0.12.10] - 2026-06-27

### Changed
- **Natural-language member queries now resolve structural definitions more precisely.** Object-qualified references such as `app.handle` and `res.sendFile`, plus bounded `... internals` phrases, use the existing parser-derived symbol index while preserving the winning file and score and centering the exact member definition.

### Performance
- **The paired 63-repository, 1,251-query retrieval suite improves overall nDCG@10 from `0.8238` to `0.8254` and architecture nDCG@10 from `0.7681` to `0.7736`.** All 11 queries eligible for the new structural rules show three improvements and no regressions, while warm p50 remains effectively flat at `4.24 ms`.
- **The generated-corpus CI gate reports no recall loss or significant performance regression.** Warm distinct-query p95 measures `0.792x` the baseline, with index throughput at `1.021x` and unchanged expected recall@20.

### Testing
- Added end-to-end and unit coverage for object-qualified prose, exact-case member selection, bounded compound-symbol inference, and namespace, package, path, and type-shaped false-positive rejection.

## [0.12.9] - 2026-06-26

### Added
- **Rust modules now index parser-derived `#[doc = include_str!(...)]` documentation under the owning source file.** Included documentation participates in hybrid retrieval, dependency edits refresh the owner incrementally, worktree overlays remain current, and traversal, symlink escape, ignored-file, binary, and size limits bound the feature.
- **Tree-sitter structural chunking now covers Kotlin, Elixir, and Zig.** Kotlin classes, objects, functions, and type aliases; Elixir modules, protocols, implementations, functions, and macros; and Zig containers, functions, and tests are ranked as declarations instead of fallback text windows.
- **TypeScript and TSX structural chunking now covers type aliases, enums, and abstract classes.** Language-aware retrieval can rank these declarations directly instead of relying on a surrounding module chunk. Existing indexes rebuild once under format version 16.

### Changed
- **Result count and snippet size are now documented as independent controls.** CLI help, MCP schema guidance, the agent integration guide, and benchmark notes distinguish `limit` from `context` and explicitly state that a smaller snippet payload is not itself a relevance improvement.
- **Natural-language code concepts now map to conservative canonical vocabulary without widening global candidate budgets.** Bounded phrase aliases cover multipart payloads, server-sent events, and error formatting. Mixed long aliases and precise acronyms use specificity ranking only when generic acronym density would hide the canonical file.
- **Exact symbol fusion prefers case-exact canonical declarations and less-qualified module names.** This resolves collisions such as Kotlin `Flow` versus `flow` and Elixir `Ecto.Schema` versus `Ecto.Repo.Schema`.
- **Exact-symbol candidates are ordered before truncation.** Case-exact declarations and canonical file stems now outrank partial definitions such as `SqlMapper.Async.cs`, while focused snippets skip leading documentation and center the source declaration.
- **Path ranking recognizes conservative code-word roots.** Identifier-aware path terms connect queries such as `validation`, `reflection`, `resolution`, and `connection` to canonical `Validate`, `Reflective`, `resolver`, and `connector` files without enabling broad fuzzy matching.
- **Natural-language path recall is file-aware and bounded.** A 20-file path pass removes language-extension noise, overfetches documents before file deduplication, and applies path evidence to the best lexical chunk without widening the global rerank budget.
- **Primary C and C++ headers are no longer density-demoted.** Public API declarations in `.h`, `.hh`, and `.hpp` files receive the same primary-source treatment as implementation files.
- **Result backfill now requires ten distinct candidate files.** Chunk-heavy matches in one file can no longer authorize unrelated filler results, preserving result breadth as a relevance decision instead of a benchmark artifact.

### Performance
- **The daemon removes repeated workspace and neural-readiness setup from hot queries.** Exact workspace roots and stamp-validated neural model/vector status are cached with bounded entries and invalidated when index artifacts change.
- **Semantic retrieval defers stored-text decompression until fusion knows its rerank set.** ANN candidates first load only ranking metadata, then the existing bounded top-30 pass hydrates full text. Query analysis is also reused across fusion, filtering, and presentation.
- **The built-in 23-query relevance suite improves first-result precision without trading away recall.** Five paired runs against `main` move precision@1 from `0.391` to `0.478`, MRR from `0.590` to `0.616`, and nDCG@10 from `0.627` to `0.647`, while recall@5 remains `0.739`.
- **The independent retrieval fixture preserves exact output quality while reducing warm median latency.** Five paired hash-mode runs retain `1.000` nDCG@10, MRR, and recall@20 with exactly one relevant file returned per query; warm p50 moves from `17.38 ms` to `14.42 ms`.

### Fixed
- **Declaration signature indexing now skips leading documentation comments and attributes.** JavaDoc, C-style documentation, Java annotations, and C# attributes no longer occupy the boosted signature field instead of the declaration. The field now uses its intended 5x BM25 boost, and existing indexes rebuild once under format version 16.
- **Haskell structural indexing no longer risks native heap corruption on Linux ARM64.** The vendored `tree-sitter-haskell` 0.23.1 grammar carries the upstream strict-aliasing fix proposed in tree-sitter/tree-sitter-haskell#157, replacing stale-pointer array growth in its external scanner.
- **Bulk ANN enhancement no longer bypasses bounded capacity growth.** Hash and neural stores translate a 128 MiB estimated per-entry memory budget into a growth cap using vector dimensions, quantization, and graph connectivity. The vendored USearch allocator now keeps hash-table slack from amplifying vector, mutex, and HNSW capacity. Million-chunk enhancement can no longer pre-reserve native capacity for the entire remaining corpus before low-memory checks run.
- **Optional transformer workers share one immutable model instead of reloading weights per thread.** An eight-worker MiniLM probe reduced peak RSS from 869.7 MB to 217.2 MB with equivalent median throughput. Background worker count is also capped by cgroup-aware available memory on Linux and native memory reporting on macOS and Windows.

### Testing
- Added parser, Haskell external-scanner safety, dependency invalidation, gitignore, resource-bound, worktree-overlay, Kotlin/Elixir/Zig and TypeScript structural-declaration, exact-case symbol, namespace-specificity, canonical-file ordering, definition-centered preview, primary-header density, path-morphology, canonical vocabulary, and acronym-specificity coverage.

## [0.12.8] - 2026-06-25

### Performance
- **Hybrid search avoids repeated identity and provenance allocations.** Candidate deduplication, fusion, and result filtering use persisted vector keys and compact source masks instead of formatted logical IDs and per-candidate hash sets. Criterion improves exact-symbol queries by 4.7%, complex phrases by 2.5%, and bounded reranking by 7.4%.
- **Symbol indexing uses its existing primary-key B-tree.** The redundant legacy `normalized_name` index is removed, reducing a 100k-chunk index by 3.6 MiB and improving fresh-index timing by 1.3%.
- **The cache-bypassed 60-query comparison now measures 8.98 ms p50 and 11.70 ms p95 while retaining 1.000 symbol nDCG.**

### Fixed
- **Exact symbol promotion prefers canonical definitions over re-export text.** This makes precise symbol lookup deterministic when a public re-export and a definition are both candidates.

### Testing
- Added regression coverage for redundant symbol-index cleanup and canonical-definition promotion.

## [0.12.7] - 2026-06-24

### Added
- **Optional code-specialized static embeddings.** `IVYGREP_MODEL_PROFILE=potion-code` runs the revision-pinned PotionCode Model2Vec profile through native Rust weighted pooling.

### Changed
- **Hybrid retrieval is more precise and more diverse.** Public re-exports, mixed-case symbols embedded in natural-language queries, conservative code-word roots, one-result-per-file diversity, and authority-gated backfill improve definition and architecture discovery without expanding result payloads.
- **Search work is deferred until candidates need it.** Literal variants share one bounded pass, lexical and path text loading is limited to rerank candidates, and pooled search contexts reuse validated file contents.

### Performance
- **The pinned 60-query relevance suite improved aggregate, architecture, and symbol ranking.** Cache-bypassed results report `0.813` nDCG@10, `1.000` symbol nDCG, `11.93 ms` p95, and 392 approximate top-10 snippet tokens. Payload size is reported separately from relevance.
- **Completed indexes avoid repeated completeness scans.** Generation sentinels and Tantivy manifest stamps remove unnecessary vector-cardinality and directory audits from hot queries.

### Fixed
- **Equivalent whitespace produces equivalent search behavior and cache keys.** Query normalization now applies before routing, retrieval, fusion, and reranking.
- **Comparison latency excludes result-cache replay.** The public harness explicitly disables the daemon result cache for timed queries; its `9.09 ms` p50 remains the next optimization target rather than being hidden by replay.

### Testing
- Added cache-disable, whitespace-equivalence, re-export symbol, Model2Vec profile, search-context cache, authoritative backfill, and benchmark fairness coverage.

## [0.12.6] - 2026-06-18

### Performance
- **Daemon hot queries avoid full workspace status scans.** CLI daemon routing now checks a lightweight protocol version and per-workspace runtime status instead of enumerating every indexed workspace before each query.

### Fixed
- **GPU backend smoke checks select the intended embedding profile.** The neural backend helper can set `IVYGREP_MODEL_PROFILE`, and the Metal/CUDA docs validate the Candle-backed `general` profile explicitly.

### Testing
- Added lightweight daemon runtime-status coverage and model-profile propagation coverage for neural backend validation.

## [0.12.5] - 2026-06-18

### Fixed
- **Neural acceptance tolerates transient model-host throttling.** The E2E helper retries bounded HTTP 429 and transient server/network failures while permanent model or product errors still fail immediately.

### Testing
- Added executable recovery and fail-fast coverage for the neural E2E helper.

## [0.12.4] - 2026-06-18

### Fixed
- **Windows indexing tolerates transient filesystem sharing violations.** Tantivy segment creation retries bounded `PermissionDenied` failures while preserving its existing read, lock, delete, atomic metadata, and watch behavior.

### Testing
- Added deterministic retry coverage and retained concurrent search/reindex validation across the standard Windows matrix and cross-platform E2E workflow.

## [0.12.3] - 2026-06-18

### Fixed
- **CLI upgrades replace stale daemons before dispatching work.** Legacy status payloads deserialize with conservative defaults, and every non-status daemon operation verifies the resident version before indexing, searching, or removing a workspace.

### Testing
- Added protocol compatibility and fake-daemon restart regressions, plus an installed-binary upgrade smoke test that rebuilds an outdated index and returns an indexed result.

## [0.12.2] - 2026-06-18

### Fixed
- **ARM64 tag E2E uses the required Git tooling.** The QEMU smoke lane now runs documented procedures in the same pinned Git-capable image used by release acceptance, then checks daemon equivalence in a separate Python image.
- **Critical benchmark failures require confirmation.** A threshold breach is rerun in reverse order before CI fails, preserving the regression gate while filtering transient shared-runner spikes.

## [0.12.1] - 2026-06-17

### Added
- **One-command verified installers.** Linux, macOS, and Windows users can install the latest portable release with a single command; the maintained installers select the correct archive, verify its published SHA-256 checksum, and configure a standard user binary location.
- **Coding-agent integration guidance.** Verified Claude Code, Codex, Cursor, Gemini CLI, and OpenCode configurations now document explicit workspace scoping, exact-versus-semantic query selection, and shared-base worktree overlays.

### Fixed
- **MCP sessions now keep indexed workspaces fresh.** Agent searches start or reconnect the shared daemon watcher, route queries through its cached search contexts, and fall back to local Merkle reconciliation when daemon spawning is unavailable.
- **Windows MCP can auto-spawn the daemon.** Executable detection now recognizes `ig.exe` in addition to the Unix `ig` filename.

### Testing
- Added same-session MCP edit coverage, Windows daemon-name regression coverage, installer documentation checks, Unix ShellCheck validation, and an isolated native PowerShell install smoke test in CI.

## [0.12.0] - 2026-06-17

### Changed
- **Windows releases now include local neural search and USearch ANN.** Windows uses Rust-managed buffer persistence around USearch to support Unicode index paths, replace active stores while readers are open, and retain crash recovery without falling back to linear vector scans. The executable opts into long-path-aware Windows APIs and statically links the Visual C++ runtime.
- **USearch 2.24 now builds on Windows.** The vendored backend retains the proven F16 performance while backporting MSVC fixes for the stale `MAP_FAILED` reference and consistent static-runtime linking.

### Testing
- Windows CI now runs the default neural feature set, optimized vector-store tests, backend attribution, Unicode workspace/index paths, cached offline model reuse, and exact release-archive acceptance.

## [0.11.2] - 2026-06-16

### Fixed
- **Workspace discovery validates Git roots.** Invalid `.git` ancestors are ignored instead of causing unrelated parent directories to be indexed.
- **Worktree checkpoints stay inside overlay storage.** Completing an overlay index no longer creates an empty `metadata.sqlite3` beside the delta stores.
- **Linux ARM64 release acceptance includes Git offline.** The exact-archive QEMU smoke test now uses a pinned Git-capable image while retaining network isolation.

### Performance
- **Clean worktrees reuse one shared base index.** Worktree creation skips redundant base rewrites when the indexed checkout state is unchanged; sparse-checkout and other checkout-shape changes run incremental base reconciliation before an overlay inherits data.

### Testing
- Added sequential and concurrent multi-worktree coverage that requires one shared base index, exact per-worktree chunk/tombstone deltas, no full-store artifacts in overlay directories, and correct refresh behavior for dirty or newly committed base content.

## [0.11.0] - 2026-06-16

### Added
- **Public retrieval evidence is reproducible and claim-gated.** Pinned public datasets now report repeated quality, latency, indexing, memory, and footprint metrics with leakage checks, per-task results, and machine-readable artifacts.
- **Portable neural selection and bounded learned reranking are integrated.** Query intent routes lexical, hash, and neural evidence adaptively, while a held-out public evaluation gates the local reranker and preserves deterministic fallback.
- **Release archives are authoritative.** All five supported targets publish checksums, SPDX SBOMs, provenance, and binary-size metadata, then execute the exact packaged bytes before a release can be created.
- **An evidence dashboard controls public claims.** Versioned histories retain context, variance, immutable commit links, regressions, and unavailable comparable-system results.

### Changed
- **Index format v11 reduces million-chunk storage by 57%.** Compact integer chunk keys, derived logical IDs, narrower symbol persistence, and tier accounting reduce the frozen generated-corpus index from 1.06 GiB to 469 MiB while keeping aggregate nDCG@10 within the 2-point gate.
- **Million-scale query and indexing paths are bounded.** Persistent daemon sessions, hot-query caching, less frequent storage checkpoints, and explicit I/O-ceiling reporting improve warm distinct-query p95 by more than 2x without hiding saturated-host results.
- **Status and doctor expose storage tiers and compaction health.** Deep checks remain explicit, and repair mode can checkpoint and vacuum reclaimable SQLite storage.

### Testing
- Added archive traversal checks, baseline x86 QEMU execution, no-network hash fallback, cached-model import, stale-index rebuild coverage, cross-platform daemon equivalence, public million-chunk footprint gates, and generated evidence consistency checks.

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
- **Skip minified bundles / single-line blobs when indexing.** A file with a 50 KB+ run and no line break (minified JS/CSS, packed data) is skipped during indexing because it would otherwise become one low-value chunk that dilutes relevance on large monorepos. Complements the existing 16 MB file-size cap, catching minified files that fall under it. Large hand-written docs (normal line lengths) are unaffected.

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
- **Path-based score boosting:** Files whose path contains the query term (e.g., searching for "my-service" surfaces `apps/my-service/` at the top) now receive a path boost so directory and filename matches outrank generic code hits.

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
- **BM25F multi-field scoring:** New `file_path_text` (5× boost) and `signature` (10× boost) fields bring Sourcegraph/Zoekt-style field-level relevance — function definitions and filename matches rank above body text
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
- **Search relevance overhaul:** Rebalanced hybrid RRF scoring weights and result ordering
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
- **32x larger enhancement batches:** Increased ONNX inference batch size from 16 to 512 chunks, reducing session overhead during background neural enhancement
- **Skip decompression for completed keys:** Enhancement loop now checks vector store before decompressing text, avoiding ~1M redundant zstd decompressions on resume
- **CPU affinity limiting (Linux):** Background enhancement now uses `sched_setaffinity` to pin ONNX threads to 25% of available cores (capped at 4), keeping the system responsive during long-running enhancement
- **Instant initial indexing:** `ig --add` now always uses the lightweight hash model for initial indexing; neural enhancement runs exclusively in the background daemon

### Fixed
- **Backward compatibility for `is_ignored` field:** Tantivy field is now optional, allowing v0.5.13 to read indexes created by older versions without crashing
- **Honest CUDA detection:** Added cuDNN probe to verify CUDA is actually functional before reporting GPU acceleration in `ig --status`

## [0.5.12] — 2026-04-06

### Performance
- Bounded ONNX/GPU allocations by enforcing maximum chunk counts for embeddings, capping VRAM well below 8GB during large batches
- Fixed a bug where initial indexing incorrectly instantiated the background neural model even when `--hash` was passed

## [0.5.11] — 2026-04-06

### Added
- Optional hardware acceleration for Linux users with CUDA/GPU installed (speeds up neural embedding generation)

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
- **Batched SQLite transaction commits:** The indexer now batches SQLite transaction commits by chunk count instead of per file, improving indexing throughput on Linux.

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

Introduced **Worktree-Aware Thin Overlay Indexing**.

### Feature: Shared Base + Thin Overlays
- **Worktree Indexing:** When indexing a `git worktree`, ivygrep reads the base index and constructs an overlay (`metadata.sqlite3`, `vectors...`) containing only added, modified, or deleted worktree chunks.
- **Overlay Tombstones:** Deleted or modified worktree files create SQLite tombstones. `SearchContext` merges base and overlay indexes while excluding tombstoned base content.
- **Base Auto-Indexing Cascade:** If a worktree is indexed before its base checkout, ivygrep locks and builds the base index before evaluating the overlay delta.
- **Background Upgrade Cascading:** Background neural enhancement operations automatically cascade into parent base indices when triggered from a dependent worktree.
- **UI Tracking Hierarchy:** `ig --status` displays base repositories and their worktree overlays as an indented tree. Index footprints report overlay delta bytes separately from the main checkout.

## [0.5.3] — 2026-04-03

Minor patch addressing Clippy CI constraints.
- Resolved `clippy::collapsible_if` nested block rules originating from integration test additions.

## [0.5.2] — 2026-04-03

- **CoreML Thermal/Cache Tuning:** Reduced the ONNX background execution batch size from 64 to 16. Batch size 64 caused thermal throttling and L2 cache thrashing on Apple Silicon / CoreML execution providers. The new limit retains 2× batch throughput over v0.5.0 while keeping the desktop responsive.

## [0.5.1] — 2026-04-03

- **ONNX Throughput Boost:** Increased the background neural enhancement batch size by 8× (from 8 to 64). To limit CoreML/ONNX tensor allocation, chunk text is bounded and truncated at ~1024 bytes before tokenization.

## [0.5.0] — 2026-04-03

Storage efficiency and stability release. Index-to-source ratio fell from **~6.5× to ~2.3×**.

> [!WARNING]
> **Breaking Change:** Due to the migration of neural and hash vectors to FP16 quantization, and the addition of `zstd` compression for SQLite, existing indices are incompatible. Please wipe your local `~/.local/share/ivygrep/` directory or run `ig --add . --force` before performing new searches to avoid mismatched chunks.

### Storage & Performance
- **F16 Vector Quantization:** `USearch` indexes now use `ScalarKind::F16` for hash embeddings, halving the footprint of `.usearch` stores.
- **SQLite zstd Compression:** Compressed raw `chunks.text` values with `zstd`. Legacy uncompressed rows are auto-detected and decoded.
- **Tantivy Store Truncation:** Removed the `STORED` flag from Tantivy's text index. Full lexical matches now read text from SQLite, removing more than 500 MB per index.

### Stability & Indexing Pipeline
- **Tree-sitter Timeout Engine:** Tree-sitter bindings now use `ParseOptions` with `progress_callback` and a 100 ms parser limit, preventing deadlocks on obfuscated, minified, or deeply nested input.
- **Enhancement Restart:** Fixed an interruption bug that permanently halted neural enhancement. Background tasks calculate remaining work through `.needs_neural_enhancement()` and resume processing.
- **First-run Spinner Resolution:** Initial daemon chunking progress now writes and parses `.indexing.progress`, so progress no longer remains at zero.

## [0.4.7] — 2026-04-03

Introduced an indexed literal-search path for exact string queries.

### Performance
- **Index-Backed Literal Search (`--literal` / `-l`):** 5.6× faster than the old `--regex` mode on large repositories. Skips BM25 and neural enhancement, using Tantivy phrase queries to narrow candidates before an exact case-insensitive scan.
- **Daemon-Routed Exact Matches:** The new literal fast-path runs through the daemon by default (`DaemonRequest::LiteralSearch`), meaning if the daemon hasn't finished loading the 134MB neural model, exact text searches still complete in milliseconds.
- **MCP Literal Parameter:** `ig_search` now supports `literal: true` directly to provide agents with a high-speed search alternative when semantic search isn't needed.

### Changed
- Hide the slow `--regex` flag from `--help` (still works, but users are steered to `--literal` or `rg` for pure regex).

## [0.4.6] — 2026-04-03

Query latency release. Uncached searches over more than 90,000 files measured about 15–40 ms.

### Performance
- **Identifier Fast-Path:** Single-word identifier queries such as `kfree` or `malloc` skip the ONNX vector step and use BM25 SQL. Speed increased by over 10× (`~40 ms` query latency on Linux).
- **No-Rescan Penalty:** Local searches skip duplicate workspace Merkle re-indexing. When the workspace is indexed, CLI uses daemon search and avoids about two seconds of indexing latency.
- **Daemon Speedups:** Fixed IPC RPC errors caused by old daemon sockets surviving binary restarts and enhanced search options.
- **Lazy Models:** Reduced memory use by loading embedding models on demand.

## [0.4.1] — 2026-04-02

A performance-focused release for large monorepos
(tested on a 269K-file, 2.3M-chunk, 17 GB production codebase). Indexing is up to 35%
faster, `ig --status` dropped from 20 s to 24 ms, and filtered queries now
bypass full-corpus vector scans.

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
  large indexing runs (`3c94545`).
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
