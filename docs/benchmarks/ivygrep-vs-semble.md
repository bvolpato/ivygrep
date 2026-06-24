# ivygrep vs Semble

Generated: 2026-06-24T20:21:50.031783+00:00

Semble: `41c36f789c007171a1c5d5638f7f4f88573ee9ce` (0.4.1)
ivygrep: `22a57cd3a48cc233afad719daf8c5c29caf975b1`

| Metric | ivygrep | Semble | Winner |
|---|---:|---:|---|
| nDCG@10 | 0.813 | 0.801 | ivygrep |
| Warm query p50 | 9.09 ms | 4.94 ms | Semble |
| Warm query p95 | 11.93 ms | 21.43 ms | ivygrep |
| Mean returned tokens | 392 | 1593 | ivygrep |

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
| axum | 737 ms | 852 ms | 1.16x |
| fastapi | 495 ms | 1069 ms | 2.16x |
| trpc | 620 ms | 568 ms | 0.92x |

## One-file refresh

| Metric | ivygrep | Semble |
|---|---:|---:|
| Searchable lexical refresh | 65.19 ms | n/a |
| Full hybrid refresh | 276.20 ms | 819.44 ms |

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
