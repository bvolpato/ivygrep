# Context graph hydration

Measured on November 6, 2026, on a Ryzen 9 9950X3D workstation with 32 logical
CPUs, 121 GiB RAM, Linux 6.17, and Rust 1.98.1. This follows the
[source-parse and representative-selection changes](hotpaths.md).

## Change and freshness

Transient context seeds can reach the same indexed file through several graph
edges. A context pack now shares successful representative-file lookups across
transient, persisted, and test graph expansion when it has transient seeds.
Packs without transient seeds use the original lookup path.

The cache lasts for one pack and retains at most 64 hits and 1 MiB of accounted
payload. These limits exclude allocator and cache-container overhead and are not
process RSS limits. Live-source fallback, missing files, and lookup errors are
not cached. Each caller receives a pristine hit clone before adding edge-specific
scores, reasons, and sources.

Before reusing a hit, the code checks source presence through a fresh workspace
root handle and compares the workspace and optional base SQLite stores against
an immutable initial stamp.
The stamp combines `PRAGMA data_version` on the same read-only connections with
Unix device, inode, length, modification time, and change time. A changed or
unavailable stamp, or a lookup error, disables reuse for the rest of that pack.
This detects intervening commits and staged store replacement without adding a
writer lock or refreshing the existing SearchContext snapshot. Windows keeps
the uncached path.

## Identity and method

The baseline is `43d7196aa5a3703014278164002b8d350dde3673`. Both binaries use the
release profile with ThinLTO, `CARGO_PROFILE_RELEASE_DEBUG=1`, and
`CARGO_PROFILE_RELEASE_STRIP=none`.

| Identity | SHA-256 |
| --- | --- |
| Baseline binary | `641aee034c47964e88c1e0bb0e3002526ac4f6bcc7fc6919bc6aa3ae0962cdd7` |
| Candidate binary | `f1beb5c4971ec7a316a03f624a13d2c4702fed5d069c08fdf7db4123840cca87` |
| Measured `git diff <baseline> -- src` | `084da4db0d44b576c55b3003745d81e264263859dada960e9088d9ffe8810d04` |
| Source corpus | `a817b782d2a569abf2394385d787c7453c5b919aa7a9b48dfc36fe45e2103190` |

The corpus is the same 69-file, 3,708,839-byte public-source export used in the
earlier report. Identical source is tested as a clean committed Git checkout and
as untracked files in a new Git repository. The static retrieval v1 model and
assets are cached. Each binary reuses the same idle preindexed home for a corpus.

Each run warms eight fixed queries, then measures 256 context packs at one client
and 256 at eight clients. Packs have a 2,000-token budget. The daemon query-result
cache is disabled; source-parse and neural-vector caches retain normal behavior.
The helper waits for ready, idle indexing before warmup. Startup, readiness waits,
and warmup are excluded from timings and daemon CPU deltas.

Each setup has three paired repetitions, alternating base/head execution order.
The first setup allows all 32 logical CPUs. The second uses
`taskset -c 0-7,16-23`, a single L3 cache group with 16 logical CPUs, for the driver
and all its children. Builds and measured workloads run separately. Recorded
before/after one-minute load averages ranged from 4.95–21.14 and 27.45–34.47,
respectively. These are shared-host diagnostics, not isolated latency guarantees.

All complete normalized context-pack fingerprints match between binaries in
both checkout states, both CPU setups, and every repetition. The helper also
checks token budgets, sample distribution, and unchanged corpus content. These
queries have no relevance labels; parity does not establish quality on other
tasks.

## Results

Values are medians of the three run statistics. CPU is total daemon CPU during
the 256 measured requests at that concurrency.

| All CPUs | Clients | p50 before / after | p95 before / after | CPU before / after |
| --- | ---: | ---: | ---: | ---: |
| Clean | 1 | 24.51 / 23.27 ms | 64.93 / 60.47 ms | 8.02 / 7.66 s |
| Clean | 8 | 31.50 / 33.72 ms | 108.22 / 96.50 ms | 9.74 / 8.86 s |
| Untracked | 1 | 36.87 / 32.26 ms | 75.49 / 69.58 ms | 10.72 / 9.54 s |
| Untracked | 8 | 43.41 / 31.50 ms | 94.04 / 78.22 ms | 11.41 / 8.72 s |

