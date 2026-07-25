# Optimization A/B report, 2026-07-24

This report records independent experiments from ivygrep `71676d2`. Each candidate started from the same commit in an isolated worktree. Rejected changes were removed before the next experiment. The final branch combines only retained changes.

## Decisions

### Kept

| Area | Candidate | Baseline | Candidate | Decision |
|---|---|---:|---:|---|
| Context relevance | Test-aware graph and anchor retrieval | Recall `.75000`; test recall `.000` | Recall `.81944`; test recall `1.000` | Keep. Both labeled test misses recovered; no task regressed. |
| Context budget | Calibrate deterministic estimate by `3/5` | Estimate/actual median `1.817` on `o200k_base` | `1.092`; zero underestimates across 30 packs | Keep. Same selected paths with more source inside real model budgets. |
| Context composition | Raise post-role target from 12 to 14 | Composed recall `.79167`; primary `.77778` | Both `.81944` across repeated debug/release runs; tokens +14.7% to +15.9% | Keep. Prevents test evidence from crowding a primary file while staying inside budget. |
| Context evaluation | Curated relevance labels | Labels had to be changed files | Relevant unchanged paths can be pinned to a base tree | Keep. Enables expert-labeled cross-project tasks without weakening historical validation. |
| Relevance CI | Test-role gate | Test recall visible but ungated | Frozen suite requires `1.0` | Keep. Protects the behavior fixed in this round. |
| Incremental index | Exact vector-key delta with crossover | Million-row `COUNT(DISTINCT)` took 47-48 ms warm | 100 keys took 1.05 ms per probe pass; changes over 512 keys fall back to full count | Keep. Collision-aware small deltas win; large changes retain baseline complexity. |
| Merkle finalization | Stream root hash | 45.01 ms and 128 MB temporary buffer at 1M entries | 14.85 ms and O(1) extra memory | Keep. Digest is byte-compatible with existing snapshots. |
| Neural resume | Key-first sparse reads at 75% coverage | 49.76 ms at 200K rows, 90% complete | 33.06 ms, 33.6% faster | Keep. Fresh and incomplete stores retain sequential scans. |
| Search cache | Workspace-selective eviction | One changed workspace cleared every cached query | Only keys containing changed workspace are removed | Keep. Multi-workspace queries still invalidate correctly. |
| Search cache | Preserve cache after no-op indexing | Every index request evicted results | Zero-change manual and watcher indexes retain results | Keep. Real changes still invalidate. |
| Watcher | Adaptive debounce | 2.009 s single-event quiet wait | 251.4 ms, 87.5% lower; bursts use 750 ms | Keep. Bounded 30-second starvation protection remains. |

Detailed context measurements and tokenizer calibration are in [`context-ab-2026-07-24.md`](context-ab-2026-07-24.md).

### Discarded or deferred

| Area | Candidate | Result | Decision |
|---|---|---|---|
| Context graph | Expand 12 to 24 relationships | No recall gain; tokens +4.5%; p50 +88% | Discard. |
| Context packing | Score/token plus 15% file novelty | Tokens -11.6%; recall `.81944` to `.75000` | Discard. |
| Co-change | Always-on raw edges | No recall gain; tokens +3.3%; extra Git work | Discard. |
| Co-change | Always-on Jaccard edges | No recall gain; slower than baseline | Discard. |
| Neural ANN | M16/add64 quality graph | Recall@20 +.0133, but nDCG -0.00535, MRR -0.00746, enhancement +133%, warm p95 +30% | Discard. Public task quality and latency override synthetic ANN recall. |
| Neural ANN | Native M16/add128, M4, M8 profiles | Native build cost highest; smaller graphs reached only `.061-.153` synthetic recall@10 | Discard. |
| Neural search | Global 2x ANN overfetch | nDCG +.00536 and recall +.0167; warm p50 +75%, p95 +23% | Discard. Cost is global; benefit needs targeted routing. |
| Readiness | Eager hash enhancement | Ready time 151.4 to 453.7 ms; index 4.73 to 10.58 MB; exact-corpus quality unchanged | Discard. Keep deferred enhancement. |
| Daemon scheduling | Weighted permits | No representative mixed-load win; added starvation and deadlock risk | Defer until a mixed search/index trace exists. |
| Git state | Batch metadata commands | Tree/status scan dominates; batching does not remove it | Discard. |
| Neural reranking | Router, metadata prefix, abstention | Missing fit/eval split or labeled negative queries | Defer. Do not tune against evaluation data. |
| Marketing | Publish local agent-token reduction as headline | Only three local historical tasks | Defer claim. Keep outcome screen, expand curated suite first. |

