# Context and regex hot paths

Measured on November 6, 2026, on a Ryzen 9 9950X3D workstation with 32 logical CPUs,
121 GiB RAM, Linux 6.17, and glibc. Builds and measured workloads ran separately.
These are shared-workstation measurements, not latency guarantees.

The baseline is `b2dcc6456dc85a705dbc69f9b1198ba97d71e682`.
The measured candidate source is `9f24037033c830bd9d23acc217e37143814807d1`.
Both binaries use the release profile with ThinLTO, `CARGO_PROFILE_RELEASE_DEBUG=1`,
and `CARGO_PROFILE_RELEASE_STRIP=none` for CPU profiling.

| Identity | SHA-256 |
| --- | --- |
| Baseline binary | `494bc3eaa266a1ec53d4eba5b55ba3378d1a2b5328e52c33ba67ba339d85041d` |
| Candidate binary | `bcffe1ab36d83eda2d18f93f5ddd1ca1a6844e3e8508f5e8136428c401d1899c` |
| Measured source patch, `git diff <baseline> <candidate> -- src` | `1d37daac47b47e4427d13b8fd913259e424ba27a3f9a2484f78dda8815b6dc7b` |
| Source corpus | `a817b782d2a569abf2394385d787c7453c5b919aa7a9b48dfc36fe45e2103190` |

## What changed

- Representative chunk selection computes each score once. It preserves the highest
  score, earliest start line, and first equal candidate without sorting the full list.
- Context relationship lookup shares immutable source parses. Every lookup first reads
  the file through the workspace root handle. The cache key includes path, language,
  and SHA-256 of current content; reuse also checks the complete text.
- Regex coverage caches publication metadata and uses hash membership for saved paths.
  Every request still walks current source files, applies current ignore rules, and
  rechecks the publication stamp. New and oversized files remain discoverable.

The source-parse cache retains at most eight entries and 2 MiB of source and cache
metadata. Tree-sitter allocations are opaque and are limited by entry count, not this
byte budget. Active requests can retain evicted entries. The regex cache retains at
most 32 entries and 32 MiB of accounted payload. These are not process RSS limits.

## Warm context packs

The corpus contains 69 files and 3,708,839 bytes exported from the baseline's `src`,
`README.md`, `Cargo.toml`, and `docs/architecture.md`. Two copies contain identical
source: a clean committed Git checkout and a new Git repository with untracked files.
The latter exercises transient change and graph expansion.

Each binary uses the same preindexed corpus and app home for each comparison.
`static-retrieval-v1` model assets are cached. Each run warms eight fixed queries,
then measures 128 packs at one client and 128 at eight clients, with a 2,000-token
budget and the daemon query-result cache disabled. Source-parse and neural-vector
caches keep their normal behavior. Values below are medians of three run statistics.

| Checkout | Clients | p50 before / after | p95 before / after | Daemon CPU before / after |
| --- | ---: | ---: | ---: | ---: |
| Clean | 1 | 89.45 / 21.28 ms | 161.72 / 55.30 ms | 12.12 / 3.45 s |
| Clean | 8 | 92.17 / 24.70 ms | 146.66 / 84.02 ms | 11.22 / 4.10 s |
| Untracked | 1 | 56.35 / 36.34 ms | 155.42 / 71.53 ms | 9.73 / 5.24 s |
| Untracked | 8 | 59.36 / 37.35 ms | 154.63 / 85.18 ms | 9.70 / 4.99 s |

All normalized context-pack fingerprints match the baseline for all eight queries
in both checkout states and every repetition. This protects the measured output;
the corpus has no relevance labels and does not establish quality on other tasks.

Memory has a tradeoff. After eight-client clean-checkout load, median daemon RSS
increased from 272.79 to 330.81 MiB. For the untracked checkout it fell from 293.91
to 260.04 MiB. After one-client load it increased from 95.15 to 134.39 MiB for the
clean checkout and from 98.41 to 126.48 MiB for the untracked checkout. These are
point-in-time RSS readings, including allocator retention, not live-tree sizes.

## Mixed MCP load and background indexing

The existing `bench_resource_load.py` harness exports the same untracked corpus,
creates fresh indexes, verifies forced-neural execution, and runs eight MCP clients
alongside a background indexing loop. Each run includes 128 hybrid searches and
128 context packs. Each profile ran three times with diagnostic stage timers.
These runs include initial context calls and differ from the warmed workload above.

| Profile | Metric | Before | After |
| --- | --- | ---: | ---: |
| Static retrieval v1 | Context p50 | 56.97 ms | 42.57 ms |
| Static retrieval v1 | Context p95 | 169.17 ms | 162.41 ms |
| Static retrieval v1 | Load CPU | 11.16 s | 7.23 s |
| Static retrieval v1 | Sampled peak RSS | 433.33 MiB | 424.43 MiB |
| Static retrieval v1 | RSS after short idle | 374.02 MiB | 392.03 MiB |
| Potion code 16m v2 | Context p50 | 58.60 ms | 44.56 ms |
| Potion code 16m v2 | Context p95 | 159.97 ms | 163.73 ms |
| Potion code 16m v2 | Load CPU | 11.42 s | 7.57 s |
| Potion code 16m v2 | Sampled peak RSS | 607.98 MiB | 568.87 MiB |
| Potion code 16m v2 | RSS after short idle | 567.90 MiB | 550.26 MiB |

