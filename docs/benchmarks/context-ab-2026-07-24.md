# Context retrieval A/B report, 2026-07-24

Each retrieval candidate started from `71676d2` and used the same 12 frozen tasks, clean parent worktrees, hash vectors, and 8,000-token budget. Discarded candidates were removed before testing the next candidate. The final candidate stacks only kept changes.

```bash
uv run scripts/bench_context_retrieval.py \
  --binary target/release/ig \
  --repo . \
  --tasks-from tests/fixtures/context_retrieval_tasks.json \
  --output /tmp/context-results.json \
  --modes context
```

## Decisions

| Candidate | Mean recall | Primary recall | Test recall | Mean estimated tokens | Recall / 1K tokens | Covered roles | p50 | Decision |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| Baseline | .75000 | .81944 | .000 | 4,402.8 | .16484 | 7.00 | 139 ms | Control |
| Expand graph from 12 to 24 files | .75000 | .81944 | .000 | 4,602.6 | .15868 | 7.17 | 261 ms | Discard. More work and tokens found neither missing test. |
| Test-aware graph and anchor retrieval | .81944 | .81944 | 1.000 | 4,450.3 | .17811 | 7.25 | 158 ms | Keep. Both labeled test files recovered; no task regressed. |
| Score/token packing with 15% file novelty | .75000 | .75000 | 1.000 | 3,934.8 | .18646 | 7.25 | 138 ms | Discard. Saved 11.6% tokens but lost primary files and 6.94 recall points. |
| Always-on raw co-change | .81944 | .81944 | 1.000 | 4,597.7 | .17670 | 7.58 | 166 ms | Discard. No recall gain; 3.3% more tokens and extra Git work. |
| Always-on Jaccard-ranked co-change | .81944 | .81944 | 1.000 | 4,595.2 | .17754 | 7.33 | 177 ms | Discard. Normalization removed some noise but did not improve quality. |
| Token estimator calibrated to current agent tokenizers | .81944 | .81944 | 1.000 | 3,194.9 | .25033 | 7.25 | 153 ms | Keep. Same selected-file quality with more useful source inside the real budget. |
| Raise post-role target from 12 to 14 | .81944 | .81944 | 1.000 | 3,744.8 | .21275 | 7.25 | 146 ms | Keep. Debug and release repeats restored primary non-inferiority. |

Latency is diagnostic only. These short end-to-end runs include temporary worktree creation, indexing, process startup, and filesystem noise.
`Recall / 1K tokens` is the mean of per-task recall-to-token ratios, not aggregate
mean recall divided by aggregate mean tokens.

## Test-aware retrieval

Baseline missed `tests/incremental_crud.rs` and `tests/web_server.rs`. The kept implementation adds two independent signals:

1. Direct test-file dependencies from primary graph seeds. Candidate tests rank by task overlap in their path, number of primary relationships, and representative-hit score.
2. A bounded post-anchor query for black-box tests which have no static import edge to implementation files.

`filtered-search-indexing` recall rose from `.6667` to `1.0`. `web-result-exploration` recall rose from `.5` to `1.0`. Other task recall stayed unchanged. CI now requires test-role recall `1.0` on the frozen fixture.

## Token calibration

The original deterministic estimator was compared with `o200k_base` and `cl100k_base` over 30 context packs produced from recent implementation tasks.

| Tokenizer | Baseline estimate / actual, median | Baseline p05-p95 | Calibrated median | Calibrated p05-p95 | Underestimates |
|---|---:|---:|---:|---:|---:|
| `o200k_base` | 1.817 | 1.790-1.856 | 1.092 | 1.074-1.120 | 0 / 30 |
| `cl100k_base` | 1.826 | 1.808-1.866 | 1.096 | 1.080-1.125 | 0 / 30 |

The estimator now scales its code-aware raw count by `3/5`. This leaves at least 7.3% measured headroom while reducing systematic overestimation. Frozen task recall and selected paths stayed unchanged; snippets can use more of the requested real model budget.

## Post-composition target check

Final composition exposed retrieval-graph variance which could crowd one expected primary file out of a 12-item pack. Raising the post-role target to 14 recovered overall and primary recall from `.79167` and `.77778` to `.81944` in two identical debug runs and two release confirmations. Release mean estimated tokens ranged from 3,708.4 to 3,744.8, 14.7%-15.9% above the 3,232.4 target-12 result, and remained below both the 8,000-token request and the pre-calibration 4,402.8 estimate. Latest recall per 1,000 estimated tokens remained 29.1% above baseline.

## Evaluation fixture scope

The benchmark still defaults to labels derived from changed paths. That label source can include incidental files and omit relevant unchanged evidence. Fixture schema now also accepts `"label_source": "curated"`, which validates expected paths against the pinned base tree without requiring them to be changed by a child commit. This enables externally annotated multi-repository suites without weakening historical-task validation.

## Validation

- All 640 Rust library tests passed.
- 7 Python benchmark tests passed, including curated labels and test-recall gate coverage.
- Frozen context suite passed the new test-recall `1.0` gate.
- Full-feature Clippy with warnings denied passed.
- `cargo fmt --all -- --check` and `git diff --check` passed.
