# Daemon mutation and restart soak

```sh
python3 scripts/soak_daemon.py --binary target/release/ig --repo . \
  --duration 1800 --restarts 2 --output benchmark-results/daemon-soak.json
```

This Linux-only check copies the corpus into a temporary repository, prepares hash
vectors, and issues concurrent requests over the real daemon protocol. There is
no CLI search fallback. It verifies exact indexed probe revisions, deletion and
recreation, then repeats after offline changes and process restarts. A stable
probe query also exercises result-cache invalidation.

Every process epoch first runs the same query/mutation workload for 30 seconds to
initialize lazy thread pools, then gets independent resource samples for its share
of the requested loaded duration. After discarding the first
20% for warmup, the medians of the first and last quarters of the remaining
samples must stay within these growth budgets: 32 MiB RSS, eight file descriptors,
and four threads. The report includes peaks and cooldown samples separately.
These are bounded-growth gates, not a proof that arbitrarily slow leaks cannot
exist. A restart cannot hide a failed epoch. Missing samples, failed queries,
stale content, insufficient samples, or a budget violation fail the run.

The JSON report and adjacent daemon log are retained on failure. Reports identify
the binary, source commit/dirty status, harness hash, per-epoch query counts,
mutations, correctness checks, and resource windows. Latency quantiles describe
the last 50,000 successful requests of each epoch; they are load diagnostics, not
isolated search benchmarks. The latency sample buffer is bounded.

The performance workflow runs 120 loaded seconds with two restarts on relevant
PRs, and 1,800 loaded seconds with two restarts on scheduled/manual runs. Each
epoch must have at least 30 loaded seconds and 20 actual samples. Use longer runs
for slow leaks; do not increase resource budgets just to turn a failure green.

## Linux ARM64 acceptance run

The [short-run report](daemon-soak-linux-arm64-short.json) records 79,870 successful
RPC queries, 51 content checks, and two restarts against main `7413229` on
2026-09-02. All three process epochs passed the unchanged budgets; maximum
steady-window RSS growth was 14.82 MiB, FD growth was zero or negative, and thread
growth was at most one. This is short-run acceptance, not long-soak evidence.
