# Public code-retrieval benchmark

This report is generated from pinned public CoIR datasets. It contains no hostnames, user paths, private repository names, or source text.

- Commit: `49b1571de77ab096512469e324f76a48e4257123`
- Profile: `public-core`
- Tasks: 4
- Languages: 50
- Held-out queries: 1000
- Repetitions: 3

## Aggregate results

| Mode | nDCG@10 | MRR@10 | P@5 | R@20 | Warm p95 | Index time | Index size | Peak RSS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| lexical | 0.2278 | 0.1875 | 0.0496 | 0.4640 | 129.08 ms | 3083.90 ms | 69.36 MiB | 120.38 MiB |
| hash | 0.2324 | 0.1924 | 0.0517 | 0.4733 | 117.38 ms | 3992.10 ms | 101.84 MiB | 119.82 MiB |
| hybrid | 0.2301 | 0.1893 | 0.0506 | 0.4747 | 104.20 ms | 3360.54 ms | 101.84 MiB | 121.63 MiB |

## Run variance

| Mode | nDCG@10 stddev | nDCG CV | Warm p95 stddev | Warm p95 CV |
| --- | ---: | ---: | ---: | ---: |
| lexical | 0.0024 | 1.06% | 12.34 ms | 9.56% |
| hash | 0.0004 | 0.15% | 25.21 ms | 21.48% |
| hybrid | 0.0022 | 0.97% | 11.60 ms | 11.14% |

## Per-task quality

| Task | Mode | nDCG@10 | MRR@10 | R@20 |
| --- | --- | ---: | ---: | ---: |
| codetrans-dl | lexical | 0.1905 | 0.1192 | 0.6426 |
| codetrans-dl | hash | 0.1944 | 0.1214 | 0.6981 |
| codetrans-dl | hybrid | 0.1939 | 0.1199 | 0.6944 |
| codetrans-contest | lexical | 0.3853 | 0.3476 | 0.5656 |
| codetrans-contest | hash | 0.3970 | 0.3568 | 0.5671 |
| codetrans-contest | hybrid | 0.3982 | 0.3586 | 0.5656 |
| cosqa | lexical | 0.1466 | 0.1125 | 0.3427 |
| cosqa | hash | 0.1455 | 0.1128 | 0.3407 |
| cosqa | hybrid | 0.1406 | 0.1063 | 0.3453 |
| codefeedback-st | lexical | 0.3544 | 0.3330 | 0.5253 |
| codefeedback-st | hash | 0.3724 | 0.3563 | 0.5253 |
| codefeedback-st | hybrid | 0.3724 | 0.3563 | 0.5253 |

Variance is recorded in the machine-readable JSON as population standard deviation, coefficient of variation, minimum, and maximum.

## Interpretation

These numbers establish a reproducible baseline; they are not a state-of-the-art claim. Exact-search systems are only comparable on exact-query workloads, while this matrix evaluates code information retrieval using held-out natural-language and code-to-code queries.

## Reproduce

```bash
uv run scripts/run_public_benchmark_matrix.py \
  --profile public-core \
  --modes lexical,hash,hybrid \
  --runs 3 \
  --datasets-root /tmp/ivygrep-public-datasets \
  --work-root /tmp/ivygrep-public-results \
  --output public-code-retrieval-results.json
```

Use `--modes lexical,hash,hybrid,neural` with a default-feature build to exercise every ivygrep retrieval mode. Neural runs fail if model vectors are unavailable instead of silently reporting a hash fallback.

The `full` profile contains every pinned CoIR task and language subtask. Dataset cards remain the authority for licensing; the exporter records whether a card declares a license.
