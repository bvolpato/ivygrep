# Model cost and concurrent latency

The [resource report](resource-load.html) measures the released v1.3.0 Linux x86_64 musl binary.
It compares `static-retrieval-v1` with `potion-code-16m-v2`, the current default.
Each model ran three times. The runner alternated model order between repetitions.
The report also compares unchanged source with the resource changes on the same glibc build target.

## Measured results

The source comparison uses local Linux x86_64 glibc builds. These values are medians of three runs.

| Metric | Before changes | After changes |
| --- | ---: | ---: |
| MCP hybrid search p95 | 527.83 ms | 90.96 ms |
| Context-pack p95 | 1,678.51 ms | 1,139.67 ms |
| Daemon RSS after the idle pause | 818.03 MiB | 562.06 MiB |

MCP search time includes index-readiness polling. Context packs reuse readers and parsed input after the changes.
Lexical indexing took 0.3% longer, and neural enhancement took 0.9% longer. Their ranges overlap the baseline ranges.
The HTML report includes p99, resource totals, every range, and the separate diagnostic run.

In the released musl binary, `potion-code-16m-v2` enhancement took 18.4% longer than `static-retrieval-v1` on this corpus.
Its enhancement peak RSS was 19.4% lower. The earlier screening measured peak RSS with a different scope.

## Corpus and load

The corpus contains 65 source and documentation files from revision
`89ae90d58806dac145b6129bfc68ec450658a37e`. Its contents total 3,465,965 bytes.
The runner exports `src`, `README.md`, `Cargo.toml`, and `docs/architecture.md` through `git archive`.
Uncommitted files do not enter the corpus.
The exported corpus starts as a new Git repository without a commit.
Its files are untracked, so context packs process current changes as well as retrieval results.

Every run creates new indexes in a separate app home. Model assets are already cached.
The runner measures lexical indexing, hash enhancement, and neural enhancement separately.
It then verifies forced-neural execution and measures 128 forced-neural queries.
Eight MCP clients each send 16 hybrid searches and 16 context-pack requests.
A separate client repeatedly indexes another workspace during those MCP calls.
Context packs use a 2,000-token budget. The query-result cache is disabled.
The eight queries repeat. Neural query-vector and file-preview caches keep their default behavior.
MCP clients start fresh. Their first calls include index-readiness polling and watcher setup.
These are end-to-end tool latencies, including those readiness costs.

These conditions differ from the release-preparation soak. That soak used a different request mix and indexing load.
Its latency percentiles are not directly comparable with this report.
The corpus has no relevance labels. Use the public retrieval reports to assess retrieval quality.
The [model screening](embedding-model-bakeoff.html) remains historical evidence from one repetition.
Its peak RSS uses cumulative child-process counters rather than an isolated enhancement measurement.
The repeated report uses per-child `wait4` counters. Its RSS values have a different scope.

## Reproduce the released-binary measurements

Download the release archive and verify its checksum:

```sh
mkdir -p /instance_storage/ivygrep-release
gh release download v1.3.0 \
  --pattern 'ivygrep-v1.3.0-linux-x86_64-musl.tar.gz*' \
  --dir /instance_storage/ivygrep-release
cd /instance_storage/ivygrep-release
sha256sum -c ivygrep-v1.3.0-linux-x86_64-musl.tar.gz.sha256
tar -xzf ivygrep-v1.3.0-linux-x86_64-musl.tar.gz
```

From the repository root, cache both pinned models:

```sh
uv run scripts/cache_neural_model.py --profile static --cache ~/.cache/huggingface
uv run scripts/cache_neural_model.py --profile potion-code-v2 --cache ~/.cache/huggingface
```

Run the benchmark while no builds or other benchmark jobs run on the host:

```sh
uv run scripts/bench_resource_load.py \
  --binary /instance_storage/ivygrep-release/ivygrep-v1.3.0-linux-x86_64-musl/ig \
  --revision 89ae90d58806dac145b6129bfc68ec450658a37e \
  --runs 3 --samples 128 --clients 8 \
  --work-dir /instance_storage/ivygrep-resource-release \
  --output docs/benchmarks/resource-load-release.json
```

