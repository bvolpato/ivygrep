# ivygrep: State-of-the-Art and Open-Source Growth Plan

Date: 2026-06-21

## TL;DR

Update on 2026-06-23: the worktree, neural-route, and symbol-ranking fixes are
implemented, and a reproducible same-data Semble benchmark now exists. The
remaining evidence gap is the complete public task matrix and agent-task
outcomes.

ivygrep is much stronger technically than its adoption suggests, but it is not
state of the art yet because the evidence does not prove that claim.

The project already has a credible foundation:

- Local, zero-telemetry Rust binary.
- Hybrid lexical, hash-semantic, and neural retrieval.
- CLI, TUI, daemon, MCP, symbols, references, and callers.
- Fast lexical-first indexing with background enrichment.
- Five-target releases with checksums, SBOMs, provenance, attestations, and
  exact artifact acceptance.
- A real base-plus-delta Git worktree architecture not found in the inspected
  competitors.

The audit found these immediate blockers:

1. Resolved in this change: hybrid worktree search could return a file that
   existed only in the base checkout until the overlay was explicitly
   reindexed.
2. Resolved in this change: most short natural-language queries disabled
   learned neural retrieval, including the examples used to sell the product.
3. Resolved in this change: benchmarks labeled `neural` did not prove that
   neural query retrieval ran.
4. A representative controlled Semble comparison now exists; the complete
   public benchmark matrix and a ColGrep comparison are still missing.
5. There is no evidence that ivygrep improves coding-agent task success or
   token efficiency.
6. Agent activation, contributor onboarding, documentation consistency, and
   public positioning lag the engineering.

Recommended position:

> **Local, worktree-native code retrieval for coding agents. One binary. No API
> keys. No code uploads. Shared indexes across parallel Git worktrees.**

## Audit Snapshot

Repository state at audit:

- Branch: `main`, clean, aligned with `origin/main`.
- Audited commit: `c21a223cb2dc0b9bbf2f4067f2b73893ee849948`.
- Latest release: `v0.12.6`, published June 18, 2026.
- GitHub: 5 stars, 0 forks, 0 watchers, 0 discussions, 1 open issue.
- Activity: 513 commits, 62 merged PRs, 68 issues.
- External human participation: no external issue or PR authors found.
- Releases: 97 GitHub releases since March 10, 2026.
- Archive downloads: 306 across all releases. This is not a reliable active
  user count.
- Community health: 57%.

Local Linux ARM64 audit:

- Checksum-verified latest-release install completed successfully.
- Fresh release install took about 2.6 seconds on this host.
- First hash search over this repository took about 2.65 seconds.
- Repeated local process search took about 120 ms.
- `./test.sh --quick` passed 379 release-profile library tests.
- Default debug build failed because `gemm-f16` emitted instructions requiring
  `fullfp16`; the project test script already uses release mode on this host.
- Full `cargo package` failed because published `candle_embed 0.1.4` lacks the
  locally patched `metal` feature.

These timings are audit observations, not universal performance claims.

## What Is Already Good

### Engineering

- Lexical indexes become queryable before vector enrichment completes.
- Query, daemon, watcher, index health, recovery, concurrency, worktree, and
  release paths have substantial automated coverage.
- Public evidence artifacts include quality, latency, footprint, variance,
  model identity, and release provenance.
- Exact release archives are tested on Linux x86_64, Linux ARM64, macOS Intel,
  macOS ARM64, and Windows x86_64.
- Security CI includes dependency, secret, and workflow audits.

### Product

- Installers work without a compiler or Python runtime.
- First query auto-indexes.
- MCP works with Claude Code, Codex, Cursor, Gemini, and OpenCode.
- Search supports natural language, literal text, regex, symbols, references,
  callers, language filters, path scopes, and globs.
- Local-only operation is a real differentiator against hosted retrieval.

### Strategic Differentiation

The strongest differentiator is not generic "semantic grep." It is the
combination of:

- Local and private retrieval.
- One portable binary.
- Exact, lexical, semantic, and symbol-graph paths.
- Shared base indexes with per-worktree overlays.
- Strong release and evidence discipline.

That combination is useful specifically for coding agents working concurrently
across large private repositories and many Git worktrees.

## P0 Blockers

