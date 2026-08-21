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
  --mode blended --baseline lexical \
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

Latest committed run: `docs/benchmarks/file-localization-hash-2026-08-21.json`
(`ivygrep 1.2.12`, hash-only build, 30 tasks, 8,000-token packs, 50-file lists).
With a hash-only build, `blended` routes to hash vectors + lexical fusion; no
neural vectors are involved.

| Mode | Tasks | Acc@1 | Acc@5 | Acc@10 | R@10 | R@20 | MRR | Pack R | Pack P | Pack hit |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| blended | 30/30 | 0.433 | 0.667 | 0.733 | 0.631 | 0.692 | 0.515 | 0.642 | 0.080 | 0.733 |
| lexical | 30/30 | 0.400 | 0.667 | 0.733 | 0.672 | 0.717 | 0.493 | 0.650 | 0.082 | 0.733 |
| delta | | +0.033 | +0.000 | +0.000 | -0.042 | -0.025 | +0.022 | -0.008 | -0.002 | +0.000 |

Per language (blended vs lexical):

| Language | Tasks | blended Acc@1 | blended Acc@5 | blended R@10 | blended MRR | blended Pack R | lexical Acc@5 | lexical MRR |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| python | 12 | 0.583 | 0.833 | 0.806 | 0.661 | 0.806 | 0.833 | 0.601 |
| rust | 10 | 0.200 | 0.400 | 0.375 | 0.301 | 0.258 | 0.400 | 0.299 |
| typescript | 8 | 0.500 | 0.750 | 0.688 | 0.562 | 0.875 | 0.750 | 0.573 |

Reading: `blended` on a hash-only build is hash vectors fused with lexical
retrieval, so the delta over `lexical` is small by construction (+0.033
Acc@1, +0.022 MRR, -0.042 R@10). Rust tasks are the weak spot: 5 of the
8 tasks with no gold file in the top 10 are ripgrep/fd/bat issues whose text
describes runtime behaviour rather than code identifiers. Context packs
averaged 4,230 tokens and contained a gold file for 73% of tasks.
Seven of the 30 issue texts quote a gold file path verbatim (tracebacks or
module references), which is typical of real issues and is left in place.

Run-to-run variance: a preceding run of the same binary and harness on the
same tasks (three queries differed only in how reporter home directories were
redacted) scored blended Acc@5 0.700 and MRR 0.516 versus 0.667 and 0.515
here. ANN index construction is not bit-reproducible, so expect one task to
move on Acc@k between runs; treat differences below 0.05 as noise.

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
