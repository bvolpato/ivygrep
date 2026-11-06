# Repository file-localization benchmark

Issue text in, files the fix touched out. This is the metric that matters for a
coding-agent context engine: given a real bug report or feature request, does
`ig` surface the files a human ended up editing? It follows the File Acc@k /
Recall@k / MRR protocol used by LocAgent and SweRank on SWE-bench Lite, but on
small permissively licensed repositories so a full run takes minutes on a
laptop and needs no model download.

Why this exists: the self-repository fixtures cannot distinguish a retriever
from a prior. On `tests/fixtures/context_retrieval_tasks.json` the five most
frequent gold files returned as a constant answer score mean recall 0.78, which
cleared the old `--min-context-recall 0.70` floor without reading the task.
The workflow now scores that constant baseline next to ivygrep
(`scripts/bench_context_retrieval.py --baseline constant-topk`) so the gap is
visible in every run; `--min-margin-over-baseline` can enforce it once the
fixture is large enough to make the margin stable (on 12 tasks, one task is
0.083 recall and ANN build variance moved context recall between 0.771 and
0.903 on the same commit). This benchmark adds an external, repository-level
measurement that a constant answer cannot pass.

## Task file

`benchmarks/public/file_localization_tasks.jsonl`, one JSON object per line:

| Field | Meaning |
| --- | --- |
| `task_id` | `<owner>-<repo>-<pr>` |
| `repo` | public clone URL (local paths also accepted by the harness) |
| `base_commit` | merge parent of the fix: the tree the issue was filed against |
| `query` | issue title + first 1,500 characters of the issue body (template comments stripped, reporter home directories redacted) |
| `gold_files` | files the merged fix changed, excluding tests, docs, lockfiles, and files the fix created (they do not exist at `base_commit`) |
| `also_changed` | every other file the fix touched, with the exclusion reason |
| `source_url`, `issue_url`, `merge_commit`, `merge_strategy`, `repo_license` | provenance |

Curation procedure (all read-only GitHub API + git, no hand-written labels):

1. List merged pull requests with `closingIssuesReferences` via GraphQL for
   each repository; keep PRs whose linked issue has a body of at least 80
   characters and whose non-test/doc change set is 1-6 files.
2. Re-fetch the issue and PR through the REST API to confirm the issue exists
   as an issue (not a discussion or PR) and the PR is merged.
3. Clone the repository, locate the merge commit, and find the base commit
   whose `git diff --name-only base merge` equals the PR's file list
   (two-parent merge, squash, or `merge~k` for rebase merges).
4. Drop gold files that do not exist at `base_commit`; record them in
   `also_changed` as `added_in_fix`.

Repositories in the current set (30 tasks): `pallets/click`, `pallets/flask`,
`psf/requests`, `fastapi/typer` (Python); `BurntSushi/ripgrep`, `sharkdp/fd`,
`sharkdp/bat` (Rust); `colinhacks/zod`, `sindresorhus/ky`, `yargs/yargs`
(TypeScript). Licenses: MIT, BSD-3-Clause, Apache-2.0, Unlicense. Only issue
text and file paths are stored; no source code from those repositories is
committed here.

## Metrics

Search surface: `ig --json --file-name-only -n 50 "<query>"` on the
`base_commit` checkout.

- **File Acc@k** (k = 1, 5, 10): 1 if any gold file is in the top k unique
  files, else 0. This is the LocAgent / SweRank file-level accuracy.
- **Recall@k** (k = 10, 20): fraction of gold files in the top k.
- **MRR**: reciprocal rank of the first gold file.

Context-pack surface: `ig context "<query>" --json --budget 8000`.

- **Pack recall / precision**: gold coverage and gold share of the pack's file
  set.
- **Pack hit**: 1 if the pack contains at least one gold file.

Aggregates are means over scored tasks; per-language breakdowns and every
per-task row (ranked files, pack files, latencies, truncation flag) are in the
JSON report. Tasks whose `ig` call fails or times out are reported as failures
and excluded from means, never silently scored as zero.

## Running