## Real-agent outcome screen

Codex CLI `0.144.6` ran with user configuration and MCPs disabled, same default model, frozen historical trees, and an explicit no-modification instruction; every evaluation worktree remained clean. Native arm used shell/file discovery. Context arms had one required `ig context --hash --budget 8000` call and at most three extra reads.

| Task | Expected | Native hits / proposed | Previous context | Candidate context | Candidate input delta vs native |
|---|---:|---:|---:|---:|---:|
| Reduce indexing memory and defer neural work | 3 | 2 / 4 | 3 / 5 | 3 / 4 | 298,276 to 119,948, -59.8% |
| Improve web result exploration | 2 | 2 / 7 | 0 / 5 | 2 / 7 | 127,078 to 92,627, -27.1% |
| Speed filtered search and incremental indexing | 3 | 3 / 7 | 2 / 3 | 3 / 5 | 220,424 to 39,252, -82.2% |
| **Aggregate** | **8** | **7 / 18** | **5 / 13** | **8 / 16** | **645,778 to 251,827, -61.0%** |

Candidate expected-file recall is `1.000`, versus `.875` for native discovery and `.625` for previous context behavior. Expected-file precision is `.500`, versus `.389` native. Candidate used seven shell commands across three sessions, versus about 17 native. Input totals include cached/system context and are a paired agent-cost signal, not a universal token-savings claim.

## Neural profile screen

This section records local screening evidence for discarded arms, not a permanently reproducible benchmark artifact. Candidate patches and raw outputs are intentionally not tracked. The shape-based HNSW rule gives 256-dimension F16 neural stores the low-cost hash profile. A 20,000-vector random screen confirmed low exact-neighbor recall, then a three-run public task matrix tested whether denser graphs improve product outcomes.

| Metric | M2/add8 baseline | M16/add64 |
|---|---:|---:|
| nDCG@10 | `.592033` | `.586688` |
| MRR@10 | `.535456` | `.528000` |
| Precision@5 | `.136000` | `.136000` |
| Recall@20 | `.790000` | `.803333` |
| Neural enhancement | 740.5 ms | 1,728.4 ms |
| Warm p50 | 22.34 ms | 40.97 ms |
| Warm p95 | 446.52 ms | 578.66 ms |

No graph or identity migration is retained. Existing public outcome is better balanced than synthetic nearest-neighbor recall.

Arm definitions were isolated source patches over `71676d2`:

- `m2a8`: clean behavior for 256-dimensional F16 stores, connectivity `2`, add expansion `8`, search expansion `64`; binary SHA-256 `364c540685161c7dbefbfd6b8230d92a72cc48ce6024581fb2a40776cd9aa362`.
- `m16a64`: explicit hash/neural store purpose, with neural 256-dimensional F16 stores at connectivity `16`, add expansion `64`, search expansion `64`, plus a schema-2 rebuild only for affected 256-dimensional neural stores; binary SHA-256 `3f93a25f442adc7932bfff42a9501fe2d34c54990505257ec09c39c5415b171b`.
- `m2a8-over2`: clean graph settings, but unfiltered neural ANN requests `min(2 * semantic_limit, 4000)` neighbors before truncating back to `semantic_limit`; binary SHA-256 `2a7cabd62d1d2f1f1ee478fa9a3385fb8f2dec9c2aff4ccf58e2f1c242a580c3`.

Candidate patches and temporary results are not retained in repository. Binary digests and patch shapes above preserve audit context, but do not make discarded arms reproducible from retained tree. Retained tree requires no vector schema migration.

## Readiness screen

A deterministic 11,001-chunk corpus compared lexical-ready indexing with eager hash enhancement in separate homes. Both arms retained recall@20 and MRR@20 of `1.0` for the exact workload.

| Metric | Lexical ready | Eager hash |
|---|---:|---:|
| Initial index | 151.45 ms | 151.35 ms |
| Total ready time | 151.45 ms | 453.67 ms |
| Index size | 4.73 MB | 10.58 MB |
| Warm engine p95 | 0.410 ms | 1.333 ms |
| Filtered p95 | 0.500 ms | 11.379 ms |