Median context latency and CPU improve. The mixed-load p95 ranges overlap, and the
default model's median p95 is slightly higher. There is no demonstrated tail-latency
improvement for that workload. Short idle readings do not establish long-term memory
stability. See [the resource-load method](resource-load.md) for its other limits.

## Regex diagnostic

A deterministic committed fixture contains 10,000 small Rust files in 64 directories,
with a single rare regex target. Its source digest is
`e50e1175f78b488ad57448f9e89a0fe252b1eb5509cd363eed9f375309b81f23`.
Three repetitions use 128 warm requests per interface and eight process-cold CLI
calls. Warm calls replay the same pattern with warm publication metadata. Regex
requests execute the handler each time and do not use the general query-result
cache. This does not measure diverse patterns or cold coverage. The table shows
median run medians. Every run checks the rare target and
unchanged result fingerprints. This is a one-off diagnostic whose temporary driver
is not packaged in the repository. The existing Criterion regex cases remain
available for repository-native comparisons.

| Interface | Main before / after | Linked worktree before / after |
| --- | ---: | ---: |
| Raw daemon | 7.68 / 6.19 ms | 8.89 / 6.99 ms |
| Persistent MCP | 8.06 / 6.68 ms | 10.35 / 8.51 ms |
| Process-cold CLI | 15.93 / 16.91 ms | 19.22 / 18.35 ms |

Warm daemon and MCP latency improves by about 17–21%. Process-cold CLI results are
mixed; this change does not demonstrate a CLI startup improvement.

## Remaining costs

User-CPU sampling initially attributed about 52% to relationship lookup and 22% to
representative chunk selection. After the changes, relationship lookup accounts for
about 32%, including uncached parses. Representative selection accounts for about
24%, mostly SQLite chunk reads and decompression. These are inclusive sample shares,
not wall-clock percentages. Transient graph extraction remains another material cost.

Mixed-load graph-stage medians remain around 21–22 ms. Cold context calls and linked
worktree readiness still affect end-to-end tails. Regex retains a live filesystem
walk to preserve new-file and ignore-rule freshness. Further caching needs separate
freshness and memory evidence.

## Reproduce

Build the baseline and the measured candidate commit identified above in separate
checkouts and Cargo target directories. Do not run builds during measurements:

```sh
CARGO_TARGET_DIR=/tmp/ig-target-candidate \
  CARGO_PROFILE_RELEASE_DEBUG=1 CARGO_PROFILE_RELEASE_STRIP=none \
  cargo build --release --locked --bin ig
```

For mixed load, run the current harness separately for each binary with identical
options and distinct new work directories:

```sh
uv run --no-project scripts/bench_resource_load.py \
  --binary /tmp/ig-target-candidate/release/ig \
  --revision b2dcc6456dc85a705dbc69f9b1198ba97d71e682 \
  --runs 3 --samples 128 --clients 8 --diagnostics \
  --work-dir /tmp/ig-load-candidate --output /tmp/ig-load-candidate.json
```

For warmed context, export the corpus above, initialize Git, and optionally add a
signed commit to obtain the clean-checkout case. Preindex each corpus in a separate
app home with `--add <corpus> --no-watch --hash`, then run `--enhance-hash-internal`
and `--enhance-internal` for that corpus. Set `IVYGREP_MODEL_PROFILE=static`,
`IVYGREP_NO_AUTOSPAWN=1`, `IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT=1`, and
`IVYGREP_ENHANCE_MAX_LOAD_RATIO=0` for setup. Cache the pinned model assets beforehand.
Reuse the same corpus and home for both binaries and repeat each measurement three
times:

```sh
uv run scripts/bench_context_hotpaths.py \
  --binary /tmp/ig-target-candidate/release/ig \
  --corpus /tmp/ig-context-corpus --home /tmp/ig-context-home \
  --model-profile static --rounds 16 --budget 2000 \
  --output /tmp/ig-context-candidate-1.json
```

Compare `warm_fingerprints` between reports before comparing timings. The helper
requires an idle preindexed home, refuses existing daemon endpoints and reports,
and fails if the corpus changes or a context pack changes during a run.

The repository also provides a separate 1,000-file regex fixture. Run it in each
checkout with separate target directories:

```sh
CARGO_TARGET_DIR=/tmp/ig-regex-target-candidate \
  cargo bench --locked --bench indexer_bench -- \
  base_search_patterns/regex_symbol_1000_files
```

That fixture does not reproduce the one-off 10,000-file diagnostic table.