```sh
cargo build --release --no-default-features   # hash-only build, no model download
uv run scripts/bench_file_localization.py \
  --tasks benchmarks/public/file_localization_tasks.jsonl \
  --binary target/release/ig \
  --cache-dir /tmp/ivygrep-file-localization-cache \
  --mode hash --baseline lexical \
  --limit 30 --timeout-secs 120 --max-query-chars 2000 \
  --output docs/benchmarks/file-localization-hash-$(date +%F).json
```

`--mode hash|lexical|blended` selects `--hash`, `--lexical-only`, or the
default route; `--force-neural` forces neural retrieval for search queries and
`--enhance-neural` builds neural vectors first (both need a neural-capable
binary). `--baseline lexical` reruns every query with `--lexical-only` and
prints a delta row. Optional gates: `--min-acc-at-5` and
`--min-margin-over-baseline` (on Acc@5).

Each task is materialized as a clean standalone Git repository (snapshot of
`base_commit` committed into a fresh repo) with its own `IVYGREP_HOME`, so
`ig context` sees an empty change scope and no worktree overlay is involved.
Repository clones are cached in `--cache-dir` as blobless partial clones.

The benchmark needs network access to clone repositories and is not part of
CI. Run it manually before relevance-affecting changes and commit the JSON
under `docs/benchmarks/`.

## Results

Latest run: [`file-localization-hash-2026-11-05.json`](file-localization-hash-2026-11-05.json)
(`ivygrep 1.3.1`, source commit `f898466`, frozen binary SHA-256
`a092363f49d2c70a17fed221825eb46f1b954e4fe3fc68000a57a7aed1c06074`, 30
tasks, 8,000-token packs, 50-file lists). The task file SHA-256 is
`0eb0e0d1585f4bb159d7cff4a944b8bf9cf756bb1ba480b0f8ea444ec51a2006`.

| Mode | Tasks | Acc@1 | Acc@5 | Acc@10 | R@10 | R@20 | MRR | Pack R | Pack P | Pack hit |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| hash | 30/30 | 0.367 | 0.767 | 0.767 | 0.706 | 0.706 | 0.524 | 0.675 | 0.090 | 0.767 |
| lexical | 30/30 | 0.367 | 0.767 | 0.767 | 0.689 | 0.706 | 0.522 | 0.675 | 0.090 | 0.767 |
| delta | | +0.000 | +0.000 | +0.000 | +0.017 | +0.000 | +0.003 | +0.000 | +0.000 | +0.000 |

Per language (hash vs lexical):

| Language | Tasks | hash Acc@1 | hash Acc@5 | hash R@10 | hash MRR | hash Pack R | lexical Acc@5 | lexical MRR |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| python | 12 | 0.417 | 0.917 | 0.847 | 0.614 | 0.847 | 0.917 | 0.607 |
| rust | 10 | 0.100 | 0.500 | 0.400 | 0.253 | 0.308 | 0.500 | 0.253 |
| typescript | 8 | 0.625 | 0.875 | 0.875 | 0.729 | 0.875 | 0.875 | 0.729 |

The hash and lexical routes tie on Acc@1, Acc@5, Acc@10, Recall@20, pack
recall, and pack hit. Hash has small differences on Recall@10 (+0.017) and
MRR (+0.003), which do not establish a general retrieval-quality improvement.
Rust scores are lower than Python and TypeScript in this sample. Context packs
averaged 4,172 tokens and contained a gold file for 77% of tasks. Seven of the
30 issue texts quote a gold file path verbatim (tracebacks or module
references), which is typical of real issues and is left in place.

The 2026-08-21 report used `ivygrep 1.2.12`, a dirty checkout, and blended
mode. Although it used the same task-file hash, its binary and retrieval route
differ, so it is not a paired comparison. The hash and lexical results above
use the same frozen binary, task set, queries, and context budget.

Reference point for scale only (different corpus and task set, not
comparable): the LocAgent and SweRank papers report a BM25 file-level Acc@5 of
about 0.62 on SWE-bench Lite.

## Limitations

- 30 tasks from 10 small or medium repositories; confidence intervals are wide
  and one task moves Acc@k by 0.033.
- Gold files are what one merged fix changed. Alternative valid fixes exist, so
  misses are not always retrieval errors.
- Issue bodies are truncated to 1,500 characters at curation and 2,000 at
  query time; long reproduction logs are cut.
- Hash-only results measure the deterministic route. Neural routes need a
  default-feature build and `--enhance-neural`.