This small exact corpus is a routing check, not a semantic-quality benchmark. It supports current lexical-first, background-enhancement design.

## Index and daemon A/B methods

Timing-only harnesses were removed after measurement; retained regression tests cover contracts without adding benchmark-only test cost.

- **Vector cardinality:** a one-million-row SQLite table with an index on `vector_key` compared warm `COUNT(DISTINCT vector_key)` against indexed presence probes. Independent crossover measurements were 1.05 ms for 100 keys, 9.31 ms for 1,000, 93.23 ms for 10,000, and 231.48 ms for 25,000 per pass, versus 47-48 ms for a full count. Final implementation tracks initial/final presence through 512 distinct touched keys, then stops probing and uses one full count. Shared-key collisions and fallback cardinality have regression coverage.
- **Merkle finalization:** an ignored release harness built one million ordered `(path, hash)` entries. Arm A concatenated all bytes before `xxh3_128`; arm B fed the identical sequence through `Xxh3::update`. `cargo test ab_streaming_root_hash --lib --release -- --ignored --nocapture` measured 45.00667 ms versus 14.851959 ms and a 128,000,000-byte baseline allocation. Permanent compatibility test compares both digests.
- **Neural resume:** an in-memory SQLite fixture held 200,000 unique keys with 512-byte blobs and simulated 90% vector coverage. Arm A read every `(vector_key, text)` row; arm B scanned keys then point-fetched only 20,000 missing blobs. `cargo test ab_sparse_neural_resume_sqlite_reads --lib --release -- --ignored --nocapture` measured 49.755035 ms versus 33.056307 ms. Production selects sparse reads at 75% coverage.
- **Cache eviction:** the regression fixture stores A-only, B-only, and A+B query keys. Baseline global clear loses all three; candidate invalidating A retains B-only while removing A-only and combined keys.
- **No-op cache:** the regression fixture indexes, caches a query, repeats indexing without changes, then checks the cached result survives. Baseline fails because every successful index cleared all results; candidate clears only for nonzero indexed or deleted files.
- **Watcher debounce:** an ignored Tokio harness measured `wait_for_watch_quiet` after one event. Baseline 2-second policy took 2.009468024 s; candidate 250 ms policy took 251.357922 ms. Separate policy coverage keeps bursts at 750 ms and continuous-event starvation capped at 30 seconds.

Weighted daemon admission was stopped before a production patch because no mixed search/index workload measures fairness, and guessed `acquire_many_owned` weights can add head-of-line blocking. A future trial must cover 8 and 32 search clients plus one continuous watcher, and keep only a candidate with at least 20% search-p95 improvement, zero errors, and under 10% freshness-lag regression. Git metadata batching reached a narrower negative result: combining three `rev-parse` arguments saves at most two launches while dominant `git status`, ignore, and sparse-checkout scans remain separate.

## Remaining evidence gaps

- Cross-project curated agent tasks. Current real-agent screen has three ivygrep tasks.
- Labeled irrelevant and no-answer queries for abstention and router-regret calibration.
- Mixed indexing/search traffic traces for weighted admission and tail-latency fairness.
- Long-lived multi-workspace cache memory and eviction behavior under churn.
- Watcher timing on macOS and Windows. Functional behavior remains covered cross-platform; latency was measured on Linux.
- Broader neural datasets before any graph, reranker-depth, or embedding-input migration.

## Reproduction

```bash
uv run scripts/bench_context_retrieval.py \
  --binary target/release/ig \
  --repo . \
  --tasks-from tests/fixtures/context_retrieval_tasks.json \
  --output /tmp/context-results.json \
  --modes context

uv run scripts/bench_million_chunks.py \
  --binary target/release/ig \
  --files 1000 \
  --chunks-per-file 10 \
  --query-samples 30 \
  --output /tmp/ivygrep-readiness.json
```

## Validation

Independent lanes passed 640-644 Rust library tests, 19 incremental CRUD integration tests, seven context benchmark tests, all-feature Clippy with warnings denied, formatting, and focused watcher, cache, neural-resume, vector-collision, Merkle, and relevance checks. The composed kept stack passed `./test.sh`, including 644 library tests, integration and stress lanes, frontend checks/build, and 155 Python tests. After final target-14 composition, vector-count crossover, and provenance fixes, 646 library tests, seven benchmark-script tests, all-feature Clippy, formatting, and diff hygiene passed; release-built frozen context gate passed before provenance-only normalization.