| Fixed L3 group | Clients | p50 before / after | p95 before / after | CPU before / after |
| --- | ---: | ---: | ---: | ---: |
| Clean | 1 | 42.61 / 45.97 ms | 111.23 / 119.68 ms | 12.80 / 13.48 s |
| Clean | 8 | 62.73 / 51.63 ms | 202.38 / 161.35 ms | 14.27 / 13.12 s |
| Untracked | 1 | 67.50 / 56.91 ms | 132.67 / 120.38 ms | 18.07 / 16.23 s |
| Untracked | 8 | 59.82 / 52.52 ms | 149.13 / 131.47 ms | 14.85 / 12.50 s |

The untracked checkout uses about 10–24% less daemon CPU across these setups.
At eight clients its p50 falls about 12–27%. Clean-checkout results vary, with a
slower median in some comparisons. No general clean-checkout speedup is claimed.
For example, fixed-group clean single-client CPU ranges overlap: 12.27–14.49 s
before and 12.05–13.54 s after.

Memory has a tradeoff. RSS is sampled immediately after each measured phase and
includes allocator retention and existing parse/vector caches.

| Setup | Checkout | Clients | RSS before / after |
| --- | --- | ---: | ---: |
| All CPUs | Clean | 1 | 134.46 / 134.59 MiB |
| All CPUs | Clean | 8 | 371.39 / 367.28 MiB |
| All CPUs | Untracked | 1 | 127.98 / 159.19 MiB |
| All CPUs | Untracked | 8 | 298.39 / 310.81 MiB |
| Fixed L3 group | Clean | 1 | 160.63 / 161.01 MiB |
| Fixed L3 group | Clean | 8 | 353.21 / 421.90 MiB |
| Fixed L3 group | Untracked | 1 | 158.58 / 158.21 MiB |
| Fixed L3 group | Untracked | 8 | 327.13 / 367.21 MiB |

The fixed-group clean eight-client ranges are 349.92–402.57 MiB before and
354.23–432.40 MiB after. There is no demonstrated memory reduction or long-term
memory-stability claim from these snapshots. The per-pack payload bound does not
bound the process's other caches or retained allocation pages.

## Profiling and remaining costs

Separate 15-second user-cycle profiles use `perf record -F 199` and DWARF stacks
against the owned daemon while one MCP client cycles the same tasks. Both report
zero lost samples. Representative hydration accounts for about 22% of baseline
samples; the candidate hydration path accounts for about 13%. Relationship lookup
remains the largest sampled cost, about 37% in the candidate. These are inclusive
sample shares, not wall-clock percentages.

An earlier version cached packs without transient seeds too. Instrumentation
showed only zero or one reuse for 12–17 lookups in clean packs, compared with
30–41 reuses in the untracked workload. The final version avoids cache allocation
and stamp reads when there are no transient seeds. The remaining relationship
parsing cost and variable concurrent RSS need separate work before increasing
parse-cache retention.

## Reproduce

Build the baseline and candidate source in separate checkouts and target
directories using the release overrides above. Prepare idle indexes for identical
clean and untracked copies of the public corpus, with the static model profile.
Then run each binary separately, alternating order across three repetitions:

```sh
uv run scripts/bench_context_hotpaths.py \
  --binary /tmp/ig-candidate --repo . \
  --corpus /tmp/context-corpus --home /tmp/context-home \
  --model-profile static --rounds 32 --budget 2000 \
  --output /tmp/context-candidate-1.json
```

Prefix the command with `taskset -c 0-7,16-23` only on a host with the reported
CPU/cache topology to reproduce the fixed-group setup. Use new output paths for
every run. Compare `warm_fingerprints` across variants and report latency, CPU,
and RSS together. The helper refuses an existing daemon endpoint or output file.
