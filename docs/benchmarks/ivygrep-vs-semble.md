# ivygrep vs Semble

Generated: 2026-06-25T02:56:53.971549+00:00

Semble: `41c36f789c007171a1c5d5638f7f4f88573ee9ce` (0.4.1)
ivygrep: `20c3d3295bc47a09e81ef32ba234917ed241e0c6` + dirty worktree

| Metric | ivygrep | Semble | Winner |
|---|---:|---:|---|
| nDCG@10 | 0.813 | 0.801 | ivygrep |
| Warm query p50 | 8.98 ms | 4.99 ms | Semble |
| Warm query p95 | 11.70 ms | 21.60 ms | ivygrep |
| Mean returned tokens | 393 | 1593 | ivygrep |

## Quality by query type

| Category | ivygrep nDCG@10 | Semble nDCG@10 |
|---|---:|---:|
| Architecture | 0.751 | 0.745 |
| Semantic | 0.769 | 0.770 |
| Symbol | 1.000 | 0.949 |

## Initial indexing

Full hybrid-ready time includes ivygrep lexical, hash, and neural phases.

| Repository | ivygrep | Semble | Semble / ivygrep |
|---|---:|---:|---:|
| axum | 740 ms | 851 ms | 1.15x |
| fastapi | 485 ms | 1072 ms | 2.21x |
| trpc | 612 ms | 575 ms | 0.94x |

## One-file refresh

| Metric | ivygrep | Semble |
|---|---:|---:|
| Searchable lexical refresh | 63.80 ms | n/a |
| Full hybrid refresh | 273.83 ms | 823.67 ms |

## Verdict

- ivygrep leads overall retrieval quality by 0.012 nDCG@10; Semble leads warm p50 latency by 1.8x.
- ivygrep returns 4.1x fewer tokens in top-10 results.
- ivygrep full one-file refresh is 3.0x faster and exposes lexical changes before neural refresh completes.
- Initial indexing is mixed: ivygrep leads on axum, fastapi; Semble leads on trpc in this run.
- Largest remaining quality gap is semantic retrieval. Exact semantic quality is much closer.

## Notes

- Same pinned repositories, queries, labels, top-k, and nDCG implementation as Semble.
- Semble runs in-process, matching its official benchmark.
- ivygrep runs through its persistent daemon protocol, excluding CLI process startup.
- Timed ivygrep queries disable daemon result-cache replay.
- Model load is reported separately from per-repository indexing.
- ANN construction can move a small number of semantic ranks between runs; compare repeated builds before treating small deltas as signal.
