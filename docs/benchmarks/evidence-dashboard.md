# Evidence dashboard

This page is generated from retained machine-readable benchmark and release
artifacts. Every evidence link is pinned to the commit that published its bytes.

| Area | Latest retained result |
|---|---|
| Public neural retrieval | nDCG@10 0.2666, MRR@10 0.2220, 1000 queries x 3 runs |
| Learned reranker | gate passed, nDCG@10 delta 0.0192 |
| Million-chunk latency | 15.07 ms warm p95, 3.57x baseline |
| Million-chunk footprint | 491589780 bytes, ratio 0.430 |
| Daemon cache | 4.90 ms retained warm p95 |
| Release archive history | v1.0.2 with 5 archives |

## Versioned histories

| Metric family | Retained points | Unavailable points |
|---|---:|---:|
| quality | 2 | 0 |
| latency | 2 | 0 |
| indexing | 2 | 0 |
| memory | 2 | 0 |
| index size | 2 | 0 |
| binary size | 449 | 429 |
| archive size | 449 | 0 |

Each point in `evidence-dashboard.json` includes its unit, comparison series,
hardware/corpus/model context, variance or an explicit variance-unavailable
reason, source commit, and immutable artifact URL.

| Family | Comparable series | Revision/tag | Value | Variance | Artifact |
|---|---|---|---:|---|---|
| quality | semantic-retrieval/neural/ndcg_at_10 | 2c735847d43e | 0.2620 | sd 0.0002 | [source](https://github.com/bvolpato/ivygrep/blob/107e3b0726cf15dc1d4d45178aad3f732168a793/docs/benchmarks/public-million-results.json) |
| quality | semantic-retrieval/neural/ndcg_at_10 | 2c7629e18d40 | 0.2666 | sd 0.0016 | [source](https://github.com/bvolpato/ivygrep/blob/107e3b0726cf15dc1d4d45178aad3f732168a793/docs/benchmarks/public-million-results.json) |
| latency | million-chunk/warm-distinct-p95 | 4b24c627d9bd | 53.77 ms | ratio CI95 0.247-0.318 | [source](https://github.com/bvolpato/ivygrep/blob/107e3b0726cf15dc1d4d45178aad3f732168a793/docs/benchmarks/public-million-results.json) |
| latency | million-chunk/warm-distinct-p95 | c1442ebb68f5 | 15.07 ms | ratio CI95 0.247-0.318 | [source](https://github.com/bvolpato/ivygrep/blob/107e3b0726cf15dc1d4d45178aad3f732168a793/docs/benchmarks/public-million-results.json) |
| indexing | million-chunk/chunks-per-second | 4b24c627d9bd | 4963.32 chunks/s | unavailable | [source](https://github.com/bvolpato/ivygrep/blob/107e3b0726cf15dc1d4d45178aad3f732168a793/docs/benchmarks/public-million-results.json) |
| indexing | million-chunk/chunks-per-second | 2c7629e18d40 | 109005.50 chunks/s | unavailable | [source](https://github.com/bvolpato/ivygrep/blob/107e3b0726cf15dc1d4d45178aad3f732168a793/docs/benchmarks/public-million-results.json) |
| memory | million-chunk/peak-rss | 4b24c627d9bd | 468.74 MiB | unavailable | [source](https://github.com/bvolpato/ivygrep/blob/107e3b0726cf15dc1d4d45178aad3f732168a793/docs/benchmarks/public-million-results.json) |
| memory | million-chunk/peak-rss | 2c7629e18d40 | 284.58 MiB | unavailable | [source](https://github.com/bvolpato/ivygrep/blob/107e3b0726cf15dc1d4d45178aad3f732168a793/docs/benchmarks/public-million-results.json) |
| index size | million-chunk/final-index | 4b24c627d9bd | 1090.48 MiB | unavailable | [source](https://github.com/bvolpato/ivygrep/blob/107e3b0726cf15dc1d4d45178aad3f732168a793/docs/benchmarks/public-million-results.json) |
| index size | million-chunk/final-index | 2c7629e18d40 | 468.82 MiB | unavailable | [source](https://github.com/bvolpato/ivygrep/blob/107e3b0726cf15dc1d4d45178aad3f732168a793/docs/benchmarks/public-million-results.json) |
| binary size | release/linux-aarch64-musl | v1.0.2 | 67.15 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.2/ivygrep-v1.0.2-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v1.0.2 | 70.59 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.2/ivygrep-v1.0.2-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v1.0.2 | 67.65 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.2/ivygrep-v1.0.2-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v1.0.2 | 68.14 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.2/ivygrep-v1.0.2-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v1.0.2 | 69.89 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.2/ivygrep-v1.0.2-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v1.0.1 | 67.22 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.1/ivygrep-v1.0.1-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v1.0.1 | 70.68 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.1/ivygrep-v1.0.1-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v1.0.1 | 67.71 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.1/ivygrep-v1.0.1-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v1.0.1 | 68.16 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.1/ivygrep-v1.0.1-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v1.0.1 | 69.95 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.1/ivygrep-v1.0.1-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v1.0.0 | 67.22 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.0/ivygrep-v1.0.0-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v1.0.0 | 70.68 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.0/ivygrep-v1.0.0-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v1.0.0 | 67.70 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.0/ivygrep-v1.0.0-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v1.0.0 | 68.15 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.0/ivygrep-v1.0.0-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v1.0.0 | 69.95 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.0/ivygrep-v1.0.0-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.21 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.21/ivygrep-v0.12.21-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.21 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.21/ivygrep-v0.12.21-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.21 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.21/ivygrep-v0.12.21-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.21 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.21/ivygrep-v0.12.21-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.21 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.21/ivygrep-v0.12.21-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.20 | 67.21 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.20/ivygrep-v0.12.20-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.20 | 70.66 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.20/ivygrep-v0.12.20-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.20 | 67.67 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.20/ivygrep-v0.12.20-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.20 | 68.16 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.20/ivygrep-v0.12.20-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.20 | 69.94 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.20/ivygrep-v0.12.20-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.19 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.19/ivygrep-v0.12.19-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.19 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.19/ivygrep-v0.12.19-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.19 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.19/ivygrep-v0.12.19-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.19 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.19/ivygrep-v0.12.19-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.19 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.19/ivygrep-v0.12.19-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.18 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.18/ivygrep-v0.12.18-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.18 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.18/ivygrep-v0.12.18-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.18 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.18/ivygrep-v0.12.18-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.18 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.18/ivygrep-v0.12.18-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.18 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.18/ivygrep-v0.12.18-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.17 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.17/ivygrep-v0.12.17-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.17 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.17/ivygrep-v0.12.17-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.17 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.17/ivygrep-v0.12.17-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.17 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.17/ivygrep-v0.12.17-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.17 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.17/ivygrep-v0.12.17-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.16 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.16/ivygrep-v0.12.16-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.16 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.16/ivygrep-v0.12.16-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.16 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.16/ivygrep-v0.12.16-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.16 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.16/ivygrep-v0.12.16-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.16 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.16/ivygrep-v0.12.16-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.15 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.15/ivygrep-v0.12.15-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.15 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.15/ivygrep-v0.12.15-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.15 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.15/ivygrep-v0.12.15-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.15 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.15/ivygrep-v0.12.15-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.15 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.15/ivygrep-v0.12.15-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.14 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.14/ivygrep-v0.12.14-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.14 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.14/ivygrep-v0.12.14-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.14 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.14/ivygrep-v0.12.14-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.14 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.14/ivygrep-v0.12.14-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.14 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.14/ivygrep-v0.12.14-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.13 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.13/ivygrep-v0.12.13-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.13 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.13/ivygrep-v0.12.13-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.13 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.13/ivygrep-v0.12.13-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.13 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.13/ivygrep-v0.12.13-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.13 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.13/ivygrep-v0.12.13-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.12 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.12/ivygrep-v0.12.12-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.12 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.12/ivygrep-v0.12.12-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.12 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.12/ivygrep-v0.12.12-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.12 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.12/ivygrep-v0.12.12-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.12 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.12/ivygrep-v0.12.12-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.11 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.11/ivygrep-v0.12.11-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.11 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.11/ivygrep-v0.12.11-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.11 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.11/ivygrep-v0.12.11-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.11 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.11/ivygrep-v0.12.11-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.11 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.11/ivygrep-v0.12.11-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.10 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.10/ivygrep-v0.12.10-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.10 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.10/ivygrep-v0.12.10-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.10 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.10/ivygrep-v0.12.10-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.10 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.10/ivygrep-v0.12.10-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.10 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.10/ivygrep-v0.12.10-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.9 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.9/ivygrep-v0.12.9-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.9 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.9/ivygrep-v0.12.9-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.9 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.9/ivygrep-v0.12.9-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.9 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.9/ivygrep-v0.12.9-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.9 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.9/ivygrep-v0.12.9-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.8 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.8/ivygrep-v0.12.8-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.8 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.8/ivygrep-v0.12.8-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.8 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.8/ivygrep-v0.12.8-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.8 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.8/ivygrep-v0.12.8-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.8 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.8/ivygrep-v0.12.8-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.7/ivygrep-v0.12.7-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.7/ivygrep-v0.12.7-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.7/ivygrep-v0.12.7-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.7/ivygrep-v0.12.7-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.7/ivygrep-v0.12.7-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.6/ivygrep-v0.12.6-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.6/ivygrep-v0.12.6-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.6/ivygrep-v0.12.6-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.6/ivygrep-v0.12.6-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.6/ivygrep-v0.12.6-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.5/ivygrep-v0.12.5-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.5/ivygrep-v0.12.5-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.5/ivygrep-v0.12.5-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.5/ivygrep-v0.12.5-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.5/ivygrep-v0.12.5-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.4/ivygrep-v0.12.4-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.4/ivygrep-v0.12.4-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.4/ivygrep-v0.12.4-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.4/ivygrep-v0.12.4-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.4/ivygrep-v0.12.4-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.3/ivygrep-v0.12.3-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.3/ivygrep-v0.12.3-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.3/ivygrep-v0.12.3-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.3/ivygrep-v0.12.3-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.3/ivygrep-v0.12.3-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.2/ivygrep-v0.12.2-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.2/ivygrep-v0.12.2-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.2/ivygrep-v0.12.2-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.2/ivygrep-v0.12.2-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.2/ivygrep-v0.12.2-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.1/ivygrep-v0.12.1-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.1/ivygrep-v0.12.1-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.1/ivygrep-v0.12.1-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.1/ivygrep-v0.12.1-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.1/ivygrep-v0.12.1-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.12.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.0/ivygrep-v0.12.0-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.12.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.0/ivygrep-v0.12.0-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.12.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.0/ivygrep-v0.12.0-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.12.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.0/ivygrep-v0.12.0-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.12.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.0/ivygrep-v0.12.0-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.11.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.2/ivygrep-v0.11.2-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.11.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.2/ivygrep-v0.11.2-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.11.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.2/ivygrep-v0.11.2-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.11.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.2/ivygrep-v0.11.2-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.11.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.2/ivygrep-v0.11.2-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.11.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.0/ivygrep-v0.11.0-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.11.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.0/ivygrep-v0.11.0-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.11.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.0/ivygrep-v0.11.0-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.11.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.0/ivygrep-v0.11.0-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.11.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.0/ivygrep-v0.11.0-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.10.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.1/ivygrep-v0.10.1-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.10.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.1/ivygrep-v0.10.1-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.10.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.1/ivygrep-v0.10.1-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.10.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.1/ivygrep-v0.10.1-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.10.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.1/ivygrep-v0.10.1-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.10.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.0/ivygrep-v0.10.0-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.10.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.0/ivygrep-v0.10.0-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.10.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.0/ivygrep-v0.10.0-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.10.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.0/ivygrep-v0.10.0-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.10.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.0/ivygrep-v0.10.0-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.9.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.7/ivygrep-v0.9.7-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.9.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.7/ivygrep-v0.9.7-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.9.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.7/ivygrep-v0.9.7-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.9.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.7/ivygrep-v0.9.7-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.9.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.6/ivygrep-v0.9.6-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.9.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.6/ivygrep-v0.9.6-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.9.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.6/ivygrep-v0.9.6-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.9.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.6/ivygrep-v0.9.6-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.9.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.5/ivygrep-v0.9.5-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.9.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.5/ivygrep-v0.9.5-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.9.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.5/ivygrep-v0.9.5-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.9.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.5/ivygrep-v0.9.5-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.9.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.4/ivygrep-v0.9.4-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.9.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.4/ivygrep-v0.9.4-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.9.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.4/ivygrep-v0.9.4-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.9.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.4/ivygrep-v0.9.4-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.9.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.3/ivygrep-v0.9.3-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.9.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.3/ivygrep-v0.9.3-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.9.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.3/ivygrep-v0.9.3-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.9.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.3/ivygrep-v0.9.3-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.9.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.2/ivygrep-v0.9.2-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.9.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.2/ivygrep-v0.9.2-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.9.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.2/ivygrep-v0.9.2-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.9.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.2/ivygrep-v0.9.2-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.9.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.1/ivygrep-v0.9.1-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.9.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.1/ivygrep-v0.9.1-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.9.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.1/ivygrep-v0.9.1-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.9.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.1/ivygrep-v0.9.1-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.9.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.0/ivygrep-v0.9.0-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.9.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.0/ivygrep-v0.9.0-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.9.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.0/ivygrep-v0.9.0-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.9.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.0/ivygrep-v0.9.0-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.8.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.1/ivygrep-v0.8.1-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.8.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.1/ivygrep-v0.8.1-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.8.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.1/ivygrep-v0.8.1-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.8.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.1/ivygrep-v0.8.1-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.8.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.0/ivygrep-v0.8.0-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.8.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.0/ivygrep-v0.8.0-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.8.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.0/ivygrep-v0.8.0-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.8.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.0/ivygrep-v0.8.0-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.7.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.7.0/ivygrep-v0.7.0-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.7.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.7.0/ivygrep-v0.7.0-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.7.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.7.0/ivygrep-v0.7.0-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.7.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.7.0/ivygrep-v0.7.0-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.20 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.20/ivygrep-v0.6.20-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.20 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.20/ivygrep-v0.6.20-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.20 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.20/ivygrep-v0.6.20-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.20 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.20/ivygrep-v0.6.20-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.19 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.19/ivygrep-v0.6.19-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.19 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.19/ivygrep-v0.6.19-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.19 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.19/ivygrep-v0.6.19-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.19 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.19/ivygrep-v0.6.19-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.18 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.18/ivygrep-v0.6.18-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.18 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.18/ivygrep-v0.6.18-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.18 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.18/ivygrep-v0.6.18-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.18 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.18/ivygrep-v0.6.18-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.17 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.17/ivygrep-v0.6.17-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.17 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.17/ivygrep-v0.6.17-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.17 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.17/ivygrep-v0.6.17-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.17 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.17/ivygrep-v0.6.17-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.16 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.16/ivygrep-v0.6.16-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.16 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.16/ivygrep-v0.6.16-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.16 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.16/ivygrep-v0.6.16-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.16 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.16/ivygrep-v0.6.16-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.15 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.15/ivygrep-v0.6.15-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.15 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.15/ivygrep-v0.6.15-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.15 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.15/ivygrep-v0.6.15-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.15 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.15/ivygrep-v0.6.15-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.14 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.14/ivygrep-v0.6.14-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.14 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.14/ivygrep-v0.6.14-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.14 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.14/ivygrep-v0.6.14-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.14 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.14/ivygrep-v0.6.14-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.13 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.13/ivygrep-v0.6.13-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.13 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.13/ivygrep-v0.6.13-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.13 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.13/ivygrep-v0.6.13-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.13 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.13/ivygrep-v0.6.13-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.12 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.12/ivygrep-v0.6.12-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.12 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.12/ivygrep-v0.6.12-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.12 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.12/ivygrep-v0.6.12-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.12 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.12/ivygrep-v0.6.12-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.11 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.11/ivygrep-v0.6.11-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.11 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.11/ivygrep-v0.6.11-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.11 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.11/ivygrep-v0.6.11-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.11 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.11/ivygrep-v0.6.11-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.10 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.10/ivygrep-v0.6.10-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.10 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.10/ivygrep-v0.6.10-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.10 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.10/ivygrep-v0.6.10-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.10 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.10/ivygrep-v0.6.10-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.9 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.9/ivygrep-v0.6.9-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.9 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.9/ivygrep-v0.6.9-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.9 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.9/ivygrep-v0.6.9-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.9 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.9/ivygrep-v0.6.9-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.8 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.8/ivygrep-v0.6.8-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.8 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.8/ivygrep-v0.6.8-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.8 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.8/ivygrep-v0.6.8-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.8 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.8/ivygrep-v0.6.8-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.7/ivygrep-v0.6.7-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.7/ivygrep-v0.6.7-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.7/ivygrep-v0.6.7-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.7/ivygrep-v0.6.7-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.6/ivygrep-v0.6.6-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.6/ivygrep-v0.6.6-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.6/ivygrep-v0.6.6-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.6/ivygrep-v0.6.6-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.5/ivygrep-v0.6.5-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.5/ivygrep-v0.6.5-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.5/ivygrep-v0.6.5-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.5/ivygrep-v0.6.5-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.4/ivygrep-v0.6.4-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.4/ivygrep-v0.6.4-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.4/ivygrep-v0.6.4-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.4/ivygrep-v0.6.4-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.3/ivygrep-v0.6.3-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.3/ivygrep-v0.6.3-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.3/ivygrep-v0.6.3-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.3/ivygrep-v0.6.3-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.2/ivygrep-v0.6.2-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.2/ivygrep-v0.6.2-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.2/ivygrep-v0.6.2-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.2/ivygrep-v0.6.2-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.1/ivygrep-v0.6.1-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.1/ivygrep-v0.6.1-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.1/ivygrep-v0.6.1-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.1 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.1/ivygrep-v0.6.1-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.6.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.0/ivygrep-v0.6.0-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.6.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.0/ivygrep-v0.6.0-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.6.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.0/ivygrep-v0.6.0-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.6.0 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.0/ivygrep-v0.6.0-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.56 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.56/ivygrep-v0.5.56-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.56 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.56/ivygrep-v0.5.56-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.56 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.56/ivygrep-v0.5.56-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.56 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.56/ivygrep-v0.5.56-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.55 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.55/ivygrep-v0.5.55-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.55 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.55/ivygrep-v0.5.55-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.55 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.55/ivygrep-v0.5.55-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.55 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.55/ivygrep-v0.5.55-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.54 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.54/ivygrep-v0.5.54-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.54 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.54/ivygrep-v0.5.54-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.54 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.54/ivygrep-v0.5.54-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.54 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.54/ivygrep-v0.5.54-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.53 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.53/ivygrep-v0.5.53-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.53 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.53/ivygrep-v0.5.53-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.53 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.53/ivygrep-v0.5.53-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.53 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.53/ivygrep-v0.5.53-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.52 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.52/ivygrep-v0.5.52-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.52 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.52/ivygrep-v0.5.52-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.52 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.52/ivygrep-v0.5.52-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.52 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.52/ivygrep-v0.5.52-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.51 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.51/ivygrep-v0.5.51-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.51 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.51/ivygrep-v0.5.51-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.51 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.51/ivygrep-v0.5.51-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.51 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.51/ivygrep-v0.5.51-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.50 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.50/ivygrep-v0.5.50-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.50 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.50/ivygrep-v0.5.50-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.50 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.50/ivygrep-v0.5.50-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.50 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.50/ivygrep-v0.5.50-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.49 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.49/ivygrep-v0.5.49-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.49 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.49/ivygrep-v0.5.49-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.49 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.49/ivygrep-v0.5.49-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.49 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.49/ivygrep-v0.5.49-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.48 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.48/ivygrep-v0.5.48-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.48 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.48/ivygrep-v0.5.48-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.48 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.48/ivygrep-v0.5.48-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.48 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.48/ivygrep-v0.5.48-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.47 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.47/ivygrep-v0.5.47-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.47 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.47/ivygrep-v0.5.47-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.47 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.47/ivygrep-v0.5.47-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.47 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.47/ivygrep-v0.5.47-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.46 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.46/ivygrep-v0.5.46-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.46 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.46/ivygrep-v0.5.46-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.46 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.46/ivygrep-v0.5.46-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.46 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.46/ivygrep-v0.5.46-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.45 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.45/ivygrep-v0.5.45-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.45 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.45/ivygrep-v0.5.45-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.45 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.45/ivygrep-v0.5.45-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.45 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.45/ivygrep-v0.5.45-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.44 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.44/ivygrep-v0.5.44-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.44 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.44/ivygrep-v0.5.44-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.44 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.44/ivygrep-v0.5.44-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.44 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.44/ivygrep-v0.5.44-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.43 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.43/ivygrep-v0.5.43-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.43 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.43/ivygrep-v0.5.43-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.43 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.43/ivygrep-v0.5.43-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.43 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.43/ivygrep-v0.5.43-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.42 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.42/ivygrep-v0.5.42-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.42 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.42/ivygrep-v0.5.42-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.42 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.42/ivygrep-v0.5.42-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.42 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.42/ivygrep-v0.5.42-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.41 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.41/ivygrep-v0.5.41-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.41 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.41/ivygrep-v0.5.41-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.41 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.41/ivygrep-v0.5.41-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.41 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.41/ivygrep-v0.5.41-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.40 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.40/ivygrep-v0.5.40-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.40 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.40/ivygrep-v0.5.40-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.40 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.40/ivygrep-v0.5.40-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.40 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.40/ivygrep-v0.5.40-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.39 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.39/ivygrep-v0.5.39-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.39 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.39/ivygrep-v0.5.39-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.39 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.39/ivygrep-v0.5.39-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.39 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.39/ivygrep-v0.5.39-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.38 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.38/ivygrep-v0.5.38-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.38 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.38/ivygrep-v0.5.38-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.38 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.38/ivygrep-v0.5.38-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.38 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.38/ivygrep-v0.5.38-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.37 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.37/ivygrep-v0.5.37-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.37 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.37/ivygrep-v0.5.37-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.37 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.37/ivygrep-v0.5.37-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.37 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.37/ivygrep-v0.5.37-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.36 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.36/ivygrep-v0.5.36-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.36 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.36/ivygrep-v0.5.36-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.36 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.36/ivygrep-v0.5.36-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.36 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.36/ivygrep-v0.5.36-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.35 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.35/ivygrep-v0.5.35-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.35 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.35/ivygrep-v0.5.35-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.35 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.35/ivygrep-v0.5.35-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.35 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.35/ivygrep-v0.5.35-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.33 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.33/ivygrep-v0.5.33-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.33 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.33/ivygrep-v0.5.33-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.33 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.33/ivygrep-v0.5.33-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.33 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.33/ivygrep-v0.5.33-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-gnu | v0.5.32 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.32/ivygrep-v0.5.32-linux-aarch64-gnu.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.32 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.32/ivygrep-v0.5.32-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-gnu | v0.5.32 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.32/ivygrep-v0.5.32-linux-x86_64-gnu.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.32 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.32/ivygrep-v0.5.32-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.32 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.32/ivygrep-v0.5.32-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.32 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.32/ivygrep-v0.5.32-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64-gnu | v0.5.31 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.31/ivygrep-v0.5.31-linux-aarch64-gnu.tar.gz) |
| binary size | release/linux-aarch64-musl | v0.5.31 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.31/ivygrep-v0.5.31-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-gnu | v0.5.31 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.31/ivygrep-v0.5.31-linux-x86_64-gnu.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.5.31 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.31/ivygrep-v0.5.31-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.31 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.31/ivygrep-v0.5.31-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.31 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.31/ivygrep-v0.5.31-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.19 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.19/ivygrep-v0.5.19-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.19 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.19/ivygrep-v0.5.19-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.19 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.19/ivygrep-v0.5.19-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.19 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.19/ivygrep-v0.5.19-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.18 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.18/ivygrep-v0.5.18-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.18 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.18/ivygrep-v0.5.18-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.18 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.18/ivygrep-v0.5.18-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.18 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.18/ivygrep-v0.5.18-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.17 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.17/ivygrep-v0.5.17-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.17 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.17/ivygrep-v0.5.17-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.17 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.17/ivygrep-v0.5.17-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.17 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.17/ivygrep-v0.5.17-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.16 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.16/ivygrep-v0.5.16-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.16 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.16/ivygrep-v0.5.16-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.16 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.16/ivygrep-v0.5.16-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.16 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.16/ivygrep-v0.5.16-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.15 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.15/ivygrep-v0.5.15-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.15 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.15/ivygrep-v0.5.15-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.15 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.15/ivygrep-v0.5.15-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.15 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.15/ivygrep-v0.5.15-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.14 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.14/ivygrep-v0.5.14-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.14 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.14/ivygrep-v0.5.14-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.14 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.14/ivygrep-v0.5.14-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.14 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.14/ivygrep-v0.5.14-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.13 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.13/ivygrep-v0.5.13-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.13 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.13/ivygrep-v0.5.13-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.13 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.13/ivygrep-v0.5.13-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.13 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.13/ivygrep-v0.5.13-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.12 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.12/ivygrep-v0.5.12-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.12 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.12/ivygrep-v0.5.12-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.12 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.12/ivygrep-v0.5.12-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.12 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.12/ivygrep-v0.5.12-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.11 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.11/ivygrep-v0.5.11-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.11 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.11/ivygrep-v0.5.11-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.11 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.11/ivygrep-v0.5.11-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.11 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.11/ivygrep-v0.5.11-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.10 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.10/ivygrep-v0.5.10-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.10 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.10/ivygrep-v0.5.10-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.10 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.10/ivygrep-v0.5.10-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.10 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.10/ivygrep-v0.5.10-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.9 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.9/ivygrep-v0.5.9-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.9 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.9/ivygrep-v0.5.9-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.9 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.9/ivygrep-v0.5.9-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.9 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.9/ivygrep-v0.5.9-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.8 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.8/ivygrep-v0.5.8-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.8 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.8/ivygrep-v0.5.8-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.8 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.8/ivygrep-v0.5.8-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.8 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.8/ivygrep-v0.5.8-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.7/ivygrep-v0.5.7-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.7/ivygrep-v0.5.7-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.7/ivygrep-v0.5.7-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.7 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.7/ivygrep-v0.5.7-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.6/ivygrep-v0.5.6-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.6/ivygrep-v0.5.6-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.6/ivygrep-v0.5.6-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.6 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.6/ivygrep-v0.5.6-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.5/ivygrep-v0.5.5-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.5/ivygrep-v0.5.5-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.5/ivygrep-v0.5.5-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.5 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.5/ivygrep-v0.5.5-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.4/ivygrep-v0.5.4-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.4/ivygrep-v0.5.4-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.4/ivygrep-v0.5.4-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.4 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.4/ivygrep-v0.5.4-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.3/ivygrep-v0.5.3-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.3/ivygrep-v0.5.3-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.3/ivygrep-v0.5.3-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.3 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.3/ivygrep-v0.5.3-macos-x86_64.tar.gz) |
| binary size | release/linux-aarch64 | v0.5.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.2/ivygrep-v0.5.2-linux-aarch64.tar.gz) |
| binary size | release/linux-x86_64 | v0.5.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.2/ivygrep-v0.5.2-linux-x86_64.tar.gz) |
| binary size | release/macos-aarch64 | v0.5.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.2/ivygrep-v0.5.2-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.5.2 | unavailable | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.2/ivygrep-v0.5.2-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v1.0.2 | 13.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.2/ivygrep-v1.0.2-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v1.0.2 | 13.90 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.2/ivygrep-v1.0.2-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v1.0.2 | 12.94 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.2/ivygrep-v1.0.2-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v1.0.2 | 13.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.2/ivygrep-v1.0.2-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v1.0.2 | 12.71 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.2/ivygrep-v1.0.2-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v1.0.1 | 13.15 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.1/ivygrep-v1.0.1-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v1.0.1 | 13.94 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.1/ivygrep-v1.0.1-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v1.0.1 | 12.97 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.1/ivygrep-v1.0.1-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v1.0.1 | 13.14 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.1/ivygrep-v1.0.1-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v1.0.1 | 12.73 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.1/ivygrep-v1.0.1-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v1.0.0 | 13.15 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.0/ivygrep-v1.0.0-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v1.0.0 | 13.94 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.0/ivygrep-v1.0.0-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v1.0.0 | 12.96 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.0/ivygrep-v1.0.0-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v1.0.0 | 13.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.0/ivygrep-v1.0.0-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v1.0.0 | 12.73 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v1.0.0/ivygrep-v1.0.0-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.21 | 13.15 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.21/ivygrep-v0.12.21-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.21 | 13.94 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.21/ivygrep-v0.12.21-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.21 | 12.94 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.21/ivygrep-v0.12.21-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.21 | 13.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.21/ivygrep-v0.12.21-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.21 | 12.73 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.21/ivygrep-v0.12.21-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.20 | 13.14 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.20/ivygrep-v0.12.20-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.20 | 13.93 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.20/ivygrep-v0.12.20-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.20 | 12.94 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.20/ivygrep-v0.12.20-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.20 | 13.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.20/ivygrep-v0.12.20-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.20 | 12.72 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.20/ivygrep-v0.12.20-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.19 | 13.14 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.19/ivygrep-v0.12.19-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.19 | 13.92 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.19/ivygrep-v0.12.19-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.19 | 12.96 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.19/ivygrep-v0.12.19-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.19 | 13.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.19/ivygrep-v0.12.19-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.19 | 12.72 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.19/ivygrep-v0.12.19-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.18 | 13.15 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.18/ivygrep-v0.12.18-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.18 | 13.93 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.18/ivygrep-v0.12.18-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.18 | 12.94 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.18/ivygrep-v0.12.18-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.18 | 13.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.18/ivygrep-v0.12.18-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.18 | 12.72 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.18/ivygrep-v0.12.18-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.17 | 13.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.17/ivygrep-v0.12.17-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.17 | 13.91 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.17/ivygrep-v0.12.17-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.17 | 12.94 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.17/ivygrep-v0.12.17-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.17 | 13.11 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.17/ivygrep-v0.12.17-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.17 | 12.72 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.17/ivygrep-v0.12.17-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.16 | 13.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.16/ivygrep-v0.12.16-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.16 | 13.93 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.16/ivygrep-v0.12.16-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.16 | 12.94 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.16/ivygrep-v0.12.16-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.16 | 13.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.16/ivygrep-v0.12.16-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.16 | 12.73 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.16/ivygrep-v0.12.16-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.15 | 13.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.15/ivygrep-v0.12.15-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.15 | 13.92 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.15/ivygrep-v0.12.15-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.15 | 12.95 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.15/ivygrep-v0.12.15-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.15 | 13.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.15/ivygrep-v0.12.15-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.15 | 12.74 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.15/ivygrep-v0.12.15-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.14 | 13.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.14/ivygrep-v0.12.14-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.14 | 13.92 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.14/ivygrep-v0.12.14-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.14 | 12.75 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.14/ivygrep-v0.12.14-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.14 | 12.95 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.14/ivygrep-v0.12.14-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.14 | 12.73 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.14/ivygrep-v0.12.14-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.13 | 13.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.13/ivygrep-v0.12.13-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.13 | 13.92 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.13/ivygrep-v0.12.13-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.13 | 12.75 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.13/ivygrep-v0.12.13-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.13 | 12.95 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.13/ivygrep-v0.12.13-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.13 | 12.72 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.13/ivygrep-v0.12.13-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.12 | 13.12 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.12/ivygrep-v0.12.12-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.12 | 13.92 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.12/ivygrep-v0.12.12-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.12 | 12.75 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.12/ivygrep-v0.12.12-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.12 | 12.95 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.12/ivygrep-v0.12.12-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.12 | 12.73 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.12/ivygrep-v0.12.12-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.11 | 13.12 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.11/ivygrep-v0.12.11-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.11 | 13.92 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.11/ivygrep-v0.12.11-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.11 | 12.75 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.11/ivygrep-v0.12.11-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.11 | 12.95 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.11/ivygrep-v0.12.11-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.11 | 12.72 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.11/ivygrep-v0.12.11-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.10 | 13.10 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.10/ivygrep-v0.12.10-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.10 | 13.89 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.10/ivygrep-v0.12.10-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.10 | 12.73 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.10/ivygrep-v0.12.10-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.10 | 12.93 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.10/ivygrep-v0.12.10-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.10 | 12.70 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.10/ivygrep-v0.12.10-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.9 | 13.10 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.9/ivygrep-v0.12.9-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.9 | 13.90 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.9/ivygrep-v0.12.9-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.9 | 12.74 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.9/ivygrep-v0.12.9-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.9 | 12.92 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.9/ivygrep-v0.12.9-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.9 | 12.70 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.9/ivygrep-v0.12.9-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.8 | 12.53 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.8/ivygrep-v0.12.8-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.8 | 13.29 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.8/ivygrep-v0.12.8-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.8 | 12.12 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.8/ivygrep-v0.12.8-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.8 | 12.33 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.8/ivygrep-v0.12.8-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.8 | 12.12 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.8/ivygrep-v0.12.8-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.7 | 12.53 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.7/ivygrep-v0.12.7-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.7 | 13.30 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.7/ivygrep-v0.12.7-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.7 | 12.11 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.7/ivygrep-v0.12.7-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.7 | 12.34 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.7/ivygrep-v0.12.7-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.7 | 12.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.7/ivygrep-v0.12.7-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.6 | 12.37 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.6/ivygrep-v0.12.6-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.6 | 13.12 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.6/ivygrep-v0.12.6-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.6 | 11.96 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.6/ivygrep-v0.12.6-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.6 | 12.16 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.6/ivygrep-v0.12.6-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.6 | 11.95 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.6/ivygrep-v0.12.6-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.5 | 12.36 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.5/ivygrep-v0.12.5-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.5 | 13.09 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.5/ivygrep-v0.12.5-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.5 | 11.94 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.5/ivygrep-v0.12.5-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.5 | 12.14 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.5/ivygrep-v0.12.5-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.5 | 11.93 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.5/ivygrep-v0.12.5-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.4 | 12.35 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.4/ivygrep-v0.12.4-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.4 | 13.09 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.4/ivygrep-v0.12.4-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.4 | 11.95 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.4/ivygrep-v0.12.4-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.4 | 12.14 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.4/ivygrep-v0.12.4-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.4 | 11.93 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.4/ivygrep-v0.12.4-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.3 | 12.35 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.3/ivygrep-v0.12.3-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.3 | 13.09 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.3/ivygrep-v0.12.3-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.3 | 11.94 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.3/ivygrep-v0.12.3-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.3 | 12.14 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.3/ivygrep-v0.12.3-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.3 | 11.93 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.3/ivygrep-v0.12.3-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.2 | 12.33 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.2/ivygrep-v0.12.2-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.2 | 13.06 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.2/ivygrep-v0.12.2-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.2 | 11.93 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.2/ivygrep-v0.12.2-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.2 | 12.12 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.2/ivygrep-v0.12.2-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.2 | 11.92 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.2/ivygrep-v0.12.2-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.1 | 12.33 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.1/ivygrep-v0.12.1-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.1 | 13.06 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.1/ivygrep-v0.12.1-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.1 | 11.93 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.1/ivygrep-v0.12.1-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.1 | 12.12 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.1/ivygrep-v0.12.1-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.1 | 11.92 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.1/ivygrep-v0.12.1-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.12.0 | 12.31 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.0/ivygrep-v0.12.0-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.12.0 | 13.05 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.0/ivygrep-v0.12.0-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.12.0 | 11.91 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.0/ivygrep-v0.12.0-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.12.0 | 12.11 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.0/ivygrep-v0.12.0-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.12.0 | 11.90 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.12.0/ivygrep-v0.12.0-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.11.2 | 12.32 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.2/ivygrep-v0.11.2-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.11.2 | 13.05 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.2/ivygrep-v0.11.2-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.11.2 | 11.91 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.2/ivygrep-v0.11.2-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.11.2 | 12.11 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.2/ivygrep-v0.11.2-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.11.2 | 9.73 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.2/ivygrep-v0.11.2-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.11.0 | 12.31 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.0/ivygrep-v0.11.0-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.11.0 | 13.05 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.0/ivygrep-v0.11.0-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.11.0 | 11.91 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.0/ivygrep-v0.11.0-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.11.0 | 12.11 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.0/ivygrep-v0.11.0-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.11.0 | 9.72 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.11.0/ivygrep-v0.11.0-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.10.1 | 12.09 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.1/ivygrep-v0.10.1-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.10.1 | 12.82 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.1/ivygrep-v0.10.1-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.10.1 | 11.69 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.1/ivygrep-v0.10.1-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.10.1 | 11.88 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.1/ivygrep-v0.10.1-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.10.1 | 9.61 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.1/ivygrep-v0.10.1-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.10.0 | 12.09 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.0/ivygrep-v0.10.0-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.10.0 | 12.87 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.0/ivygrep-v0.10.0-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.10.0 | 11.69 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.0/ivygrep-v0.10.0-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.10.0 | 11.88 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.0/ivygrep-v0.10.0-macos-x86_64.tar.gz) |
| archive size | release/windows-x86_64 | v0.10.0 | 9.61 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.0/ivygrep-v0.10.0-windows-x86_64.zip) |
| archive size | release/linux-aarch64-musl | v0.9.7 | 122.09 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.7/ivygrep-v0.9.7-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.9.7 | 124.20 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.7/ivygrep-v0.9.7-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.9.7 | 12.72 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.7/ivygrep-v0.9.7-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.9.7 | 12.83 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.7/ivygrep-v0.9.7-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.9.6 | 122.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.6/ivygrep-v0.9.6-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.9.6 | 124.18 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.6/ivygrep-v0.9.6-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.9.6 | 12.71 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.6/ivygrep-v0.9.6-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.9.6 | 12.83 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.6/ivygrep-v0.9.6-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.9.5 | 122.05 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.5/ivygrep-v0.9.5-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.9.5 | 124.18 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.5/ivygrep-v0.9.5-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.9.5 | 12.71 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.5/ivygrep-v0.9.5-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.9.5 | 12.81 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.5/ivygrep-v0.9.5-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.9.4 | 121.97 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.4/ivygrep-v0.9.4-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.9.4 | 124.15 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.4/ivygrep-v0.9.4-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.9.4 | 12.71 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.4/ivygrep-v0.9.4-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.9.4 | 12.82 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.4/ivygrep-v0.9.4-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.9.3 | 121.91 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.3/ivygrep-v0.9.3-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.9.3 | 123.95 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.3/ivygrep-v0.9.3-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.9.3 | 12.69 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.3/ivygrep-v0.9.3-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.9.3 | 12.80 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.3/ivygrep-v0.9.3-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.9.2 | 121.89 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.2/ivygrep-v0.9.2-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.9.2 | 123.94 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.2/ivygrep-v0.9.2-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.9.2 | 12.68 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.2/ivygrep-v0.9.2-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.9.2 | 12.79 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.2/ivygrep-v0.9.2-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.9.1 | 121.89 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.1/ivygrep-v0.9.1-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.9.1 | 123.94 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.1/ivygrep-v0.9.1-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.9.1 | 12.68 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.1/ivygrep-v0.9.1-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.9.1 | 12.79 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.1/ivygrep-v0.9.1-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.9.0 | 109.38 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.0/ivygrep-v0.9.0-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.9.0 | 111.23 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.0/ivygrep-v0.9.0-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.9.0 | 12.58 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.0/ivygrep-v0.9.0-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.9.0 | 12.69 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.9.0/ivygrep-v0.9.0-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.8.1 | 109.09 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.1/ivygrep-v0.8.1-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.8.1 | 110.99 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.1/ivygrep-v0.8.1-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.8.1 | 12.55 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.1/ivygrep-v0.8.1-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.8.1 | 12.65 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.1/ivygrep-v0.8.1-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.8.0 | 109.06 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.0/ivygrep-v0.8.0-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.8.0 | 111.01 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.0/ivygrep-v0.8.0-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.8.0 | 12.53 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.0/ivygrep-v0.8.0-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.8.0 | 12.65 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.8.0/ivygrep-v0.8.0-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.7.0 | 108.35 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.7.0/ivygrep-v0.7.0-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.7.0 | 110.39 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.7.0/ivygrep-v0.7.0-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.7.0 | 12.30 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.7.0/ivygrep-v0.7.0-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.7.0 | 12.40 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.7.0/ivygrep-v0.7.0-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.20 | 108.36 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.20/ivygrep-v0.6.20-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.20 | 110.37 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.20/ivygrep-v0.6.20-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.20 | 12.29 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.20/ivygrep-v0.6.20-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.20 | 12.40 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.20/ivygrep-v0.6.20-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.19 | 108.34 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.19/ivygrep-v0.6.19-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.19 | 110.37 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.19/ivygrep-v0.6.19-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.19 | 12.29 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.19/ivygrep-v0.6.19-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.19 | 12.40 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.19/ivygrep-v0.6.19-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.18 | 108.31 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.18/ivygrep-v0.6.18-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.18 | 110.35 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.18/ivygrep-v0.6.18-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.18 | 12.28 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.18/ivygrep-v0.6.18-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.18 | 12.39 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.18/ivygrep-v0.6.18-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.17 | 108.08 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.17/ivygrep-v0.6.17-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.17 | 110.05 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.17/ivygrep-v0.6.17-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.17 | 12.27 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.17/ivygrep-v0.6.17-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.17 | 12.37 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.17/ivygrep-v0.6.17-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.16 | 107.98 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.16/ivygrep-v0.6.16-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.16 | 109.99 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.16/ivygrep-v0.6.16-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.16 | 12.25 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.16/ivygrep-v0.6.16-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.16 | 12.35 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.16/ivygrep-v0.6.16-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.15 | 107.95 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.15/ivygrep-v0.6.15-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.15 | 109.92 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.15/ivygrep-v0.6.15-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.15 | 12.24 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.15/ivygrep-v0.6.15-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.15 | 12.35 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.15/ivygrep-v0.6.15-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.14 | 107.94 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.14/ivygrep-v0.6.14-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.14 | 109.88 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.14/ivygrep-v0.6.14-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.14 | 12.24 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.14/ivygrep-v0.6.14-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.14 | 12.35 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.14/ivygrep-v0.6.14-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.13 | 107.83 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.13/ivygrep-v0.6.13-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.13 | 109.90 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.13/ivygrep-v0.6.13-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.13 | 12.25 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.13/ivygrep-v0.6.13-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.13 | 12.35 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.13/ivygrep-v0.6.13-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.12 | 107.95 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.12/ivygrep-v0.6.12-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.12 | 109.97 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.12/ivygrep-v0.6.12-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.12 | 12.27 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.12/ivygrep-v0.6.12-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.12 | 12.37 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.12/ivygrep-v0.6.12-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.11 | 107.78 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.11/ivygrep-v0.6.11-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.11 | 109.87 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.11/ivygrep-v0.6.11-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.11 | 12.25 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.11/ivygrep-v0.6.11-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.11 | 12.35 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.11/ivygrep-v0.6.11-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.10 | 107.48 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.10/ivygrep-v0.6.10-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.10 | 109.55 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.10/ivygrep-v0.6.10-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.10 | 12.22 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.10/ivygrep-v0.6.10-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.10 | 12.33 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.10/ivygrep-v0.6.10-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.9 | 107.55 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.9/ivygrep-v0.6.9-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.9 | 109.62 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.9/ivygrep-v0.6.9-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.9 | 12.22 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.9/ivygrep-v0.6.9-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.9 | 12.32 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.9/ivygrep-v0.6.9-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.8 | 107.32 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.8/ivygrep-v0.6.8-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.8 | 109.38 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.8/ivygrep-v0.6.8-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.8 | 12.19 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.8/ivygrep-v0.6.8-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.8 | 12.29 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.8/ivygrep-v0.6.8-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.7 | 107.22 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.7/ivygrep-v0.6.7-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.7 | 109.28 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.7/ivygrep-v0.6.7-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.7 | 12.17 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.7/ivygrep-v0.6.7-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.7 | 12.27 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.7/ivygrep-v0.6.7-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.6 | 107.22 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.6/ivygrep-v0.6.6-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.6 | 109.28 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.6/ivygrep-v0.6.6-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.6 | 12.17 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.6/ivygrep-v0.6.6-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.6 | 12.27 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.6/ivygrep-v0.6.6-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.5 | 107.23 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.5/ivygrep-v0.6.5-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.5 | 109.27 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.5/ivygrep-v0.6.5-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.5 | 12.17 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.5/ivygrep-v0.6.5-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.5 | 12.27 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.5/ivygrep-v0.6.5-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.4 | 107.19 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.4/ivygrep-v0.6.4-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.4 | 109.25 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.4/ivygrep-v0.6.4-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.4 | 12.17 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.4/ivygrep-v0.6.4-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.4 | 12.27 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.4/ivygrep-v0.6.4-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.3 | 103.89 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.3/ivygrep-v0.6.3-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.3 | 105.72 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.3/ivygrep-v0.6.3-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.3 | 12.12 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.3/ivygrep-v0.6.3-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.3 | 12.23 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.3/ivygrep-v0.6.3-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.2 | 103.84 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.2/ivygrep-v0.6.2-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.2 | 105.72 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.2/ivygrep-v0.6.2-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.2 | 12.12 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.2/ivygrep-v0.6.2-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.2 | 12.22 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.2/ivygrep-v0.6.2-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.1 | 98.72 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.1/ivygrep-v0.6.1-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.1 | 100.27 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.1/ivygrep-v0.6.1-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.1 | 11.52 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.1/ivygrep-v0.6.1-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.1 | 11.61 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.1/ivygrep-v0.6.1-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.6.0 | 98.73 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.0/ivygrep-v0.6.0-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.6.0 | 100.26 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.0/ivygrep-v0.6.0-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.6.0 | 11.52 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.0/ivygrep-v0.6.0-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.6.0 | 11.61 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.6.0/ivygrep-v0.6.0-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.56 | 96.23 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.56/ivygrep-v0.5.56-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.56 | 98.20 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.56/ivygrep-v0.5.56-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.56 | 8.99 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.56/ivygrep-v0.5.56-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.56 | 9.29 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.56/ivygrep-v0.5.56-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.55 | 96.21 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.55/ivygrep-v0.5.55-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.55 | 98.23 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.55/ivygrep-v0.5.55-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.55 | 9.00 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.55/ivygrep-v0.5.55-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.55 | 9.29 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.55/ivygrep-v0.5.55-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.54 | 96.25 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.54/ivygrep-v0.5.54-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.54 | 98.20 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.54/ivygrep-v0.5.54-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.54 | 8.99 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.54/ivygrep-v0.5.54-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.54 | 9.29 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.54/ivygrep-v0.5.54-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.53 | 96.25 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.53/ivygrep-v0.5.53-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.53 | 98.20 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.53/ivygrep-v0.5.53-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.53 | 8.99 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.53/ivygrep-v0.5.53-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.53 | 9.30 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.53/ivygrep-v0.5.53-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.52 | 96.26 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.52/ivygrep-v0.5.52-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.52 | 98.19 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.52/ivygrep-v0.5.52-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.52 | 8.99 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.52/ivygrep-v0.5.52-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.52 | 9.29 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.52/ivygrep-v0.5.52-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.51 | 96.24 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.51/ivygrep-v0.5.51-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.51 | 98.20 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.51/ivygrep-v0.5.51-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.51 | 8.99 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.51/ivygrep-v0.5.51-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.51 | 9.29 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.51/ivygrep-v0.5.51-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.50 | 96.19 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.50/ivygrep-v0.5.50-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.50 | 98.23 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.50/ivygrep-v0.5.50-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.50 | 8.99 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.50/ivygrep-v0.5.50-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.50 | 9.29 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.50/ivygrep-v0.5.50-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.49 | 96.20 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.49/ivygrep-v0.5.49-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.49 | 98.13 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.49/ivygrep-v0.5.49-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.49 | 8.99 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.49/ivygrep-v0.5.49-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.49 | 9.29 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.49/ivygrep-v0.5.49-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.48 | 96.21 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.48/ivygrep-v0.5.48-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.48 | 98.14 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.48/ivygrep-v0.5.48-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.48 | 8.99 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.48/ivygrep-v0.5.48-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.48 | 9.29 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.48/ivygrep-v0.5.48-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.47 | 96.21 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.47/ivygrep-v0.5.47-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.47 | 98.21 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.47/ivygrep-v0.5.47-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.47 | 8.98 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.47/ivygrep-v0.5.47-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.47 | 9.29 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.47/ivygrep-v0.5.47-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.46 | 96.19 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.46/ivygrep-v0.5.46-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.46 | 98.14 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.46/ivygrep-v0.5.46-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.46 | 8.98 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.46/ivygrep-v0.5.46-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.46 | 9.28 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.46/ivygrep-v0.5.46-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.45 | 96.16 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.45/ivygrep-v0.5.45-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.45 | 98.16 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.45/ivygrep-v0.5.45-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.45 | 8.97 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.45/ivygrep-v0.5.45-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.45 | 9.28 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.45/ivygrep-v0.5.45-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.44 | 94.55 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.44/ivygrep-v0.5.44-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.44 | 96.58 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.44/ivygrep-v0.5.44-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.44 | 7.89 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.44/ivygrep-v0.5.44-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.44 | 8.28 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.44/ivygrep-v0.5.44-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.43 | 94.41 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.43/ivygrep-v0.5.43-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.43 | 96.42 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.43/ivygrep-v0.5.43-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.43 | 7.88 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.43/ivygrep-v0.5.43-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.43 | 8.25 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.43/ivygrep-v0.5.43-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.42 | 94.41 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.42/ivygrep-v0.5.42-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.42 | 96.37 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.42/ivygrep-v0.5.42-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.42 | 7.86 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.42/ivygrep-v0.5.42-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.42 | 8.25 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.42/ivygrep-v0.5.42-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.41 | 94.39 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.41/ivygrep-v0.5.41-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.41 | 96.41 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.41/ivygrep-v0.5.41-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.41 | 7.86 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.41/ivygrep-v0.5.41-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.41 | 8.25 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.41/ivygrep-v0.5.41-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.40 | 94.35 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.40/ivygrep-v0.5.40-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.40 | 96.39 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.40/ivygrep-v0.5.40-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.40 | 7.86 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.40/ivygrep-v0.5.40-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.40 | 8.25 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.40/ivygrep-v0.5.40-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.39 | 94.35 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.39/ivygrep-v0.5.39-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.39 | 96.82 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.39/ivygrep-v0.5.39-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.39 | 7.93 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.39/ivygrep-v0.5.39-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.39 | 8.39 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.39/ivygrep-v0.5.39-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.38 | 94.34 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.38/ivygrep-v0.5.38-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.38 | 96.82 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.38/ivygrep-v0.5.38-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.38 | 7.93 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.38/ivygrep-v0.5.38-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.38 | 8.39 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.38/ivygrep-v0.5.38-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.37 | 94.29 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.37/ivygrep-v0.5.37-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.37 | 96.74 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.37/ivygrep-v0.5.37-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.37 | 7.93 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.37/ivygrep-v0.5.37-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.37 | 8.39 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.37/ivygrep-v0.5.37-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.36 | 94.29 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.36/ivygrep-v0.5.36-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.36 | 96.74 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.36/ivygrep-v0.5.36-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.36 | 7.93 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.36/ivygrep-v0.5.36-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.36 | 8.39 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.36/ivygrep-v0.5.36-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.35 | 94.30 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.35/ivygrep-v0.5.35-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.35 | 96.74 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.35/ivygrep-v0.5.35-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.35 | 7.93 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.35/ivygrep-v0.5.35-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.35 | 8.38 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.35/ivygrep-v0.5.35-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.33 | 53.35 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.33/ivygrep-v0.5.33-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.33 | 54.59 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.33/ivygrep-v0.5.33-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.33 | 7.93 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.33/ivygrep-v0.5.33-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.33 | 6.11 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.33/ivygrep-v0.5.33-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-gnu | v0.5.32 | 91.27 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.32/ivygrep-v0.5.32-linux-aarch64-gnu.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.32 | 53.33 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.32/ivygrep-v0.5.32-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-gnu | v0.5.32 | 102.83 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.32/ivygrep-v0.5.32-linux-x86_64-gnu.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.32 | 54.58 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.32/ivygrep-v0.5.32-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.32 | 14.38 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.32/ivygrep-v0.5.32-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.32 | 6.11 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.32/ivygrep-v0.5.32-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64-gnu | v0.5.31 | 91.28 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.31/ivygrep-v0.5.31-linux-aarch64-gnu.tar.gz) |
| archive size | release/linux-aarch64-musl | v0.5.31 | 53.33 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.31/ivygrep-v0.5.31-linux-aarch64-musl.tar.gz) |
| archive size | release/linux-x86_64-gnu | v0.5.31 | 102.82 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.31/ivygrep-v0.5.31-linux-x86_64-gnu.tar.gz) |
| archive size | release/linux-x86_64-musl | v0.5.31 | 54.58 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.31/ivygrep-v0.5.31-linux-x86_64-musl.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.31 | 14.38 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.31/ivygrep-v0.5.31-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.31 | 6.11 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.31/ivygrep-v0.5.31-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.19 | 53.24 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.19/ivygrep-v0.5.19-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.19 | 54.55 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.19/ivygrep-v0.5.19-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.19 | 14.38 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.19/ivygrep-v0.5.19-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.19 | 6.11 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.19/ivygrep-v0.5.19-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.18 | 53.25 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.18/ivygrep-v0.5.18-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.18 | 54.58 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.18/ivygrep-v0.5.18-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.18 | 14.38 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.18/ivygrep-v0.5.18-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.18 | 6.10 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.18/ivygrep-v0.5.18-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.17 | 53.22 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.17/ivygrep-v0.5.17-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.17 | 54.50 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.17/ivygrep-v0.5.17-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.17 | 14.37 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.17/ivygrep-v0.5.17-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.17 | 6.10 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.17/ivygrep-v0.5.17-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.16 | 53.20 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.16/ivygrep-v0.5.16-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.16 | 54.53 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.16/ivygrep-v0.5.16-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.16 | 14.37 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.16/ivygrep-v0.5.16-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.16 | 6.09 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.16/ivygrep-v0.5.16-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.15 | 53.17 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.15/ivygrep-v0.5.15-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.15 | 54.50 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.15/ivygrep-v0.5.15-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.15 | 14.38 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.15/ivygrep-v0.5.15-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.15 | 6.10 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.15/ivygrep-v0.5.15-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.14 | 53.07 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.14/ivygrep-v0.5.14-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.14 | 54.37 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.14/ivygrep-v0.5.14-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.14 | 14.37 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.14/ivygrep-v0.5.14-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.14 | 6.08 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.14/ivygrep-v0.5.14-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.13 | 53.07 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.13/ivygrep-v0.5.13-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.13 | 54.45 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.13/ivygrep-v0.5.13-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.13 | 14.36 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.13/ivygrep-v0.5.13-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.13 | 6.08 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.13/ivygrep-v0.5.13-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.12 | 53.04 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.12/ivygrep-v0.5.12-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.12 | 54.42 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.12/ivygrep-v0.5.12-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.12 | 14.36 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.12/ivygrep-v0.5.12-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.12 | 6.08 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.12/ivygrep-v0.5.12-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.11 | 53.09 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.11/ivygrep-v0.5.11-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.11 | 54.41 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.11/ivygrep-v0.5.11-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.11 | 14.34 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.11/ivygrep-v0.5.11-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.11 | 6.06 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.11/ivygrep-v0.5.11-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.10 | 53.09 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.10/ivygrep-v0.5.10-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.10 | 54.40 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.10/ivygrep-v0.5.10-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.10 | 14.34 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.10/ivygrep-v0.5.10-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.10 | 6.06 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.10/ivygrep-v0.5.10-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.9 | 53.10 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.9/ivygrep-v0.5.9-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.9 | 54.41 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.9/ivygrep-v0.5.9-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.9 | 14.34 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.9/ivygrep-v0.5.9-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.9 | 6.06 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.9/ivygrep-v0.5.9-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.8 | 53.08 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.8/ivygrep-v0.5.8-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.8 | 54.42 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.8/ivygrep-v0.5.8-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.8 | 14.33 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.8/ivygrep-v0.5.8-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.8 | 6.05 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.8/ivygrep-v0.5.8-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.7 | 53.05 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.7/ivygrep-v0.5.7-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.7 | 54.38 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.7/ivygrep-v0.5.7-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.7 | 14.33 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.7/ivygrep-v0.5.7-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.7 | 6.05 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.7/ivygrep-v0.5.7-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.6 | 53.00 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.6/ivygrep-v0.5.6-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.6 | 54.36 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.6/ivygrep-v0.5.6-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.6 | 14.33 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.6/ivygrep-v0.5.6-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.6 | 6.05 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.6/ivygrep-v0.5.6-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.5 | 53.01 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.5/ivygrep-v0.5.5-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.5 | 54.36 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.5/ivygrep-v0.5.5-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.5 | 14.33 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.5/ivygrep-v0.5.5-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.5 | 6.05 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.5/ivygrep-v0.5.5-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.4 | 50.80 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.4/ivygrep-v0.5.4-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.4 | 55.23 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.4/ivygrep-v0.5.4-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.4 | 14.37 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.4/ivygrep-v0.5.4-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.4 | 6.11 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.4/ivygrep-v0.5.4-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.3 | 50.64 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.3/ivygrep-v0.5.3-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.3 | 55.05 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.3/ivygrep-v0.5.3-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.3 | 14.34 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.3/ivygrep-v0.5.3-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.3 | 6.08 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.3/ivygrep-v0.5.3-macos-x86_64.tar.gz) |
| archive size | release/linux-aarch64 | v0.5.2 | 50.63 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.2/ivygrep-v0.5.2-linux-aarch64.tar.gz) |
| archive size | release/linux-x86_64 | v0.5.2 | 55.04 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.2/ivygrep-v0.5.2-linux-x86_64.tar.gz) |
| archive size | release/macos-aarch64 | v0.5.2 | 14.33 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.2/ivygrep-v0.5.2-macos-aarch64.tar.gz) |
| archive size | release/macos-x86_64 | v0.5.2 | 6.07 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.5.2/ivygrep-v0.5.2-macos-x86_64.tar.gz) |

