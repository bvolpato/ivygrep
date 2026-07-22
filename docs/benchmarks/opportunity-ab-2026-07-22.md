# Optimization A/B report, 2026-07-22

This report records independent experiments run from ivygrep `41a6017`, with one candidate changed at a time before composing winners. Performance trials used isolated homes and worktrees. Relevance candidates passed a 100-query screen before full public-core confirmation. Neutral results were discarded.

## Kept changes

| Area | Candidate | Baseline | Candidate | Decision |
|---|---|---:|---:|---|
| Context evaluation | Frozen tasks on clean pre-change trees | Current checkout included future and dirty files | 12 pinned tasks, exact parent trees, quality gates | Keep. Removes benchmark leakage and makes failures actionable. |
| Context selection | 400-token snippet cap | 600 tokens | Same `.7500` recall, 11.5% fewer tokens | Keep. |
| Context selection | Role-aware 12-item target | Up to 20 items | Same recall and 7.0 covered roles, 16.1% fewer tokens | Keep. |
| Context composition | Cap plus role-aware target | 6,703 mean tokens | 4,412 mean tokens, 34.1% lower | Keep. Recall stayed `.7500`; recall per 1K tokens rose 54.6%. |
| Search relevance | Always retain hash ANN when neural runs | Hash could be skipped | Full nDCG `.263440` to `.268975` | Keep. Hash and neural evidence are complementary. |
| Neural routing | BM25 confidence fallback at score `2.0`, gap `0.25` | Neural on 987 of 1,000 queries | Neural on 470, 52.4% lower | Keep. Full MRR rose 3.21%, P@5 rose 1.98%, recall rose `.003`. |
| Clean indexing | Reuse matching indexed Git state | Full discovery and Merkle reconciliation | 100K no-op mean 616 ms to 494 ms | Keep. 19.8% faster and peak RSS about 56% lower. |
| Public evidence | Current-release million-chunk snapshot | Prominent v0.10.1 results | Three v1.2.6 trials | Keep. Median warm CLI p95 13.73 ms, 159,649 chunks/s, 0.42 GiB index. |
| Evidence freshness | Package-version and median consistency test | No stale-evidence guard | Release mismatch fails benchmark tooling tests | Keep. |
| Install docs | Advertise only active channels | Two unavailable commands | Zero unavailable commands | Keep. WinGet remains pending; crates.io index remains absent. |
| Onboarding | One search plus one context command | Five unrelated quick-start steps | Two product-defining steps | Keep. |
| Hero | Functional workflow asset | Search-only 5.2 MB PNG | Search plus context 4.6 KB SVG | Keep. Social PNG also fell from 873 KB to 301 KB. |
| Discoverability | Canonical URLs, sitemap, robots, JSON-LD | None | Homepage plus six agent pages and benchmark index covered | Keep. |

## Discarded changes

| Area | Candidate | Result | Decision |
|---|---|---|---|
| Context selection | Hard stop at 12 items | Lost test evidence on 1 of 12 tasks | Discard. Token savings do not justify role loss. |
| Neural routing | Query-shape-only skip | Neural 75 of 100, but recall `.790` to `.770` | Discard. |
| Neural routing | Strict score `5.0`, gap `0.5` | Neural 21 of 100; recall `.770` | Discard. |
| Neural routing | Score `10.0`, gap `1.0` | Lower nDCG and MRR than selected gate | Discard. |
| Fusion | Hash floors `.5` and `.7` | Recall fell to `.770` | Discard. |
| Fusion | Neural weights `.9` and `1.2` | Relevance below selected blend | Discard. |
| Index producer | File batch 16 | Small workload 2.27% faster; 30K bulk 2.98% slower, significant | Discard. Keep 64. |
| Index producer | File batch 32 | About 1% changes in opposite directions, within noise | Discard. |
| Index producer | Queue depth 0 | 0.67% slower, `p=.59` | Discard. Keep 2. |
| Index producer | Queue depth 8 | Repeats centered near baseline, `p=.20` | Discard. |
| SQLite persistence | Dynamic multi-row insert | 13.44% slower | Discard. |
| SQLite persistence | Cached 64-row insert | 1.71% slower | Discard. |
| SQLite persistence | Cached 128-row insert | 0.61% faster bulk, neutral small workload, row-order assumption | Discard. |
| SQLite schema | Normalize path, language, and kind | Maximum gross saving 5.3%; realistic saving below 5% | Discard. Joins and migration cost more than storage saved. |
| Worktrees | Git-only delta fast path | Ignore, attribute, sparse, filter, dirty, and untracked semantics narrow safe use | Discard. Retain content comparison. |
| Search cache | Replace clear-all caches with LRU | Default candidate and context breadth stay below current cliffs | Discard. No representative win demonstrated. |
| Search execution | Parallelize remaining passes | `SearchContext` is not `Sync`; duplicate SQLite contexts add open cost | Discard. Existing lexical expansions already use Rayon. |
| Testing | Whole-crate Miri gate | Pinned nightly fails in `libsqlite3-sys` on `cfg_select`; native dependencies remain poor Miri targets | Discard. Isolate pure Rust kernels before revisiting. |
| Website testing | Permanent Playwright CI for static pages | Browser install cost protects no executable product contract | Discard. Keep proportional HTML, XML, JSON-LD, and local render checks. |

