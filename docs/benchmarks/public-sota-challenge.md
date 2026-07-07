# Public code-retrieval benchmark

This report is generated from pinned public CoIR datasets. It contains no hostnames, user paths, private repository names, or source text.

- Commit: `e5e9741ffa35ec939271b39858d10f6d0eee800b`
- Profile: `sota-challenge`
- Tasks: 6
- Languages: 3
- Held-out queries: 600
- Repetitions: 1
- Query text limit: 2048 characters

## Aggregate results

| Mode | nDCG@10 | MRR@10 | P@5 | R@20 | Warm p95 | Index time | Index size | Peak RSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| hash | 0.5895 | 0.5586 | 0.1287 | 0.6950 | 657.48 ms | 756.83 ms | 45.46 MiB | 228.99 MiB |
| hybrid | 0.5895 | 0.5586 | 0.1287 | 0.6950 | 662.04 ms | 729.61 ms | 45.45 MiB | 228.28 MiB |
| neural | 0.5950 | 0.5654 | 0.1300 | 0.6967 | 660.33 ms | 721.80 ms | 65.56 MiB | 322.19 MiB |

## Run variance

| Mode | nDCG@10 stddev | nDCG CV | Warm p95 stddev | Warm p95 CV |
| --- | ---: | ---: | ---: | ---: |
| hash | 0.0000 | 0.00% | 0.00 ms | 0.00% |
| hybrid | 0.0000 | 0.00% | 0.00 ms | 0.00% |
| neural | 0.0000 | 0.00% | 0.00 ms | 0.00% |

## Per-task quality

| Task | Mode | nDCG@10 | MRR@10 | R@20 |
| --- | --- | ---: | ---: | ---: |
| stackoverflow-qa | hash | 0.5829 | 0.5231 | 0.7700 |
| stackoverflow-qa | hybrid | 0.5829 | 0.5231 | 0.7700 |
| stackoverflow-qa | neural | 0.5988 | 0.5458 | 0.7700 |
| apps | hash | 0.0033 | 0.0014 | 0.0200 |
| apps | hybrid | 0.0033 | 0.0014 | 0.0200 |
| apps | neural | 0.0033 | 0.0014 | 0.0200 |
| codefeedback-mt | hash | 0.5657 | 0.5112 | 0.7600 |
| codefeedback-mt | hybrid | 0.5657 | 0.5112 | 0.7600 |
| codefeedback-mt | neural | 0.5788 | 0.5243 | 0.7600 |
| synthetic-text2sql | hash | 0.8633 | 0.8390 | 0.9500 |
| synthetic-text2sql | hybrid | 0.8633 | 0.8390 | 0.9500 |
| synthetic-text2sql | neural | 0.8607 | 0.8322 | 0.9600 |
| CodeSearchNet-python | hash | 0.9400 | 0.9400 | 0.9400 |
| CodeSearchNet-python | hybrid | 0.9400 | 0.9400 | 0.9400 |
| CodeSearchNet-python | neural | 0.9400 | 0.9400 | 0.9400 |
| CodeSearchNet-java | hash | 0.5816 | 0.5370 | 0.7300 |
| CodeSearchNet-java | hybrid | 0.5816 | 0.5370 | 0.7300 |
| CodeSearchNet-java | neural | 0.5885 | 0.5488 | 0.7300 |

Variance is recorded in the machine-readable JSON as population standard deviation, coefficient of variation, minimum, and maximum.

## Interpretation

These numbers establish a reproducible baseline; they are not a state-of-the-art claim. Exact-search systems are only comparable on exact-query workloads, while this matrix evaluates code information retrieval using held-out natural-language and code-to-code queries.

## Reproduce

```bash
uv run scripts/run_public_benchmark_matrix.py \
  --profile sota-challenge \
  --modes hash,hybrid,neural \
  --runs 1 \
  --datasets-root /tmp/ivygrep-public-datasets \
  --work-root /tmp/ivygrep-public-results \
  --max-query-chars 2048 \
  --output public-sota-challenge-results.json
```

Use `--modes lexical,hash,hybrid,neural` with a default-feature build to exercise every ivygrep retrieval mode. Neural runs fail if model vectors are unavailable instead of silently reporting a hash fallback.

The `full` profile contains every pinned CoIR task and language subtask. Dataset cards remain the authority for licensing; the exporter records whether a card declares a license.
