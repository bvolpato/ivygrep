# ivygrep vs Semble

Generated: 2026-06-23T18:11:57.173114+00:00

Semble: `41c36f789c007171a1c5d5638f7f4f88573ee9ce` (0.4.1)
ivygrep: `c973615e3d67e2872bf1aa66b969c81cccfa95c6`

| Metric | ivygrep | Semble | Winner |
|---|---:|---:|---|
| nDCG@10 | 0.688 | 0.801 | Semble |
| Warm query p50 | 16.17 ms | 4.90 ms | Semble |
| Warm query p95 | 29.64 ms | 21.15 ms | Semble |
| Mean returned tokens | 252 | 1593 | ivygrep |

## Quality by query type

| Category | ivygrep nDCG@10 | Semble nDCG@10 |
|---|---:|---:|
| Architecture | 0.544 | 0.745 |
| Semantic | 0.687 | 0.770 |
| Symbol | 0.914 | 0.949 |

## Initial indexing

Full hybrid-ready time includes ivygrep lexical, hash, and neural phases.

| Repository | ivygrep | Semble | Semble / ivygrep |
|---|---:|---:|---:|
| axum | 751 ms | 1557 ms | 2.07x |
| fastapi | 489 ms | 1628 ms | 3.33x |
| trpc | 622 ms | 1161 ms | 1.87x |

## One-file refresh

| Metric | ivygrep | Semble |
|---|---:|---:|
| Searchable lexical refresh | 65.38 ms | n/a |
| Full hybrid refresh | 279.08 ms | 830.66 ms |

## Verdict

- Semble leads overall retrieval quality by 0.112 nDCG@10 and warm p50 latency by 3.3x.
- ivygrep returns 6.3x fewer tokens in top-10 results.
- ivygrep full one-file refresh is 3.0x faster and exposes lexical changes before neural refresh completes.
- ivygrep hybrid-ready indexing is faster on every benchmark repository in this run.
- Largest remaining quality gap is architecture retrieval. Exact symbol quality is much closer.

## Notes

- Same pinned repositories, queries, labels, top-k, and nDCG implementation as Semble.
- Semble runs in-process, matching its official benchmark.
- ivygrep runs through its persistent daemon protocol, excluding CLI process startup.
- Model load is reported separately from per-repository indexing.
- ANN construction can move a small number of semantic ranks between runs; compare repeated builds before treating small deltas as signal.
