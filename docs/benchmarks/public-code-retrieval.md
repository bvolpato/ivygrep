# Public code-retrieval benchmark

Pinned public CoIR datasets. Report excludes hostnames, user paths, private repository names, and source text.

- Commit: `82686c6ff6e6030a08365ac30a3a51a541cab9a7`
- Profile: `public-core`
- Tasks: 4
- Languages: 50
- Held-out queries: 1000
- Repetitions: 3

## Aggregate results

| Mode | nDCG@10 | MRR@10 | P@5 | R@20 | Warm p95 | Index time | Index size | Peak RSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| blended | 0.2625 | 0.2167 | 0.0605 | 0.4683 | 238.82 ms | 1256.08 ms | 91.14 MiB | 373.34 MiB |
| neural | 0.2633 | 0.2178 | 0.0608 | 0.4690 | 242.35 ms | 1265.64 ms | 91.14 MiB | 366.00 MiB |

## Change from frozen baseline

Baseline commit `49b1571de77ab096512469e324f76a48e4257123` mode `hash` is compared with current mode `neural`.

| Metric | Baseline | Current | Relative change |
| --- | ---: | ---: | ---: |
| nDCG@10 | 0.2324 | 0.2633 | +13.31% |
| MRR@10 | 0.1924 | 0.2178 | +13.21% |
| P@5 | 0.0517 | 0.0608 | +17.68% |
| R@20 | 0.4733 | 0.4690 | -0.92% |

| Task | Baseline nDCG@10 | Current nDCG@10 | Absolute change |
| --- | ---: | ---: | ---: |
| codetrans-dl | 0.1944 | 0.2290 | +0.0346 |
| codetrans-contest | 0.3970 | 0.3778 | -0.0191 |
| cosqa | 0.1455 | 0.1505 | +0.0050 |
| codefeedback-st | 0.3724 | 0.6394 | +0.2670 |

## Run variance

| Mode | nDCG@10 stddev | nDCG CV | Warm p95 stddev | Warm p95 CV |
| --- | ---: | ---: | ---: | ---: |
| blended | 0.0004 | 0.16% | 1.77 ms | 0.74% |
| neural | 0.0008 | 0.29% | 2.97 ms | 1.22% |

## Per-task quality

| Task | Mode | nDCG@10 | MRR@10 | R@20 |
| --- | --- | ---: | ---: | ---: |
| codetrans-dl | blended | 0.2259 | 0.1396 | 0.5648 |
| codetrans-dl | neural | 0.2290 | 0.1435 | 0.5722 |
| codetrans-contest | blended | 0.3777 | 0.3392 | 0.5068 |
| codetrans-contest | neural | 0.3778 | 0.3393 | 0.5068 |
| cosqa | blended | 0.1501 | 0.1127 | 0.3633 |
| cosqa | neural | 0.1505 | 0.1135 | 0.3620 |
| codefeedback-st | blended | 0.6394 | 0.6083 | 0.7374 |
| codefeedback-st | neural | 0.6394 | 0.6083 | 0.7374 |

Variance is recorded in the machine-readable JSON as population standard deviation, coefficient of variation, minimum, and maximum.

## Scope

Matrix covers held-out natural-language and code-to-code retrieval. Exact-search tools require a separate exact-query workload.

## Reproduce

```bash
uv run scripts/run_public_benchmark_matrix.py \
  --profile public-core \
  --modes blended,neural \
  --runs 3 \
  --datasets-root /tmp/ivygrep-public-datasets \
  --work-root /tmp/ivygrep-public-results \
  --output public-code-retrieval-results.json
```

Use `--modes lexical,hash,hybrid,blended,neural` with a default-feature build to exercise every retrieval mode. `blended` measures normal production routing with neural vectors available; `neural` forces neural retrieval and fails if vectors are unavailable.

The `full` profile contains every pinned CoIR task and language subtask. Dataset cards remain the authority for licensing; the exporter records whether a card declares a license.
