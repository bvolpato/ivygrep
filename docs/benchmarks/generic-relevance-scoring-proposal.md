# Generic Relevance Scoring Proposal

## Problem

The Linux benchmark is useful as a stress case, but scorer changes must work across arbitrary codebases. Domain-specific aliases such as `block io -> blk_mq` or `cpu frequency -> cpufreq` make one corpus look better while damaging generality.

## Approach

Use repository-shape signals that apply across codebases:

- Rank implementation code above support files when the query does not ask for docs, tests, tools, scripts, examples, or fixtures.
- Treat selftests as tests, not primary implementation.
- Penalize deep paths with weak query overlap, so a wrapper or example buried far from the repo root does not beat a direct implementation module.
- Keep support sources eligible when query intent explicitly asks for them.
- Apply a softer chunk-density penalty to implementation source files, because large implementation modules should not disappear solely because they have many chunks.

This is deliberately not a semantic dictionary. It does not know Linux subsystem names, function names, or file names.

## Mechanical Result

Command:

```bash
uv run scripts/bench_linux_relevance.py --kernel /home/bruno/githubworkspace/linux --bench-home /tmp/ivygrep-linux-bench-home --skip-index --skip-build
```

Result after rebuilding `target/release/ig` from this branch:

| Metric | Baseline `96b5a1d` | This branch |
| --- | ---: | ---: |
| `linux_relevance_score` | `3.5167` | `6.3915` |
| `spam_top10_rate` | `0.3462` | `0.1385` |
| `forbidden_top3_rate` | `0.3077` | `0.1795` |
| `mean_recall@20` | `0.0641` | `0.2051` |

The gain is smaller than the rejected Linux-specific run, but the mechanism is portable: role-aware authority, query intent, and path-depth scoring.

## Guardrails

- No benchmark query IDs in scorer code.
- No Linux path names in scorer code.
- No Linux subsystem aliases in scorer code.
- Docs/tests/tools/examples still rank when the query asks for them.
- Synthetic relevance tests cover generic support-path demotion.