### 1. Fix Worktree Correctness

The worktree architecture is real, but hybrid search can leak base-only files.

Implementation status on 2026-06-21:

- Core leak fixed. Search now compares overlay and base generations before
  loading hybrid, literal, symbol, or daemon search state.
- Stale and malformed overlays rebuild under the index lock before results are
  served. Search contexts also fail closed if stale state reaches them.
- Regression coverage now adds a base-only file, reindexes only the base, and
  proves worktree search neither returns the file nor loses worktree-only
  content.
- Remaining work: proactive sibling-overlay propagation, full mutation oracle,
  and 1/5/20-worktree scale benchmarks.

Reproduction with `v0.12.6`:

1. Index base checkout and a worktree overlay.
2. Add and commit a file only in the base checkout.
3. Reindex the base.
4. Confirm the file does not exist in the worktree.
5. Run hybrid search from the worktree.
6. The base-only file is returned.
7. Explicitly reindex the worktree.
8. A tombstone is created and the false result disappears.

Literal mode did not reproduce the leak; hybrid search did.

Likely cause:

- Base-generation drift is reconciled during overlay indexing.
- Search can open the newer base index with older overlay tombstones.
- Watchers observe individual worktree roots and do not propagate base changes
  to sibling overlays.

Required work:

- Compare base generation before every worktree search.
- Reconcile or block stale overlays before serving results.
- Propagate base changes to overlays sharing the repository identity.
- Add a regression test for base-only additions.
- Add stale-result oracle tests for add, modify, delete, rename, branch switch,
  and empty-file replacement.
- Benchmark 1, 5, and 20 worktrees under concurrent queries and mutations.

Acceptance:

- Zero stale false positives and false negatives.
- Watched updates correct within 2 seconds p99.
- At least 5x less total index disk than independent full indexes at 20
  worktrees with 1% divergence.
- Less than 25% query p95 regression from 1 to 20 worktrees.

Relevant code:

- `src/workspace.rs`
- `src/indexer.rs`
- `src/search.rs`
- `src/daemon.rs`
- `tests/git_branch_switch.rs`

### 2. Make Neural Retrieval Observable and Real

Before this change, `QueryRouting::classify` used a 13-term threshold:

Implementation status on 2026-06-21:

- Short mixed natural-language queries now request neural retrieval when a
  compatible model and vectors are available. Exact identifiers and paths keep
  the cheaper default route.
- Hidden `--force-neural` mode crosses CLI/daemon protocol boundaries and is
  isolated in query-cache identity.
- Results expose `neural_requested` and `neural_executed`; semantic candidates
  preserve `hash` and `neural` channel provenance without replacing the
  backward-compatible `semantic` source.
- Neural benchmark modes force the route, retain execution/contribution counts,
  and fail if neural execution does not occur.
- End-to-end self-relevance verification executed neural retrieval for 23/23
  queries; neural candidates contributed to returned hits for 16/23.
- Remaining work: paired ablations, confidence intervals, candidate/timing
  diagnostics, and quality/latency acceptance gates.

- Exact identifier: neural disabled.
- Path-like query: neural disabled.
- Short error/literal query: neural disabled.
- Docs/tests/examples query: neural enabled.
- Other query with 13 or more terms: neural enabled.
- Other query with 2-12 terms: `Mixed`, neural disabled.

Hash-semantic retrieval still ran, but learned neural retrieval did not.

This affects flagship examples:

- `authentication flow`
- `error handling`
- `where is tax calculated?`
- `where is request authentication enforced?`
- `find error handling logic`

All 23 self-relevance fixture queries are shorter than 13 terms and currently
route with neural disabled.

Benchmark problem:

- `hybrid` and `neural` modes pass identical query arguments.
- Neural mode builds neural vectors, but normal routing can still skip them.
- Existing checks confirm that neural vectors exist, not that neural query
  retrieval executed.
- Published aggregate artifacts do not preserve route and source diagnostics.

Required work:

- Record classified route, hash execution, neural execution, candidate counts,
  channel weights, and per-channel timing.
- Fail a `neural` benchmark if no neural query path executes.
- Preserve per-query route/source diagnostics in public artifacts.
- Add tests that require compatible neural model identities and prove neural
  candidates affect ranking.
