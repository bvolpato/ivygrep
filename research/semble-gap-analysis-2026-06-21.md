# Semble gap analysis

Date: 2026-06-23

## Executive verdict

Semble is ahead on retrieval quality, query latency, public benchmark evidence,
and onboarding polish. ivygrep is ahead on incremental freshness, returned
context size, worktree support, exact code-navigation modes, deployment
simplicity, and release hardening.

This change closes most measured exact-symbol gap without trading away semantic
quality:

| Metric | Before | Current | Semble |
|---|---:|---:|---:|
| Overall nDCG@10 | 0.657 | 0.688 | 0.801 |
| Symbol nDCG@10 | 0.770 | 0.914 | 0.949 |
| Architecture nDCG@10 | 0.544 | 0.544 | 0.745 |
| Semantic nDCG@10 | 0.687 | 0.687 | 0.770 |
| Warm query p50 | 16.49 ms | 16.17 ms | 4.90 ms |
| Mean top-10 tokens | 251 | 252 | 1,593 |

Current bottleneck is architecture retrieval, followed by latency. Exact symbol
quality is now close enough that more symbol-specific tuning is lower priority.

Raw data and methodology:

- [Current result](../benchmarks/ivygrep-vs-semble.md)
- [Current JSON](../benchmarks/ivygrep-vs-semble.json)
- [Pre-fix baseline](../benchmarks/ivygrep-vs-semble-baseline.md)
- Harness: `scripts/benchmark_semble.py`

## Method

Representative benchmark uses Semble v0.4.1's pinned Axum, FastAPI, and tRPC
tasks:

- 60 annotated queries, 20 per repository.
- Same source files, labels, top-10 cutoff, and binary nDCG calculation.
- Three warm query measurements per task.
- Semble runs in-process, matching its official benchmark.
- ivygrep runs through one persistent daemon, excluding CLI startup.
- Both build fresh indexes for each run.

Repeated fresh builds produced ivygrep overall nDCG between 0.688 and 0.692.
Symbol nDCG remained exactly 0.914. Small semantic movement comes from ANN
construction variance.

## Gap closed

Investigation found three concrete symbol-ranking defects:

1. Symbol table allowed only one name per chunk.
2. Rust `impl Router` and TypeScript re-exports could be mistaken for canonical
   declarations.
3. Exact canonical definitions lost to substring usages such as `PathRouter`.

Changes:

- Migrated symbol schema to `(normalized_name, chunk_key)` composite identity.
- Extracted multiple explicit declarations from module chunks.
- Stopped treating implementation blocks and TypeScript re-exports as
  canonical definitions.
- Added a distinct exact-symbol fusion source.
- Promoted best canonical declaration for a single-identifier lookup.

Measured query movement:

- `Router`: relevant rank 3 to rank 1.
- `HTTPException`: rank 6 to rank 1.
- `inferProcedureInput`: rank 3 to rank 1.
- `Depends`: one relevant target rank 2 to rank 1.
- No benchmark query regressed against pre-fix baseline.

## What Semble has

Semble capabilities ivygrep should evaluate:

| Capability | Semble | ivygrep status |
|---|---|---|
| Static code embedding model | `potion-code-16M` Model2Vec | Larger local neural path plus hash fallback |
| Hybrid ranker | BM25, static vectors, RRF, code-aware ranking | BM25, hash/neural ANN, literal/path/symbol fusion |
| Public benchmark | 1,251 queries, 63 repos, 19 languages | Smaller public and internal suites |
| Related-code search | `find-related` | Missing as user-facing command/tool |
| Remote repository input | GitHub/GitLab URL support | Local paths only |
| Content presets | code, docs, config, all | Language/path filters, no equivalent presets |
| Ignore file | `.sembleignore` | Gitignore behavior and command filters |
| Agent setup | Interactive installer for major agents | Manual MCP setup |
| Savings reporting | Token/time savings tracker | Benchmark-only token reporting |
| Python API | Importable library | CLI, daemon protocol, and MCP |

Do not copy all of these blindly. Python API is not necessary for primary
positioning. Remote repositories, related-code search, content presets, and
setup automation have clear user value.

## Where ivygrep is better

Measured:

- Returns 6.3x fewer top-10 tokens on this benchmark.
- Full one-file hybrid refresh is about 3.0x faster.
- Lexical updates become searchable in roughly 65 ms before neural refresh.
- Hybrid-ready indexing is faster on all three repositories in this run.

Product and engineering:

- One Rust binary. No Python runtime or environment management.
- Incremental daemon and filesystem watcher.
- Shared base index plus per-worktree overlays.
- Literal, regex, symbol, reference, and caller modes.
- Local-only operation with no API keys or code uploads.
- Linux, macOS, and Windows release targets.
- Checksums, SPDX SBOMs, provenance, and attestations.

These are defensible differentiators. Quality and latency work must preserve
incremental updates, compact context, and worktree semantics.

## Next plan

### P0: Architecture retrieval

Target: architecture nDCG@10 at least 0.70, then Semble parity at 0.745.

1. Add per-query failure analysis for architecture tasks.
2. Evaluate `potion-code-16M` or an equivalent static code embedding model
   against current neural and hash tiers.
3. Add structure-aware rank features: declaration-to-reference proximity,
   module ownership, import/export flow, and call/implementation graph support.
4. Train or fit rank weights only against a development split. Keep held-out
   repositories for regression checks.
5. Expand same-data harness from 3 repositories to Semble's complete public
   task set.

### P0: Query latency

Target: warm p50 below 8 ms without reducing current quality.

1. Profile daemon query phases on the 60-query corpus.
2. Cache normalized query analysis and neural embeddings.
3. Skip passes that cannot affect routed query intent.
4. Bound SQLite candidate joins and reranking allocations.
5. Add p50/p95 guards to benchmark CI.

### P1: High-value product parity

1. `ig --related <path:line>` and matching MCP tool.
2. Remote Git URL input with explicit cache lifecycle.
3. `--content code|docs|config|all` presets.
4. `.ivygrepignore`, layered after `.gitignore`.
5. `ig setup` for agent installation and end-to-end MCP validation.
6. Optional local token-savings telemetry with no network reporting.

### P1: Public evidence

1. Run complete benchmark in scheduled CI.
2. Publish raw JSON, commit SHAs, machine details, and variance.
3. Put honest Semble comparison in README after full-suite validation.
4. Add a short demo showing worktree freshness and compact agent context.

## Success gates

- Overall nDCG@10 at least 0.80 on full shared benchmark.
- Symbol nDCG@10 at least 0.95.
- Architecture nDCG@10 at least 0.74.
- Warm p50 at most 8 ms.
- Full one-file refresh at most 300 ms.
- Mean top-10 output at most 400 tokens.
- Zero worktree freshness regressions.

## Sources

- [Semble repository](https://github.com/MinishLab/semble)
- [Semble benchmark methodology](https://github.com/MinishLab/semble/blob/main/benchmarks/README.md)
- [Semble introduction](https://minish.ai/packages/semble/introduction/)
