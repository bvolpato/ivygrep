# Public learned reranker

The embedded `public-linear-reranker-v2` model is trained only from pinned public
retrieval traces. The deterministic ranker remains available with
`IVYGREP_RERANKER=deterministic`.

## Integrated result

- Commit: `76a404ec051cc9009433fe0ed021f51774b104ee`
- Binary SHA-256: `92f91629fa9a5dbad4f9f540e35703e27cc188776a763c7818cc06ee7b619a61`
- Held-out queries: 520
- nDCG@10: 0.2476 -> 0.2668 (+7.74%)
- MRR@10: 0.2009 -> 0.2212 (+10.12%)
- Warm p50 delta: +15.04 ms
- Warm p95 delta: -54.42 ms
- Acceptance gate: **PASS**

## Per-task quality

| Task | deterministic nDCG@10 | learned nDCG@10 | nDCG delta | MRR delta |
| --- | ---: | ---: | ---: | ---: |
| codetrans-dl | 0.2380 | 0.2785 | +0.0404 | +0.0359 |
| codetrans-contest | 0.4008 | 0.3937 | -0.0070 | -0.0033 |
| cosqa | 0.1521 | 0.1637 | +0.0116 | +0.0137 |
| codefeedback-st | 0.4077 | 0.4769 | +0.0692 | +0.0725 |

## Offline transfer check

The model artifact also records 1320
held-out public queries across eight tasks. It improved aggregate nDCG@10 by
+11.25% and MRR@10 by
+13.61%; every task stayed
within the two-point loss cap.

Raw evidence: [`public-reranker-results.json`](public-reranker-results.json),
[`public-reranker-deterministic-results.json`](public-reranker-deterministic-results.json),
and [`public-reranker-learned-results.json`](public-reranker-learned-results.json).
