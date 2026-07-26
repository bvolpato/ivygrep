# Memory and general retrieval A/B report, 2026-07-25

Experiments started from `3721feb`. Screening used one-variable-at-a-time
external probes against the same release binary and pinned MemoryQuest bytes.
Final product comparison used three alternating runs per arm through default
daemon-backed search. Discarded settings are not product defaults.

## Outcome

Final default uses two local probes, fetches up to 80 candidates per probe,
weights original query by `1.25` during reciprocal-rank fusion, and limits probe
text to 512 characters. Original query remains intact.

| MemoryQuest metric | Previous default | Final default | Delta |
|---|---:|---:|---:|
| Recall@5 | 33.77% | 34.36% | +0.59 points |
| Recall@20 | 73.85% | 74.91% | +1.06 points |
| Exact@20 | 43.18% | 44.86% | +1.68 points |
| nDCG@10 | 0.42392 | 0.42731 | +0.00339 |
| MRR@10 | 0.50323 | 0.51283 | +0.00960 |
| Warm p95, three-run median | 94.33 ms | 86.05 ms | -8.8% |
| Peak daemon RSS | 354.5 MB | 354.8 MB | +0.1% |

Final p95 is 5.22 ms above fully client-parallel four-probe diagnostic
(`80.84 ms`) without adding client/daemon round trips. p50 rose from 14.52 to
16.25 ms because deeper probe candidates do more work on expanded queries.

MemoryQuest users 30-49 formed a diagnostic user split after initial screening.
Winner improved held-out recall@20 from 71.85% to 74.02%, exact@20 from 39.43%
to 42.68%, recall@5 from 33.43% to 34.04%, nDCG@10 from 0.41844 to 0.42397,
and MRR@10 from 0.50138 to 0.50956. This is not a pristine blind test: all
MemoryQuest queries were available during earlier feature development.

## Decisions

| Idea | Best result | Decision |
|---|---|---|
| Probe subset | Context + action matched three-probe recall while cutting screening p95 from 104.75 to 66.43 ms. Held-out recall and exact improved. | Keep. Remove history probe. |
| Probe depth | Depth 80 reached 75.09% recall@20, 45.23% exact@20, 34.25% recall@5, and 0.51149 MRR. | Keep 80. Depth 40 improved deep recall but lost top-five quality. |
| Original-query anchor | Weight 1.25 reached 74.21% recall@20 and 44.11% exact@20. Weight 1.5 regressed. | Keep 1.25. Discard 1.15 and 1.5. |
| Combined two-probe candidate | Context + action, depth 80, weight 1.25 reached 75.00% recall@20 and 44.86% exact@20 in external screening. | Keep. Best relevance/latency balance. |
| Generic retrieval prompts | 72.27% recall@20 and 39.07% exact@20. | Discard. Large deep-recall regression. |
| RRF constant | `20`, `40`, and `100` did not improve baseline quality. | Discard. Keep `60`. |
| Three probes at depth 80 | 75.20% recall@20 and 45.79% exact@20, but 122.37 ms p95 and worse held-out balance than two probes. | Discard. Marginal deep gain costs latency. |
| Disable expansion for long queries | StackOverflow QA fell to 0.53926 nDCG, 0.48754 MRR, and 71% recall@20. | Discard. Long questions still benefit from semantic probes. |
| Probe text cap | 512 characters reached 0.56098 nDCG, 0.52015 MRR, and 75% recall@20 on StackOverflow QA. | Keep 512. Discard 256 for relevance loss and 1024 for weaker quality. |

## General-retrieval controls

Two deterministic, untuned 100-query samples check that MemoryQuest gains do
not depend on personal-memory phrasing. CodeSearchNet-go covers semantic code
retrieval. StackOverflow QA covers long natural-language questions and
answer-like text. Both use 5,000 documents.

| Dataset | Metric | Previous default | Final default |
|---|---|---:|---:|
| CodeSearchNet-go | nDCG@10 | 0.61976 | 0.64790 |
| CodeSearchNet-go | MRR@10 | 0.58260 | 0.60518 |
| CodeSearchNet-go | Recall@20 | 79% | 82% |
| CodeSearchNet-go | Warm p95 | 194.07 ms | 129.41 ms |
| StackOverflow QA | nDCG@10 | 0.56064 | 0.56098 |
| StackOverflow QA | MRR@10 | 0.51543 | 0.52015 |
| StackOverflow QA | Recall@20 | 73% | 75% |
| StackOverflow QA | Warm p95 | 3,483.13 ms | 1,355.67 ms |

StackOverflow queries are unusually long: median 928 characters, p95 4,131,
maximum 10,386. Limiting only generated probe text removes repeated long-input
work while preserving full original-query retrieval. This control supports a
general retrieval optimization, not a MemoryQuest-specific prompt shortcut.

## Product correctness

- Fused results now obey requested file limit across CLI, MCP, Web, and TUI.
- Failed or empty probes preserve original scores and provenance.
- Daemon protocol version advances from 4 to 5 for new search wire behavior.
- Probe truncation uses UTF-8-safe character boundaries.

## Reproduction

```bash
uv run scripts/eval_code_retrieval.py \
  --dataset /tmp/ivygrep-memoryquest \
  --binary target/release/ig \
  --mode blended \
  --query-expansion memory-context-action \
  --query-expansion-workers 4 \
  --probe-limit 80 \
  --probe-query-chars 512 \
  --original-weight 1.25 \
  --output /tmp/memoryquest-candidate.json
```

Public-control datasets use `scripts/export_public_retrieval.py` with tasks
`CodeSearchNet-go` and `stackoverflow-qa`, 100 queries, 5,000 documents, and
seed `20260725`. Leakage checker passed across both controls and MemoryQuest.

## Remaining work

- Single-pipeline classification and batched probe embedding remain promising,
  but current change reaches near-client-parallel p95 with smaller risk.
- Broad arbitrary-notes claim still needs genuine non-code authored-note corpus.
- Future tuning needs frozen user-level development and blind test partitions.
