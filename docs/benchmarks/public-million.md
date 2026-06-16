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
  109006 chunks/s
  (+2096.2%)
- Wall time: 203.5 ->
  9.3 s
- Filesystem writes: 23.96 GiB
  -> 0.00 GiB
  (-100.0%)
- Peak RSS: 0.46 GiB ->
  0.28 GiB
  (-39.3%)
- Peak disk: 1.34 GiB ->
  0.46 GiB
  (-65.7%)
- Final index size: 1.06 GiB ->
  0.46 GiB
  (-57.0%)
- Current tiers: stored chunks 0.23 GiB,
  graph 0.07 GiB, SQLite auxiliary
  0.07 GiB, lexical
  0.09 GiB, hash vectors
  0.00 GiB, neural vectors
  0.00 GiB

The indexing target did not reach 2x. The measured ceiling is storage and
scheduler bound: producer-side compression and checkpoint changes improved the
controlled run by 21.96x and reduced
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

| Metric | baseline mean +/- sd | current mean +/- sd | delta |
| --- | ---: | ---: | ---: |
| ndcg_at_10 | 0.2620 +/- 0.0002 | 0.2666 +/- 0.0016 | +0.0046 |
| mrr_at_10 | 0.2178 +/- 0.0004 | 0.2220 +/- 0.0014 | +0.0042 |
| precision_at_5 | 0.0561 +/- 0.0002 | 0.0601 +/- 0.0015 | +0.0039 |
| recall_at_20 | 0.5080 +/- 0.0024 | 0.4890 +/- 0.0008 | -0.0190 |
| no_hit_rate | 0.0000 +/- 0.0000 | 0.0000 +/- 0.0000 | +0.0000 |

### Per-dataset nDCG@10

| Dataset | baseline mean +/- sd | current mean +/- sd | delta |
| --- | ---: | ---: | ---: |
| codetrans-dl | 0.2365 +/- 0.0005 | 0.2402 +/- 0.0033 | +0.0037 |
| codetrans-contest | 0.4269 +/- 0.0000 | 0.4278 +/- 0.0001 | +0.0009 |
| cosqa | 0.1464 +/- 0.0003 | 0.1566 +/- 0.0041 | +0.0102 |
| codefeedback-st | 0.5243 +/- 0.0000 | 0.5108 +/- 0.0000 | -0.0135 |

Raw evidence is published beside this report. CI runs repeated paired base/head
trials on the same runner, bootstraps p95 and median indexing throughput, and
rejects statistically significant regressions.
