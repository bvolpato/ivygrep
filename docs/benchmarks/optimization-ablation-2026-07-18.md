# Optimization ablation, 2026-07-18

## Method

Baseline was clean `main` at `f0e6f9a48dce2f726a913a238ef9a6030ca89776` (`ivygrep 1.2.4`). Baseline and candidate release binaries were copied to immutable paths before trials. Paired trials alternated baseline/candidate order. Generated-corpus index trials used identical 100,001-chunk and 1,000,001-chunk corpora. Retrieval trials used pinned public datasets and existing Criterion fixtures.

Host: 32 logical CPUs, 125 GiB RAM, NVIDIA RTX 3060 12 GiB, CUDA 13.2. GPU remained under concurrent system load, making CUDA results conservative.

## Decisions

| # | Hypothesis | Decision | Independent evidence | Reason |
|---:|---|---|---|---|
| 1 | Replace intent routing with a neural-usefulness router | Discard prototype | Fresh 1,000-query public-core run favored hash over blended by 2.40% nDCG and 3.31% MRR, but 100-query model bakeoff showed neural improving `codetrans-contest` nDCG by 2.29% while losing on three other tasks. Published challenge tasks also contain neural wins. | No stable query-level usefulness signal generalized across profiles. Restored existing intent router instead of overfitting one matrix. |
| 2 | Add reranker confidence fallback | Discard fallback; keep learned reranker | Pinned 251-query artifact: learned nDCG 0.266765 versus deterministic 0.247594, +7.74%; learned MRR 0.221245 versus 0.200913, +10.12%. Fresh model-bakeoff aggregate gains were 4.9% to 6.2% nDCG and 6.9% to 8.8% MRR depending on mode. | Existing learned reranker wins strongly. No calibrated confidence feature justified fallback complexity. |
| 3 | Add pinned mixed-language real-repo performance corpus | Keep as monthly protocol | DataFusion commit `35f7501ba34ccfa0fe4b3d6b830f27cbac2b283d` produced 619,293 chunks across 15 languages/types. Generated million corpus is effectively one synthetic Rust shape. | Real-repo run exposed literal-search behavior absent from generated benchmark. Keep outside every-PR gate due checkout size and runtime. |
| 4 | Batch SQLite chunk inserts | Discard | Three paired 1M trials: multi-row `INSERT ... RETURNING` median 6,258.7 ms versus 6,107.5 ms single-row, 2.48% slower. Peak RSS fell only 1.40%. Counts remained exact. | Dynamic SQL, binding, and returned-row handling cost more than saved execute calls. Prototype removed. |
| 5 | Use dot-only exact scoring for normalized vectors | Discard | Criterion medians with dot-only: 50K 2.3378 ms versus 2.3370 ms; 25K 1.2815 ms versus 1.2817 ms; 5K 356.2 us versus 354.0 us. No significant win. | Vector retrieval dominates; normalization is effectively free beside it. F16 stored-vector norm drift adds relevance risk. Prototype removed. |
| 6 | Always use maximum first-pass filtered ANN overfetch | Discard | Three paired 100K trials: direct max overfetch p50 9.585 ms versus 8.494 ms, 12.8% slower; p95 10.608 ms versus 9.558 ms, 11.0% slower. Recall stayed 1.0. | Existing adaptive first pass plus bounded fallback is faster. Prototype removed. |
| 7 | Defer neural store/model work until neural-worthy query | Keep | Exact-identifier query after hash indexing: prior path 1.06 s and 334,432 KiB RSS, creating 7,842 neural vectors; candidate 0.55 s and 28,740 KiB RSS, retaining hash-only store. Final smoke on a separate 388-chunk corpus left exact search at 388 hash/0 neural vectors, while a natural-language search finished at 388 hash/388 neural vectors. | 48.1% lower wall time and 91.4% lower peak RSS for exact query. Natural-language routing still requests neural vectors; existing neural stores remain current. |
| 8 | Avoid fresh-index final full-table counts | Keep | Five 1M trials: candidate median 6,207.0 ms versus baseline 6,257.8 ms, 0.81% faster. Finalize median 1,661.2 ms versus 1,690.1 ms, 1.71% faster. All runs reported 10,001 files and 1,000,001 chunks. | Small, repeatable scale win with exact fresh-path counters. Incremental path keeps SQL-derived accounting. |
| 9 | Tune producer batch, channel, commit, and Tantivy memory | Keep file batch 64 only | Three paired 1M trials: batch 64 peak RSS 292.7 MB versus 343.8 MB, 14.9% lower; throughput 163,732 versus 163,721 chunks/s, unchanged. Commit 10K was neutral on repeat. Batch 256 used 438 MB; batch 512 used 611 MB. Writer 25 MB halved throughput; writer 200 MB regressed 22%. | Batch 64 is clean memory win. Channel, commit, and writer changes discarded. |
| 10 | Add ARM NEON exact scorer | Discard for now | Rust AArch64 target installed, but full cross-check stopped before crate compilation because `aarch64-linux-gnu-gcc` was unavailable. No ARM host existed for latency A/B. | Compile-only evidence would not satisfy performance decision. Prototype removed. |
| 11 | Overlap lexical and semantic retrieval lanes | Discard | 100K phase profile: lexical median 0.172 ms, semantic 0.004 ms, fusion 0.060 ms, presentation 0.055 ms, total 0.291 ms. Neural query embedding is already precomputed alongside lexical work. | Remaining semantic lane is too small for thread scheduling and synchronization overhead. |
| 12 | Lazily construct Tantivy `QueryParser` | Discard | Candidate Criterion: complex phrase improved 3.75% and bounded rerank improved 1.27%, but simple symbol regressed 1.47%. Paired 1M combined gate then observed warm-distinct p95 ratio 1.151. It was not statistically significant, but direction was inconsistent with a safe general win. | Double simple-query construction and workload-dependent results make optimization too uncertain. Restored eager parser. |
| 13 | Replace bounded-cache FIFO/clear-all behavior with true LRU | Discard | Final paired 1M gate found cache-replay p95 32.1% slower: ratio 1.321 with 95% bootstrap interval 1.237 to 1.371. Absolute median-run p95 rose from 0.116 ms to 0.152 ms. File/glob LRU also added O(n) touch work to presentation. | Recency scans cost more than FIFO misses at current bounds. All LRU changes removed. |
| 14 | Gate every million-benchmark query path | Keep | Comparator now checks process-cold, CLI warm, daemon warm, cache replay, filtered, and concurrent p95 plus recall. Focused test injects filtered-only regression while warm path stays flat; comparator rejects it. | Prior gate could pass while five measured paths regressed. |
| 15 | Run stratified routing matrix on PR relevance gate | Keep | Existing gate covered deterministic hash plus neural only. Workflow now evaluates hash, hybrid, automatic blended routing, and forced neural on 100 stratified public queries and applies floors to every mode. | Default-routing regressions become observable instead of hiding behind forced-neural smoke. |
| 16 | Auto-import benchmark evidence into dashboard | Discard as duplicate | CI and public-retrieval workflows already run `render_evidence_dashboard.py` and fail on generated JSON/HTML diff. | Freshness invariant already exists. More automation would duplicate current gate. |
| 17 | Add real CUDA smoke/benchmark | Keep operational gate | Same 388-chunk transformer-profile corpus: CUDA 2.55 s versus CPU 18.95 s, 7.43x faster. Peak RSS 430,044 KiB versus 2,413,056 KiB, 82.2% lower. Persisted backend was `BERT embedding via Candle CUDA`; all 388 vectors completed. | Real RTX 3060 execution validates build beyond feature compilation. Default static profile remains CPU-optimized as designed. |
| 18 | Add controlled external search baseline | Keep | DataFusion exact query, all matches: local ivygrep literal p50 159.0 ms versus ripgrep 35.2 ms; warm daemon ivygrep 124.5 ms versus ripgrep 35.8 ms. Bounded file output: daemon ivygrep 121.3 ms versus ripgrep 5.45 ms. Match counts are recorded because scopes can differ. | External control exposed a real process/open-context and bounded-literal gap. Added interleaved `ivygrep`/`rg`/`git grep` harness instead of making unsupported speed claims. |