## Claim status

- Portable: **supported** under the qualified five-target artifact definition.
- Competitive: **not claimed** without a controlled comparable-system result.
- State of the art: **not claimed**. Pareto evidence is
  present,
  while a top-tier comparable public result is
  unavailable.

## Comparable-system evidence

| Class | Status | Reason |
|---|---|---|
| exact-search | unavailable | No retained same-hardware exact-search comparison is available. |
| semantic-retrieval | unavailable | No retained external local semantic system uses the same corpus, model budget, and hardware. |

Regressions and unavailable comparisons remain listed; the renderer never
deletes them to improve the presentation.

## Immutable source artifacts

- [Frozen public retrieval baseline](https://github.com/bvolpato/ivygrep/blob/262cb61f70624a8d0f10844d204ae7593a85a6d8/docs/benchmarks/public-code-retrieval-baseline-results.json) (`6c855c4308b45f1c...`)
- [Current public retrieval matrix](https://github.com/bvolpato/ivygrep/blob/262cb61f70624a8d0f10844d204ae7593a85a6d8/docs/benchmarks/public-code-retrieval-results.json) (`37a4821a98769114...`)
- [Compact-index public retrieval matrix](https://github.com/bvolpato/ivygrep/blob/107e3b0726cf15dc1d4d45178aad3f732168a793/docs/benchmarks/public-million-quality-current.json) (`0a939ed959689bd9...`)
- [Portable embedding model selection](https://github.com/bvolpato/ivygrep/blob/262cb61f70624a8d0f10844d204ae7593a85a6d8/docs/benchmarks/embedding-model-bakeoff.json) (`5b3596feabcdfa83...`)
- [Held-out learned reranker](https://github.com/bvolpato/ivygrep/blob/4b24c627d9bd733edec5d145437e9aa35c2ab2ca/docs/benchmarks/public-reranker-results.json) (`e9ba2fee6fe5bb3e...`)
- [Public million-chunk benchmark](https://github.com/bvolpato/ivygrep/blob/107e3b0726cf15dc1d4d45178aad3f732168a793/docs/benchmarks/public-million-results.json) (`f4ba8051b2c7cf88...`)
- [Daemon hot-query cache](https://github.com/bvolpato/ivygrep/blob/a99f5fb6eafef9c41d656a42dccaf44e27972d32/docs/benchmarks/daemon-hot-query-cache-explore-results.tsv) (`bf168523793c2c3f...`)
- [Release artifact acceptance gate](https://github.com/bvolpato/ivygrep/blob/030e32e275494896d4b5e348c6c370f205c0bb69/.github/workflows/release.yml) (`a8a2a1292a798bc3...`)
- [Release artifact history](https://github.com/bvolpato/ivygrep/blob/b2d5a02608c33ac16c33bce9a15b76c41151c9c3/docs/benchmarks/release-artifact-history.json) (`3f9d87e6409a8285...`)

Raw machine-readable dashboard:
[`evidence-dashboard.json`](evidence-dashboard.json).