## Detailed results

### Context

Corrected baseline used each historical task's parent tree, not current HEAD. The old method measured future-state code and allowed generated result files into context packs.

Two combined confirmation runs produced identical quality:

- Recall: `.7500`; primary recall: `.81944`; MRR: `.80556`.
- Zero-recall rate: `.08333`; covered roles: `7.0`.
- Mean tokens: `4,419` and `4,404`, from baseline `6,703`.
- Recall per 1K tokens: `.16478` and `.16481`, from `.10662`.
- Precision: `.19648`, from `.12807`.

Latency p95 was noisy across short runs, so no context-latency claim is made. Test-file recall remained zero on two labeled tasks. Test-specific retrieval is next relevance target.

CI release builds observed `6.9167` mean covered roles instead of the local `7.0` when parser timeouts removed one candidate role across 12 tasks. The integration floor is `6.8` to allow that environment-dependent candidate variation. A deterministic unit regression still requires role-aware selection to retain every available non-related role before stopping at 12 items.

### Neural routing and fusion

The selected gate runs neural retrieval only when lexical evidence is weak or ambiguous. Forced neural mode still bypasses the gate. When neural executes, hash ANN remains in fusion.

Full 1,000-query public-core confirmation against official v1.2.6:

| Metric | Baseline | Candidate | Delta |
|---|---:|---:|---:|
| nDCG@10 | .263440 | .268975 | +2.10% |
| MRR@10 | .216967 | .223943 | +3.21% |
| P@5 | .0606 | .0618 | +1.98% |
| Recall@20 | .469 | .472 | +.003 |
| Neural executions | 987 | 470 | -52.4% |

Same-build 100-query screen held recall at `.790`, reduced warm p50 39.6%, reduced warm p95 9.8%, and added 0.16% RSS. Full-run latency used different libc builds, so it is not treated as paired acceptance evidence.

### Clean Git no-op indexing

Fast return requires all of these conditions:

- Main Git workspace, clean before and after state capture.
- Queryable existing index and normal Git-ignore behavior.
- Indexed state matches HEAD, Git index, sparse-checkout state, repository excludes, global excludes, and ignored `.gitignore` or `.ignore` controls.

Any uncertainty falls back to full reconciliation.

| Corpus | Runs | Baseline mean | Candidate mean | RSS baseline | RSS candidate |
|---|---:|---:|---:|---:|---:|
| 10K files | 12 | ~74 ms | ~68 ms | ~20-21 MB | ~15-16 MB |
| 100K files | 8 | 616 ms | 494 ms | ~80-82 MB | ~34-36 MB |

### Current-release scale evidence

Three sequential v1.2.6 trials used the deterministic CC0 one-million-chunk corpus. Median values and each trial are published in `public-million-current.json`.

| Metric | Median |
|---|---:|
| Index wall time | 6.264 s |
| Index throughput | 159,649 chunks/s |
| Final index | 446,936,323 bytes |
| Peak RSS | 296,198,144 bytes |
| Warm CLI p95 | 13.73 ms |
| Warm engine p95 | 0.62 ms |
| Concurrent throughput | 2,498 queries/s |

## Validation performed during experiments

- Final composition: full `./test.sh` suite passed with 640 library tests, all Rust integration suites, 153 Python tests, three web tests, clippy with warnings denied, formatting, shellcheck, and production web build. Eleven download-backed stress tests remained intentionally ignored.
- Context: 22 unit tests, one focused CLI test, six benchmark tests, clippy, format, and two live gated runs.
- Routing: three focused tests, 638 library tests, full-feature clippy, format, 100-query screens, and 1,000-query confirmation.
- Indexing: 59 indexer tests, 19 incremental CRUD tests, 33 Git/worktree tests, release build, clippy, format, and repeated 10K/100K timing runs.
- Product surfaces: community and distribution tests, benchmark privacy check, HTML validation, XML validation, JSON-LD parse, and Chromium renders.