- Add paired 12-term and 13-term cases.
- Add zero-lexical-overlap semantic cases.
- Run factorial ablations for neural candidates, hash down-weighting, and
  semantic fusion weighting.

Do not immediately enable neural for every query. Measure adaptive routing
first.

Acceptance:

- At least 0.03 absolute and 10% relative nDCG@10 improvement for short
  natural-language queries.
- At least 5 percentage points improvement in top-3 success.
- Paired bootstrap 95% confidence interval excludes zero.
- Warm distinct-query p95 overhead no greater than 25 ms.
- Exact/path/error top-1 regression below 1 percentage point.
- No public task declines by more than 2% relative.

Relevant code:

- `src/search.rs`
- `src/embedding.rs`
- `scripts/eval_relevance.py`
- `scripts/eval_code_retrieval.py`
- `scripts/run_public_benchmark_matrix.py`
- `scripts/e2e_neural_backend.sh`

### 3. Repair Evidence and Marketing Consistency

The evidence dashboard correctly says:

- Competitive: not claimed.
- State of the art: not claimed.
- Same-hardware exact-search comparison: unavailable.
- Comparable semantic-system evidence: unavailable.

The website nevertheless presents speedup bars comparing full-scan exact
`grep`/`rg` queries against indexed ivygrep queries and labels them `22x` and
`1837x`. The note explains the distinction, but the visual comparison still
mixes different workloads and hardware evidence.

Documentation also drifts:

- `ARCHITECTURE.md` says AllMiniLM-L6-v2 is the default model.
- Code and README use `static-retrieval-v1`.
- `ARCHITECTURE.md` says 10 Tree-sitter languages.
- README and website say 21.
- Website and README disagree on default embedding dimensions.
- Website includes manual indexing language while README documents auto-index.

Required work:

- Remove non-comparable speedup multipliers.
- Show separate exact-search, semantic quality, cold indexing, warm distinct,
  and cache replay results.
- Generate model identity, dimensions, language counts, release version, and
  install behavior from code-owned facts.
- Extend claim checks beyond regulated words to benchmark comparability and
  execution provenance.
- Add a benchmark invariant: claimed mode must match executed retrieval paths.

## State-of-the-Art Evidence Plan

### Competitive Retrieval Benchmark

Direct local baselines:

