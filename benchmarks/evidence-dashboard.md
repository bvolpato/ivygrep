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
| Release archive history | v0.10.1 with 5 archives |

## Versioned histories

| Metric family | Retained points | Unavailable points |
|---|---:|---:|
| quality | 2 | 0 |
| latency | 2 | 0 |
| indexing | 2 | 0 |
| memory | 2 | 0 |
| index size | 2 | 0 |
| binary size | 42 | 32 |
| archive size | 42 | 0 |

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
| binary size | release/linux-aarch64-musl | v0.10.1 | 62.25 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.1/ivygrep-v0.10.1-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.10.1 | 65.47 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.1/ivygrep-v0.10.1-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.10.1 | 61.83 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.1/ivygrep-v0.10.1-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.10.1 | 62.31 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.1/ivygrep-v0.10.1-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.10.1 | 59.03 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.1/ivygrep-v0.10.1-windows-x86_64.zip) |
| binary size | release/linux-aarch64-musl | v0.10.0 | 62.25 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.0/ivygrep-v0.10.0-linux-aarch64-musl.tar.gz) |
| binary size | release/linux-x86_64-musl | v0.10.0 | 65.79 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.0/ivygrep-v0.10.0-linux-x86_64-musl.tar.gz) |
| binary size | release/macos-aarch64 | v0.10.0 | 61.83 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.0/ivygrep-v0.10.0-macos-aarch64.tar.gz) |
| binary size | release/macos-x86_64 | v0.10.0 | 62.31 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.0/ivygrep-v0.10.0-macos-x86_64.tar.gz) |
| binary size | release/windows-x86_64 | v0.10.0 | 59.03 MiB | not-applicable | [source](https://github.com/bvolpato/ivygrep/releases/download/v0.10.0/ivygrep-v0.10.0-windows-x86_64.zip) |
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
- [Release artifact acceptance gate](https://github.com/bvolpato/ivygrep/blob/030e32e275494896d4b5e348c6c370f205c0bb69/.github/workflows/release.yml) (`c3fc661a5408e187...`)
- [Release artifact history](https://github.com/bvolpato/ivygrep/blob/a21f59b13b692d495b83e7d33ae4bcb712c4c195/docs/benchmarks/release-artifact-history.json) (`be897033dc727958...`)

Raw machine-readable dashboard:
[`evidence-dashboard.json`](evidence-dashboard.json).
