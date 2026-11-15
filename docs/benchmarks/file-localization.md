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

To measure default routing after neural enhancement, use a default-feature
build and run:

```sh
uv run scripts/bench_file_localization.py \
  --tasks benchmarks/public/file_localization_tasks.jsonl \
  --binary target/release/ig \
  --cache-dir /tmp/ivygrep-file-localization-cache \
  --mode blended --enhance-neural \
  --limit 30 --timeout-secs 120 --max-query-chars 2000 \
  --output docs/benchmarks/file-localization-default-neural-$(date +%F).json
```

This prepares neural vectors and leaves query routing at its default. It does
not force every search to use neural retrieval. The harness does not record
neural execution for each query.

Each task is materialized as a clean standalone Git repository (snapshot of
`base_commit` committed into a fresh repo) with its own `IVYGREP_HOME`, so
`ig context` sees an empty change scope and no worktree overlay is involved.
Repository clones are cached in `--cache-dir` as blobless partial clones.

The benchmark needs network access to clone repositories and is not part of
CI. Run it manually before relevance-affecting changes and commit the JSON
under `docs/benchmarks/`.

## Results

Latest runs: [hash and lexical](file-localization-hash-2026-11-15.json) and
[default routing with neural vectors](file-localization-default-neural-2026-11-15.json).
Both used `ivygrep 1.3.5`, clean source commit
`7c9fd25e1aecb3d5c135bc28e5e612b264dfd20f`, and frozen binary SHA-256
`7925d392876e2041e2d58af5272d8f5896711c2df6527153cbce8bb197d146c6`.
Each mode scored all 30 tasks without errors or timeouts, using 8,000-token
packs and a requested file limit of 50. The task file SHA-256 is
`0eb0e0d1585f4bb159d7cff4a944b8bf9cf756bb1ba480b0f8ea444ec51a2006`.
The task set and harness are unchanged from the November 5 run.

The default-routing run prepared vectors with the default
`potion-code-16m-v2` model. It did not pass `--force-neural`. These are single
runs on Linux x86_64 while a separate build was active. The recorded times
are not a controlled performance comparison.

| Mode | Tasks | Acc@1 | Acc@5 | Acc@10 | R@10 | R@20 | MRR | Pack R | Pack P | Pack hit |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| hash | 30/30 | 0.433 | 0.700 | 0.700 | 0.614 | 0.614 | 0.548 | 0.675 | 0.090 | 0.767 |
| lexical | 30/30 | 0.433 | 0.700 | 0.700 | 0.597 | 0.614 | 0.543 | 0.675 | 0.090 | 0.767 |
| default, neural vectors prepared | 30/30 | 0.467 | 0.700 | 0.700 | 0.614 | 0.614 | 0.565 | 0.675 | 0.091 | 0.767 |

Default routing finds a gold file first on one more task than hash or lexical
retrieval. All three modes tie on Acc@5, Acc@10, Recall@20, pack recall, and
pack hit. This sample does not establish a general retrieval-quality gain.
Context packs average about 4,172 tokens and contain a gold file for 23 of 30
tasks. Seven issue texts quote a gold file path verbatim, such as a traceback
or module reference; those references remain in the queries.

Per language:

| Language | Tasks | Acc@5, all modes | hash MRR | lexical MRR | default MRR | Pack R, all modes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| python | 12 | 0.833 | 0.646 | 0.632 | 0.688 | 0.847 |
| rust | 10 | 0.400 | 0.270 | 0.270 | 0.270 | 0.308 |
| typescript | 8 | 0.875 | 0.750 | 0.750 | 0.750 | 0.875 |

Rust scores remain lower than Python and TypeScript in this sample.

### Comparison with the previous report

Against the [November 5 v1.3.1 report](file-localization-hash-2026-11-05.json),
hash Acc@1 rises from 0.367 to 0.433 and MRR from 0.524 to 0.548, but Acc@5
falls from 0.767 to 0.700 and Recall@20 from 0.706 to 0.614. Hash and lexical
pack recall and pack hit are unchanged.

The two lost top-five matches are `pallets-click-3152` and `sharkdp-fd-1914`.
Gold-file coverage also falls on `sharkdp-fd-1917` and `sharkdp-fd-1805`.
A check of those four tasks using the published v1.3.4 Linux CUDA archive
returned exactly the same ranked files, pack files, and metrics as v1.3.5 in
both hash and lexical modes. These losses already occur in v1.3.4; this
release does not change the runtime search source.

The earlier command sent an unlimited request to retrieval before grouping
hits into files. [PR #478](https://github.com/bvolpato/ivygrep/pull/478),
included in v1.3.4, passes an explicit filename limit through to retrieval.
On the two lost tasks, a separate lexical check with `--no-limit` instead of
`-n 50` restores a gold file at rank 4 and rank 5 respectively. This confirms
that the retrieval limit affects these results. The main reports retain the
frozen `-n 50` protocol and include the losses; no ranking changes were made
to fit this task set.

The 2026-08-21 report used `ivygrep 1.2.12`, a dirty checkout, and blended
mode. Although it used the same task-file hash, its binary and retrieval route
differ, so it is not a paired comparison. The three current modes use the
same frozen binary, task set, queries, and context budget; the default-routing
run additionally builds neural vectors.

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
- Hash and lexical results do not use neural vectors. The default-routing
  report prepares neural vectors but does not establish neural execution for
  every query.
- The requested file limit can affect candidate retrieval and final file
  ranking. Results with `--no-limit` are a different measurement.
- Each mode ran once. Latencies reflect shared-host load and are not evidence
  of a speed improvement or regression.