## Combined candidate

Five fresh 1M baseline/candidate pairs alternated execution order. Combined candidate passed every query-path and recall gate:

| Measure | Candidate/baseline | 95% bootstrap interval | Result |
|---|---:|---:|---|
| Fresh-index throughput | 1.016 | 0.917 to 1.083 | No regression; median 1.62% faster |
| Peak RSS | 0.854 | n/a | 14.55% lower |
| Index size | 1.00007 | n/a | Operationally unchanged |
| Process-cold p95 | 0.997 | 0.845 to 1.261 | No regression |
| CLI warm-distinct p95 | 0.963 | 0.892 to 1.027 | No regression |
| Daemon warm-distinct p95 | 1.022 | 0.917 to 1.105 | No regression |
| Cache-replay p95 | 0.940 | 0.865 to 0.978 | 6.0% faster |
| Filtered p95 | 0.987 | 0.939 to 1.036 | No regression |
| Concurrent p95 | 0.946 | 0.715 to 1.346 | No regression |
| Expected recall@20 loss, every path | 0.000 | n/a | Identical |

Self-relevance baseline and candidate were identical across all 23 queries: nDCG@10 0.919794, MRR 0.949275, P@1 0.913043, recall@5 0.956522, and zero no-hit queries. Fresh 100-query public routing matrix also cleared every workflow floor:

