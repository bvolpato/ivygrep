# Public million-chunk benchmark

This report uses a deterministic CC0 Rust corpus with
1,000,000 generated chunks. It separates paired
query latency, controlled indexing, a saturated full-system run, and pinned
public retrieval quality.

## Acceptance

- Gate: **PASS**
- Interleaved warm-distinct p95: 53.77 ms ->
  15.07 ms (3.57x faster)
- Bootstrap p95 ratio: 0.280
  (95% CI 0.247 to
  0.318)
- Expected recall@20: 1.000
- Public quality: 1000 queries across
  4 tasks

The paired run alternated request order evenly while both daemons were live.
The host load average was 91.9/81.8/92.7 on
32 logical CPUs, so the absolute latency is not
a dedicated-host claim.

## Controlled indexing

- Throughput: 4963 ->
  6231 chunks/s
  (+25.5%)
- Wall time: 203.5 ->
  162.1 s
- Filesystem writes: 23.96 GiB
  -> 18.90 GiB
  (-21.1%)
- Peak RSS: 0.46 GiB ->
  0.42 GiB
  (-7.3%)
- Final index size: 1.07 GiB

The indexing target did not reach 2x. The measured ceiling is storage and
scheduler bound: producer-side compression and checkpoint changes improved the
controlled run by 1.26x and reduced
writes, while the exact full run had nearly identical process CPU time but
materially different host load and wall time.

## Full-system query paths

| Path | baseline p95 ms | current p95 ms | ratio |
| --- | ---: | ---: | ---: |
| Process cold | 307.22 | 478.02 | 1.556 |
| Warm distinct | 112.57 | 58.29 | 0.518 |
| Cache replay | 27.85 | 31.96 | 1.148 |
| Filtered | 257.31 | 45.22 | 0.176 |
| Warm CLI | 151.27 | 138.99 | 0.919 |
| Concurrent | 164.74 | 99.17 | 0.602 |

These paths come from the same exact full-run artifacts and are reported
separately: process cold, warm distinct, replay, filtered, CLI, and concurrent.

## Public retrieval quality

| Metric | baseline | current | delta |
| --- | ---: | ---: | ---: |
| ndcg_at_10 | 0.2620 | 0.2632 | +0.0012 |
| mrr_at_10 | 0.2178 | 0.2192 | +0.0014 |
| precision_at_5 | 0.0561 | 0.0598 | +0.0037 |
| recall_at_20 | 0.5080 | 0.4887 | -0.0193 |
| no_hit_rate | 0.0000 | 0.0000 | +0.0000 |

Raw evidence is published beside this report. CI runs repeated paired base/head
trials on the same runner, bootstraps p95 and median indexing throughput, and
rejects statistically significant regressions.
