# Linux Kernel Relevance Strategy

## Goal

Optimize ivygrep for contextual code search on a large real repository. The benchmark uses the Linux kernel because it has many near-miss files: implementation code, headers, docs, selftests, samples, drivers, and generated-looking helpers all share terms. Good ranking should put core implementation files first and push secondary or spammy matches down.

## Query Design

The query set lives in `tests/fixtures/linux_kernel_relevance_queries.json`. Each query is written as a natural-language intent, not as a function name, path, literal query, or regex. Examples:

- `where does kernel allocate tiny reusable memory objects`
- `where are eBPF programs checked before running`
- `where are block IO requests queued to hardware`
- `how are cgroup memory charges tracked`

Each case has graded path-pattern judgments:

- Grade `3`: primary implementation file for the intent.
- Grade `2`: adjacent implementation or API header.
- Grade `1`: useful supporting context, but not the main answer.
- Grade `0`: everything else.

Case-specific spam patterns mark tempting but usually wrong top results, such as `tools/testing/selftests/**` for implementation questions or GPU scheduler wrappers for core scheduler questions.

## Relevance Score

The harness is `scripts/bench_linux_relevance.py`. It runs `ig --hash --json --no-watch -n 50` for every query, grades ranked file paths, and prints metrics JSON as the final line.

Primary metric:

```text
linux_relevance_score =
  100 * (
    0.55 * mean_nDCG@10 +
    0.20 * mean_MRR@10 +
    0.15 * mean_precision@5 +
    0.10 * mean_recall@20
  ) *
  max(0,
    1 -
    0.35 * spam_top10_rate -
    0.50 * forbidden_top3_rate -
    0.50 * no_hit_rate
  )
```

Higher is better.

Why these components:

- `nDCG@10` rewards putting grade-3 and grade-2 files near the top while still allowing useful secondary results.
- `MRR@10` rewards first relevant result appearing early.
- `precision@5` rewards a clean first screen.
- `recall@20` catches cases where relevant files are retrievable but ranked too low.
- `spam_top10_rate` and `forbidden_top3_rate` punish docs/tools/selftests/samples or subsystem-specific noise when the query asks for implementation.
- `no_hit_rate` prevents a high average from hiding broken queries.

## Benchmark Command

Fast run using an existing Linux index:

```bash
uv run scripts/bench_linux_relevance.py --kernel /home/bruno/githubworkspace/linux --bench-home /tmp/ivygrep-linux-bench-home --skip-index --skip-build --details
```

Reproducible run that creates a dedicated relevance index if missing:

```bash
uv run scripts/bench_linux_relevance.py --kernel /home/bruno/githubworkspace/linux --bench-home /tmp/ivygrep-linux-relevance-home --details
```

The final output line is JSON and contains `linux_relevance_score`, component metrics, spam counts, kernel commit, query count, and elapsed time.

## Baseline

Baseline on May 15, 2026, using Linux commit `1d5dcaa3bd65` and existing index `/tmp/ivygrep-linux-bench-home`:

| Metric | Value |
| --- | ---: |
| `linux_relevance_score` | `3.5167` |
| `quality_points` | `4.8507` |
| `penalty_factor` | `0.7250` |
| `mean_nDCG@10` | `0.0355` |
| `mean_MRR@10` | `0.0897` |
| `mean_precision@5` | `0.0308` |
| `mean_recall@20` | `0.0641` |
| `top_relevant_rate` | `0.0769` |
| `spam_top10_rate` | `0.3462` |
| `forbidden_top3_rate` | `0.3077` |

This is intentionally low. Current ranking finds some adjacent signals, but spam and subsystem-adjacent files dominate top results.

## v0.9.6 Validation

Fresh validation on June 12, 2026 used Linux commit `062871f13`, 93,502 files,
and 4,419,660 indexed chunks. The retained implementation keeps query expansion
portable: no benchmark query IDs, expected paths, or Linux subsystem aliases are
encoded in ranking logic. The comparison baseline is the `6.3325` score measured
from `main` immediately before the v0.9.6 relevance work.

| Metric | Baseline | v0.9.6 |
| --- | ---: | ---: |
| `linux_relevance_score` | `6.3325` | `41.1951` |
| `mean_nDCG@10` | `0.0553` | `0.4162` |
| `mean_MRR@10` | `0.0590` | `0.4897` |
| `mean_precision@5` | `0.0615` | `0.2615` |
| `mean_recall@20` | `0.2051` | `0.6026` |
| `spam_top10_rate` | `0.1231` | `0.0231` |

## Optimization Strategy

Keep changes only when the benchmark improves by at least 1% without making ranking logic worse or more brittle. Small gains with large complexity should be discarded.

Useful experiment directions:

- Query expansion for intent words, such as `checked` to verifier-like concepts or `deferred background work` to workqueue-like concepts.
- File authority signals that favor implementation paths over docs, tools, selftests, samples, and driver wrappers when query intent is core behavior.
- Better path and file-stem matching so `block IO requests queued to hardware` can surface `block/blk-mq.c` without literal path terms.
- RRF and threshold tuning that keeps relevant secondary results but suppresses unsupported semantic neighbors.
- Spam demotion that is intent-sensitive, so documentation and tests can rank when the user asks for docs or tests.

Guardrails:

- Do not hard-code query IDs or expected Linux paths in search code.
- Preserve exact/literal lookup behavior.
- Preserve doc/test surfacing when query intent asks for secondary sources.
- Run `cargo test --test relevance_quality -- --nocapture` and the Linux relevance benchmark before retaining changes.