The work directory must not exist before the run. Logs and indexes remain there for inspection.
If you do not enable diagnostics, unset `RUST_LOG` before the run.
The JSON contains raw latencies, per-phase resource measurements, corpus identity, binary identity, and harness hashes.
`wait4` records RSS and CPU for each child separately. Earlier child processes do not inflate later peak RSS values.
On Linux, filesystem-write counters measure block I/O. Writes to a memory-backed filesystem can report zero block I/O.
Load RSS is sampled every 20 ms. Idle RSS is the median during a three-second pause after the clients exit.
Load resource totals describe the daemon and exclude MCP client processes.
Enhancement resource totals describe the invoked CLI process.
The background loop completes a variable number of index runs.
Load CPU and write totals also reflect that count, which the JSON records.
That short pause can include allocator retention. It does not establish a memory leak or a long-term idle footprint.
The empirical p99 uses the nearest observed rank. It does not establish a production latency guarantee.

## Compare source changes

Build the unchanged source and the candidate for the same target, with the same feature flags.
Copy each binary before another build replaces it.
Use `--profiles potion-code-16m-v2` and the same revision, sample count, and client count for both runs.
Save their reports as `resource-load-baseline.json` and `resource-load-candidate.json`.
The local comparison uses Linux x86_64 glibc binaries. It is separate from the released musl measurements.
The baseline uses source revision `89ae90d58806dac145b6129bfc68ec450658a37e`.
The candidate starts from release revision `0e1e39386d0c98caf3d44dbe618787350ecfe937`.
Its implementation is pinned to commit `7906274fc219dc5e97afee8557c0e1a91f0fda96` in the candidate JSON.
The intervening commit changed version metadata, reports, and one test. It did not change the implementation under `src`.

Render all three reports:

```sh
uv run scripts/render_resource_load.py \
  --input docs/benchmarks/resource-load-release.json \
  --baseline docs/benchmarks/resource-load-baseline.json \
  --candidate docs/benchmarks/resource-load-candidate.json \
  --output docs/benchmarks/resource-load.html
```

The renderer refuses incomplete runs or comparisons with different corpora, methods, or harness hashes.
Keep the positive and negative changes in the table. Use the ranges to assess variation between repetitions.

## Inspect scheduling and exact scans

Use a separate run with `--diagnostics` to collect stage timings and exact-scan counts.
That run adds debug-log writes. Do not compare its resource totals with runs that disable diagnostics.
The runner reports CPU-permit waits, context assembly, semantic ANN stages, recovery count, and scanned-key count.
See [memory budgets and diagnostics](../architecture.md#memory-budgets-and-performance-diagnostics) for the field definitions.

```sh
uv run scripts/bench_resource_load.py \
  --binary /path/to/candidate/ig \
  --revision 89ae90d58806dac145b6129bfc68ec450658a37e \
  --profiles potion-code-16m-v2 --runs 1 --samples 16 --clients 8 \
  --diagnostics \
  --work-dir /instance_storage/ivygrep-resource-diagnostics \
  --output docs/benchmarks/resource-load-diagnostics.json
```

Add `--diagnostics docs/benchmarks/resource-load-diagnostics.json` to the renderer command to include its stage table.
Stages can be nested. Do not add their percentiles to estimate request p95.

Zero exact scans in this corpus do not rule out scans in a workspace with stale, ignored, or shadowed vector candidates.
The search regression tests cover recovery past those candidates.
Adaptive overfetch keeps ANN approximate and retains exact recovery when retries still underfill.
The focused regression fixture places eight orphan vectors ahead of four visible keys.
Overfetch recovers all four without an exact scan. A request for five still scans the four eligible SQLite keys.

The checked-in JSON adds model identities read from each run's persisted `neural_model.json`.
It also records build targets and source revisions. Candidate source hashes identify the files at the pinned implementation commit.
