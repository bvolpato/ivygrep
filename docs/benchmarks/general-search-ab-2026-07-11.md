# General search A/B study, 2026-07-11

This study tested five search optimizations independently against clean v1.1.13
commit `977418b`. Criterion used a Ryzen 9 3950X with 32 logical CPUs, 20 flat
samples, five-second warmup, 15-second measurement, and 95% confidence
intervals. Values below are Criterion time point estimates. Ranking-sensitive
changes must also pass deterministic relevance and output-equivalence gates.

## Baseline

| Search path | v1.1.13 time, 95% CI |
|---|---:|
| 200-file hybrid | 4.751 ms, 4.662 to 4.845 ms |
| 1K-file simple symbol | 3.131 ms, 3.080 to 3.184 ms |
| 1K-file complex phrase | 3.340 ms, 3.300 to 3.381 ms |
| 1K-file bounded rerank | 3.538 ms, 3.460 to 3.620 ms |

The symbolized complex-search profile attributed 10.59% flat CPU to Tantivy
buffered-union refill, 3.37% to substring containment, 2.59% to `memcmp`,
2.16% to allocator internals, 1.72% to `memmove`, and 1.00% to SQLite. No
single hot function could produce a general 2x improvement by itself.
After retained parallel lexical execution, the two buffered-union refill symbols
accounted for 8.13% flat CPU, down from 10.59% in the baseline profile. Rayon
work stealing appeared at 1.87% combined, consistent with trading bounded
scheduler work for lower wall latency.

## Independent decisions

| Experiment | Representative A/B result | Decision |
|---|---|---|
| Fuse semantic metadata and text hydration | Complex +0.30%; simple -1.91%; all intervals crossed zero | Discard |
| Remove one-child literal Boolean union | Complex -1.24%, repeat -0.26%; rerank +1.35%; all nonsignificant | Discard |
| Replace semantic source sets with bit masks | Complex +0.03%; rerank +3.60%, `p=0.02` | Discard |
| Cache/precompute neural query vectors | Warm option-changing daemon query 0.790 to 0.399 ms median, 1.98x; embedding 64 to 126 µs down to 0.2 to 0.6 µs | Keep |
| Run independent lexical expansions in parallel | Final repeat: simple -13.03%; complex -4.10%; 200-file +1.27% nonsignificant | Keep |

### 1. Fused hydration

The prototype replaced semantic metadata-only hydration plus bounded rerank
text hydration with one full-row fetch. It read text for candidates that never
reached reranking. Point changes were -1.14% for 200-file hybrid, -1.91% for
simple symbol, +0.30% for complex phrase, and -1.00% for bounded rerank. Every
95% interval crossed zero. Extra I/O without reliable latency reduction does
not justify retention.

### 2. Tantivy Boolean simplification

The prototype returned a sole literal Boolean child directly, removing one
union wrapper without changing query meaning. Complex phrase moved -1.24% on
the first run and -0.26% on repeat. The 200-file path moved -2.50% then -2.08%,
but both intervals crossed zero. Simple symbol moved +0.40%; bounded rerank
moved +1.35%. This did not remove the measured union hotspot broadly enough.

### 3. Fusion allocation reduction

The prototype replaced one heap-backed semantic source `HashSet` per candidate
with the existing 16-bit source mask. It removed up to 50 small allocations per
default query. Complex phrase stayed flat at +0.03%, simple symbol moved
-0.45%, and 200-file hybrid moved -2.68% without significance. Bounded rerank
regressed +3.60% with `p=0.02`. Lower allocation count was not lower latency.

### 4. Neural query-vector cache and precompute

The daemon already caches complete results, so this test reused identical query
text while changing `--limit` on every request. Complete-result cache keys
therefore missed. Both sides used the real default 256-dimensional static
retrieval model and same neural index. Model construction plus existing warmup
stayed near 150 ms on first forced-neural request.

Across two reverse-order v1.1.13 runs, warm daemon medians were 0.801 and
0.780 ms. The prototype median was 0.399 ms. Cached neural embedding fell from
64 to 126 µs in the reverse baseline to 0.2 to 0.6 µs. Cache size is bounded to
128 query vectors, about 128 KiB for the default 256-dimensional profile.
Scores and source labels use the exact cached vector bytes.

### 5. Parallel lexical expansions

The prototype runs independent lexical expansion queries and Tantivy document
loads on Rayon workers, then filters candidates and performs max-score
deduplication sequentially. This preserves candidate scores and deterministic
tie handling.

| Search path | v1.1.13 | Retained | Change, 95% CI |
|---|---:|---:|---:|
| 1K-file simple symbol | 3.131 ms | 2.723 ms | -13.03%, -14.95% to -11.14% |
| 1K-file complex phrase | 3.340 ms | 3.203 ms | -4.10%, -5.61% to -2.62% |
| 1K-file bounded rerank | 3.538 ms | 3.554 ms | +0.46%, -2.27% to +3.12% |
| 200-file hybrid | 4.751 ms | 4.871 ms | +1.27%, -1.26% to +3.76% |

A forced one-thread run initially exposed Rayon overhead. A one-thread
sequential fallback removed it: simple symbol measured +0.58% and complex
phrase -1.37%, both nonsignificant. Final multi-core repeats retained -13.03%
and -4.10% improvements.

The first final-artifact bounded-rerank run reported +13.77% with 20% outliers.
The required reverse-order confirmation measured +0.46% with a confidence
interval crossing zero. The confirmed result, not the anomalous sample, is
reported above.

## Reproduction

```bash
cargo bench --locked --no-default-features --bench indexer_bench --no-run
target/release/deps/indexer_bench-* \
  'hybrid_complex_phrase_1000_files' --bench \
  --baseline v113-general-base --noplot
RAYON_NUM_THREADS=1 target/release/deps/indexer_bench-* \
  'hybrid_simple_symbol_1000_files' --bench \
  --baseline v113-general-base --noplot
```

Raw structured results are in
[`general-search-ab-2026-07-11.json`](general-search-ab-2026-07-11.json).