| Mode | nDCG@10 | MRR@10 | Recall@20 | No-hit rate |
|---|---:|---:|---:|---:|
| Hash | 0.589979 | 0.536151 | 0.77 | 0.00 |
| Hybrid | 0.596680 | 0.542262 | 0.78 | 0.00 |
| Blended | 0.581083 | 0.529996 | 0.78 | 0.00 |
| Neural | 0.588973 | 0.537663 | 0.79 | 0.00 |

Controlled external harness smoke used DataFusion commit `35f7501ba34ccfa0fe4b3d6b830f27cbac2b283d` and 10 interleaved samples per tool. Median latency was 164.8 ms for local ivygrep literal, 37.2 ms for ripgrep, and 29.3 ms for git grep. Recorded match counts were 323, 322, and 322 respectively, so result correctly exposes scope mismatch rather than asserting equivalent output.

## Retained source changes

- Fresh index counts use already-observed counters; incremental counts remain transaction-derived.
- Index producer batch falls from 128 files to 64.
- Exact/path queries request hash enhancement only. Neural-routed queries and existing neural stores keep neural enhancement behavior.
- Million-scale comparator gates every emitted query path.
- PR relevance smoke covers all four retrieval modes.
- `scripts/bench_external_search.py` records interleaved exact-search baselines, versions, binary digest, repo commit, match counts, and latency distributions.

## Removed experiments

- All-hash automatic router.
- Multi-row SQLite chunk insert.
- Dot-only exact scoring.
- Maximum first-pass filtered ANN overfetch.
- Index channel/commit/writer-memory changes.
- Search-context file/glob LRU.
- Daemon query-result/neural-query LRU.
- Lazy Tantivy parser construction.
- ARM NEON scorer without ARM performance evidence.

## Acceptance gates

Passed:

- `cargo fmt --check` and `git diff --check`.
- Locked all-target check and strict all-target clippy.
- Strict accelerate/metal clippy.
- Locked default all-target Rust tests and no-default-feature library/binary/integration tests.
- 142 Python harness tests.
- Web typecheck, three Vitest tests, production build, and committed-dist parity.
- End-to-end CLI procedures.
- Self-relevance parity and four-mode public retrieval floors.
- Five-pair 1M comparison across all six query paths with zero recall loss.
- Exact/natural enhancement routing smoke.
- Interleaved external harness smoke.
- Generated evidence-dashboard parity.

Release action is outside this ablation report.
