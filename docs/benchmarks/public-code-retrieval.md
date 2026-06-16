# Public code-retrieval benchmark

This report is generated from pinned public CoIR datasets. It contains no hostnames, user paths, private repository names, or source text.

- Commit: `2c735847d43edbe8a31d516b0fbb7c22b20105c2`
- Profile: `public-core`
- Tasks: 4
- Languages: 50
- Held-out queries: 1000
- Repetitions: 3

## Aggregate results

| Mode | nDCG@10 | MRR@10 | P@5 | R@20 | Warm p95 | Index time | Index size | Peak RSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| neural | 0.2620 | 0.2178 | 0.0561 | 0.5080 | 220.14 ms | 16331.47 ms | 134.25 MiB | 317.12 MiB |

## Change from frozen baseline

Baseline commit `49b1571de77ab096512469e324f76a48e4257123` mode `hash` is compared with current mode `neural`.

| Metric | Baseline | Current | Relative change |
| --- | ---: | ---: | ---: |
| nDCG@10 | 0.2324 | 0.2620 | +12.77% |
| MRR@10 | 0.1924 | 0.2178 | +13.22% |
| P@5 | 0.0517 | 0.0561 | +8.65% |
| R@20 | 0.4733 | 0.5080 | +7.32% |

| Task | Baseline nDCG@10 | Current nDCG@10 | Absolute change |
| --- | ---: | ---: | ---: |
| codetrans-dl | 0.1944 | 0.2365 | +0.0421 |
| codetrans-contest | 0.3970 | 0.4269 | +0.0299 |
| cosqa | 0.1455 | 0.1464 | +0.0009 |
| codefeedback-st | 0.3724 | 0.5243 | +0.1518 |

## Run variance

| Mode | nDCG@10 stddev | nDCG CV | Warm p95 stddev | Warm p95 CV |
| --- | ---: | ---: | ---: | ---: |
| neural | 0.0002 | 0.07% | 55.26 ms | 25.10% |

## Per-task quality

| Task | Mode | nDCG@10 | MRR@10 | R@20 |
| --- | --- | ---: | ---: | ---: |
| codetrans-dl | neural | 0.2365 | 0.1454 | 0.7796 |
| codetrans-contest | neural | 0.4269 | 0.3921 | 0.5973 |
| cosqa | neural | 0.1464 | 0.1130 | 0.3413 |
| codefeedback-st | neural | 0.5243 | 0.4897 | 0.6566 |

Variance is recorded in the machine-readable JSON as population standard deviation, coefficient of variation, minimum, and maximum.

## Interpretation

These numbers establish a reproducible baseline; they are not a state-of-the-art claim. Exact-search systems are only comparable on exact-query workloads, while this matrix evaluates code information retrieval using held-out natural-language and code-to-code queries.

## Reproduce

```bash
uv run scripts/run_public_benchmark_matrix.py \
  --profile public-core \
  --modes neural \
  --runs 3 \
  --datasets-root /tmp/ivygrep-public-datasets \
  --work-root /tmp/ivygrep-public-results \
  --output public-code-retrieval-results.json
```

Use `--modes lexical,hash,hybrid,neural` with a default-feature build to exercise every ivygrep retrieval mode. Neural runs fail if model vectors are unavailable instead of silently reporting a hash fallback.

The `full` profile contains every pinned CoIR task and language subtask. Dataset cards remain the authority for licensing; the exporter records whether a card declares a license.
