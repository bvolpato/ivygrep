# Public code-retrieval benchmark

Pinned public CoIR datasets. Report excludes hostnames, user paths, private repository names, and source text.

- Commit: `82686c6ff6e6030a08365ac30a3a51a541cab9a7`
- Profile: `sota-challenge`
- Tasks: 6
- Languages: 3
- Held-out queries: 600
- Repetitions: 3
- Query text limit: 2048 characters

## Aggregate results

| Mode | nDCG@10 | MRR@10 | P@5 | R@20 | Warm p95 | Index time | Index size | Peak RSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| hash | 0.5900 | 0.5582 | 0.1287 | 0.7000 | 688.94 ms | 899.58 ms | 45.43 MiB | 229.03 MiB |
| hybrid | 0.5896 | 0.5578 | 0.1287 | 0.7000 | 722.46 ms | 800.66 ms | 45.45 MiB | 228.79 MiB |
| blended | 0.5955 | 0.5652 | 0.1300 | 0.7000 | 778.19 ms | 882.76 ms | 65.54 MiB | 333.95 MiB |
| neural | 0.5963 | 0.5661 | 0.1303 | 0.7000 | 798.38 ms | 932.02 ms | 65.55 MiB | 322.61 MiB |

## Run variance

| Mode | nDCG@10 stddev | nDCG CV | Warm p95 stddev | Warm p95 CV |
| --- | ---: | ---: | ---: | ---: |
| hash | 0.0000 | 0.00% | 24.00 ms | 3.48% |
| hybrid | 0.0004 | 0.08% | 74.31 ms | 10.29% |
| blended | 0.0003 | 0.05% | 51.98 ms | 6.68% |
| neural | 0.0000 | 0.00% | 24.66 ms | 3.09% |

## Per-task quality

| Task | Mode | nDCG@10 | MRR@10 | R@20 |
| --- | --- | ---: | ---: | ---: |
| stackoverflow-qa | hash | 0.5829 | 0.5231 | 0.7700 |
| stackoverflow-qa | hybrid | 0.5810 | 0.5206 | 0.7700 |
| stackoverflow-qa | blended | 0.5988 | 0.5458 | 0.7700 |
| stackoverflow-qa | neural | 0.5988 | 0.5458 | 0.7700 |
| apps | hash | 0.0033 | 0.0014 | 0.0200 |
| apps | hybrid | 0.0033 | 0.0014 | 0.0200 |
| apps | blended | 0.0033 | 0.0014 | 0.0200 |
| apps | neural | 0.0033 | 0.0014 | 0.0200 |
| codefeedback-mt | hash | 0.5657 | 0.5112 | 0.7600 |
| codefeedback-mt | hybrid | 0.5657 | 0.5112 | 0.7600 |
| codefeedback-mt | blended | 0.5788 | 0.5243 | 0.7600 |
| codefeedback-mt | neural | 0.5788 | 0.5243 | 0.7600 |
| synthetic-text2sql | hash | 0.8662 | 0.8366 | 0.9800 |
| synthetic-text2sql | hybrid | 0.8662 | 0.8366 | 0.9800 |
| synthetic-text2sql | blended | 0.8658 | 0.8330 | 0.9800 |
| synthetic-text2sql | neural | 0.8683 | 0.8363 | 0.9800 |
| CodeSearchNet-python | hash | 0.9400 | 0.9400 | 0.9400 |
| CodeSearchNet-python | hybrid | 0.9400 | 0.9400 | 0.9400 |
| CodeSearchNet-python | blended | 0.9400 | 0.9400 | 0.9400 |
| CodeSearchNet-python | neural | 0.9400 | 0.9400 | 0.9400 |
| CodeSearchNet-java | hash | 0.5816 | 0.5370 | 0.7300 |
| CodeSearchNet-java | hybrid | 0.5816 | 0.5370 | 0.7300 |
| CodeSearchNet-java | blended | 0.5865 | 0.5465 | 0.7300 |
| CodeSearchNet-java | neural | 0.5885 | 0.5488 | 0.7300 |

Variance is recorded in the machine-readable JSON as population standard deviation, coefficient of variation, minimum, and maximum.

## Scope

Matrix covers held-out natural-language and code-to-code retrieval. Exact-search tools require a separate exact-query workload.

## Reproduce

```bash
uv run scripts/run_public_benchmark_matrix.py \
  --profile sota-challenge \
  --modes hash,hybrid,blended,neural \
  --runs 3 \
  --datasets-root /tmp/ivygrep-public-datasets \
  --work-root /tmp/ivygrep-public-results \
  --max-query-chars 2048 \
  --output public-sota-challenge-results.json
```

Use `--modes lexical,hash,hybrid,blended,neural` with a default-feature build to exercise every retrieval mode. `blended` measures normal production routing with neural vectors available; `neural` forces neural retrieval and fails if vectors are unavailable.

The `full` profile contains every pinned CoIR task and language subtask. Dataset cards remain the authority for licensing; the exporter records whether a card declares a license.