| Project | Stars on 2026-06-21 | Role |
|---|---:|---|
| [Semble](https://github.com/MinishLab/semble) | 5,328 | Strongest current CPU reference |
| [CocoIndex Code](https://github.com/cocoindex-io/cocoindex-code) | 2,189 | Local embedding and agent integration reference |
| [ck](https://github.com/BeaconBay/ck) | 1,638 | Local hybrid grep reference |
| [SeaGOAT](https://github.com/kantord/SeaGOAT) | 1,298 | Established local semantic search |
| [Probe](https://github.com/probelabs/probe) | 637 | Structural and lexical agent-search baseline |
| [ColGrep](https://github.com/lightonai/next-plaid/tree/main/colgrep) | 496 | Multi-vector ColBERT/PLAID reference |

Separate tracks:

- Claude Context: normal path uses external embedding/vector services.
- mgrep: hosted and uploads indexed content.
- ripgrep and git grep: exact-search baselines, not semantic-retrieval peers.

Protocol:

1. Pin releases, revisions, models, configuration, and complete process trees.
2. Re-run Semble's 1,251-query, 63-repository suite.
3. Add a blinded holdout with 30 unseen repositories, 600 queries, at least 10
   languages, and mixed semantic/architecture/symbol/error/path workloads.
4. Freeze configurations before revealing holdout labels.
5. Add 500 deterministic identifier, path, fixed-string, and regex cases for
   exact-search evaluation.
6. Run on dedicated CPU-only Ubuntu hardware with fixed cores, memory, NVMe,
   CPU governor, and filesystem.
7. Report product-default and resource-constrained leaderboards.
8. Count unsupported repositories and failed runs instead of omitting them.
9. Capture network traffic and repeat with networking disabled.

Metrics:

- nDCG@10, MRR@10, top-3 success, Recall@20.
- Tokens to first relevant result.
- Process-cold, warm-distinct, and repeated-cache p50/p95.
- Concurrency at 1, 8, and 32 clients.
- Lexical-ready and full-quality-ready indexing time.
- One-file and 100-file update latency.
- Peak process-tree RSS and steady daemon RSS.
- Final and peak index bytes.
- Model download bytes.
- Platform and offline success.
- Branch-switch and 1/5/20-worktree correctness.

Pareto claim gate:

- Within 0.01 nDCG@10 of the best local tool on blinded holdout.
- Best or statistically tied in at least two of latency, indexing, RSS, disk,
  or portability.
- No exact-search recall regression.
- Bootstrap 95% confidence intervals.
- Differences below 5% treated as ties for resource metrics.

### Coding-Agent Outcome Benchmark

Retrieval metrics are necessary but insufficient. Current 2026 research points
toward agentic exploration and programmatic context delivery:

- [SWE-Explore](https://github.com/Qiushao-E/SWE-Explore-Bench)
- [ContextBench](https://github.com/EuniAI/ContextBench)
- [FastContext](https://github.com/microsoft/fastcontext)
- [CORE-Bench](https://github.com/siegelz/core-bench)

Stage 1: SWE-Explore screening

- Run all 848 tasks.
- Compare BM25, TF-IDF, ivygrep lexical, hash hybrid, adaptive neural, forced
  neural, and strongest practical external retrievers.
- Return exactly five ranked regions.
- Measure file/region hit rate, nDCG by line budget, context efficiency, first
  useful hit, and noise.

Stage 2: restricted-context repair

- Use the shared 150-task validation subset.
- Keep patch agent, model, prompt, tools, and budget fixed.
- Change only the retrieved context.

Stage 3: ContextBench Lite

- Run 500 tasks with the same agent scaffold.
- Control: standard shell search and file reads.
- Treatment: same tools plus ivygrep.
- Keep `rg` and file-reading tools available in treatment.
- Report cold-index and warm-index results separately.
- Start with 50 tasks and two seeds before full confirmation.

Pass through either route:

- Success: Pass@1 improves by at least 5 percentage points, 95% confidence
  interval excludes zero, and cost increases no more than 10%.
- Efficiency: Pass@1 is non-inferior within 2 points, input tokens fall by at
  least 25%, and billed cost falls by at least 20%.

Headline target:

- 10 percentage points better Pass@1, or
- 40% fewer tokens at equal success.

## Product Plan

### Agent-Native Tool Surface

MCP currently exposes search and status while the CLI already has richer
capabilities.

Prioritize:

- `ig_search` with route/source diagnostics, token budget, pagination, and
  session deduplication.
- Symbol definition, reference, and caller access through MCP.
- Bounded code extraction by file, symbol, or line range.
- Related-code search from an existing result.
- Compact output profiles for planner versus reader workflows.

Avoid building a full coding agent inside ivygrep initially. Make the retrieval
tools composable, predictable, and cheap for existing agents.

### One-Command Activation

Binary installation is already strong. Agent setup is the larger gap.

Ship:

```text
ig setup
ig teardown
```

`ig setup` should:

- Detect Claude Code, Codex, OpenCode, Cursor, and Gemini.
- Install MCP configuration and canonical usage instructions.
- Merge configuration safely with backups.
- Optionally install a shared skill/plugin.
- Validate MCP initialization, tool listing, and a real search.
- Offer model prefetch or hash-only operation.

Complete lifecycle:

- Daemon stop/restart.
- Model status/download/purge.
- Integration list/remove.
- Channel-aware upgrade instructions.
- Uninstall that preserves indexes by default and supports explicit purge.

Distribution priorities:

1. WinGet.
2. Project-owned Scoop bucket.
3. Claude, Codex, and OpenCode skills/plugins.
4. MCP Registry package through a thin bootstrap or supported bundle.
5. Fix `cargo package` and publish crates.io as a secondary source channel.
6. Pursue Homebrew core only after external notability.

Preserve zero default runtime telemetry.

## Open-Source Readiness

### Immediate Repository Work

- Add `SECURITY.md`.
- Enable private vulnerability reporting.
- Add `CODE_OF_CONDUCT.md`.
- Add `SUPPORT.md`.
- Add lightweight governance and maintainer expectations.
- Add CODEOWNERS.
- Add structured issue forms:
  - Bug.
  - Search miss.
  - Performance regression.
  - Feature request.
- Add PR template.
- Protect `main` with required checks, signed commits, resolved conversations,
  and review.
- Add release tag/version consistency validation.
- Set stable release train to one every 2-4 weeks; use prereleases/nightlies
  for rapid iteration.

### Contributor Experience

- Pin Rust version/MSRV.
- Document ShellCheck, Python, and platform prerequisites.
- Make one documented contributor command work across x86 Linux, ARM64 Linux,
  macOS, and Windows.
- Explain hash-only versus neural contributor paths.
- Add `cargo publish --dry-run` CI.
- Maintain 10 scoped `good first issue` and 5 `help wanted` issues.
- Label by component and platform.
- Split hotspots only around active ownership boundaries:
  - Query routing and diagnostics.
  - Candidate retrieval and fusion.
  - Ranking and filtering.
  - Worktree overlay reconciliation.
  - Index storage and enrichment.

Avoid broad refactors that do not improve testability or contributor ownership.

## Positioning and Launch

### Target Persona

Primary:

- Terminal-first engineers using Claude Code, Codex, or OpenCode.
- Large private repositories.
- Multiple parallel Git worktrees.
- Strong privacy or offline requirements.

Secondary:

- Teams that need auditable local retrieval infrastructure.

Do not position for every editor and every search workflow.

### Message

Suggested repository description:

> Local, worktree-native code search for Claude Code, Codex, Cursor, and other
> coding agents. One binary. No API keys. No code uploads.

README order:

1. One-sentence outcome.
2. Install and first query.
3. 60-90 second real demo.
4. Agent setup.
5. Why worktree-native matters.
6. Comparable benchmark evidence.
7. Advanced architecture, security, and release details.

Move supply-chain and detailed benchmark history out of the first-use path.

### Launch Assets

- 90-second demo on a real large repository.
- Demonstrate multiple worktrees sharing an index.
- Reproducible comparison report.
- Agent-outcome pilot with trajectories and costs.
- Honest "where ivygrep wins and loses" table.
- Social preview image focused on user outcome, not architecture.
- Copy-paste setup for each supported agent.

### Distribution

Launch one milestone release over 7-10 days:

- Show HN.
- Lobsters.
- Rust and command-line communities.
- Local AI and coding-agent communities.
- MCP Registry and ecosystem directories.
- Claude, Codex, and OpenCode plugin/skill channels.
- X and LinkedIn.
- Relevant awesome lists and newsletters.

Do not use six patch releases in one day as launch activity. Each stable release
should carry one user-facing narrative and one measured outcome.

### Community Growth Loop

1. User submits reproducible search miss.
2. Miss becomes public evaluation case.
3. Fix is measured against the suite.
4. Reporter is credited.
5. Release publishes benchmark movement.
6. Result becomes launch/content material.

## 12-Week Execution Plan

### Weeks 1-3: Truth and Correctness

- Fix worktree generation reconciliation.
- Add stale-result regression matrix.
- Instrument query routing and retrieval sources.
- Repair neural benchmark execution and labels.
- Remove non-comparable marketing claims.
- Fix documentation drift.
- Fix supported-platform contributor commands.

Exit:

- Zero known stale worktree results.
- Benchmark modes prove executed paths.
- Claims match evidence.
- Clean contributor path works on supported platforms.

### Weeks 4-6: Activation and OSS

- Build `ig setup` and `ig teardown`.
- Add model, daemon, integration, upgrade, and purge lifecycle.
- Expose existing graph capabilities through MCP.
- Reach 100% GitHub community health.
- Protect `main`.
- Publish newcomer issues.
- Move to stable release train.

Exit:

- 90% clean activation target in internal study.
- No manual config editing in recommended agent setup.
- 8/10 fresh contributors reach green tests within 30 minutes.

### Weeks 7-9: Comparable Evidence

- Run short-query ablations.
- Run SWE-Explore screening.
- Run Semble and ColGrep comparison.
- Run multi-worktree benchmark.
- Run 50-task, two-seed agent pilot.
- Publish raw data and failed runs.

Exit:

- Clear quality/resource frontier.
- Validated or rejected short-query routing change.
- Agent pilot shows positive direction before full spend.

### Weeks 10-12: Relaunch

- Rewrite README and website.
- Produce launch demo and benchmark report.
- Publish agent integrations and registry metadata.
- Release one milestone version.
- Stagger launch channels.
- Recruit external benchmark reproduction and contributors.

Exit:

- Product has one clear message.
- Installation to successful MCP query is under 5 minutes for at least 90% of
  test users.
- Public claims are reproducible.

## Goals

These are ambitious execution targets, not forecasts.

| Horizon | Product and evidence | Community | Adoption |
|---|---|---|---|
| 30 days | Worktree leak fixed; neural routing observable; docs consistent; `ig setup` beta | 100% community health; 10 good-first issues; 8/10 contributors green within 30 minutes | 250 stars; 1,000 installs; 150 successful activations |
| 90 days | Agent pilot and competitive benchmark published; 90% activation within 5 minutes | 3 merged external PRs; 5 external issue authors; first response under 24 hours | 1,000 stars; 5,000 installs; 1,000 activations; 300 MCP activations |
| 180 days | Full agent study; validated Pareto position; correct scaling at 20 worktrees | 10 merged external PRs from 6 contributors; 3 repeat contributors | 3,000 stars baseline; 5,000 stretch; 15,000 installs; 3,000 activations |

Stars are a distribution metric, not the product's north star. The stronger
success measures are:

- Successful agent activations.
- Seven-day reuse in opt-in studies.
- Agent task success and token-cost frontier.
- Search-miss rate.
- External contributors and benchmark reproductions.

## First Ten Issues

1. P0: Prevent base-only files from leaking into worktree hybrid search.
2. P0: Add worktree stale-result oracle and 1/5/20-worktree benchmark.
3. P0: Record and enforce executed retrieval route in benchmark artifacts.
4. P0: Add real learned-neural query tests and short-query ablations.
5. P0: Remove non-comparable website speedup multipliers and fix docs drift.
6. P0: Make contributor build/test path work on supported Linux ARM64.
7. P1: Add `ig setup` and `ig teardown` with MCP validation.
8. P1: Expose symbols, references, callers, and bounded extraction through MCP.
9. P1: Expand the Semble adapter to the complete public matrix and add a
   ColGrep blinded holdout.
10. P1: Add community files, structured issue forms, protected main, and
    newcomer backlog.

## Sources

Project:

- [ivygrep repository](https://github.com/bvolpato/ivygrep)
- [Roadmap issue #128](https://github.com/bvolpato/ivygrep/issues/128)
- [Evidence dashboard](https://bvolpato.github.io/ivygrep/benchmarks/evidence-dashboard.html)
- [Public retrieval benchmark](https://bvolpato.github.io/ivygrep/benchmarks/public-code-retrieval.html)
- [Million-chunk benchmark](https://bvolpato.github.io/ivygrep/benchmarks/public-million.html)

Competitors:

- [Semble](https://github.com/MinishLab/semble)
- [ColGrep / NextPlaid](https://github.com/lightonai/next-plaid/tree/main/colgrep)
- [CocoIndex Code](https://github.com/cocoindex-io/cocoindex-code)
- [ck](https://github.com/BeaconBay/ck)
- [SeaGOAT](https://github.com/kantord/SeaGOAT)
- [Probe](https://github.com/probelabs/probe)
- [Claude Context](https://github.com/zilliztech/claude-context)
- [mgrep](https://github.com/mixedbread-ai/mgrep)

Benchmarks and research:

- [CoIR](https://github.com/CoIR-team/coir)
- [SWE-Explore](https://github.com/Qiushao-E/SWE-Explore-Bench)
- [ContextBench](https://github.com/EuniAI/ContextBench)
- [FastContext](https://github.com/microsoft/fastcontext)
- [CORE-Bench](https://github.com/siegelz/core-bench)
- [PGR agentic code-search research](https://github.com/entireio/pgr)

Distribution:

- [MCP Registry package types](https://modelcontextprotocol.io/registry/package-types)
- [Homebrew acceptable formulae](https://docs.brew.sh/Acceptable-Formulae)
- [WinGet packages](https://github.com/microsoft/winget-pkgs)
- [Scoop inclusion criteria](https://github.com/ScoopInstaller/Scoop/wiki/Criteria-for-including-apps-in-the-main-bucket)
