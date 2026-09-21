# ivygrep architecture

This document explains current repository structure and runtime behavior. It is
written for contributors who need to change indexing, retrieval, context packs,
or client integrations without breaking storage or protocol contracts.

Source code remains authoritative. Public behavior belongs in tests and CLI
help; this document records boundaries and invariants that are easy to miss when
reading one module at a time.

## Design constraints

ivygrep is built around six constraints:

1. **Repository data stays local.** Source, queries, indexes, embeddings, and
   results are processed on the user's machine. Neural profiles may download
   pinned model assets on first use.
2. **Lexical search becomes usable first.** A fresh index commits searchable
   text before optional hash and neural vector enhancement finishes.
3. **Results are bounded and inspectable.** Search limits candidate work.
   Context packs enforce a token budget and explain why each item was selected.
4. **Index updates preserve the last healthy state.** Fresh rebuilds use staging
   artifacts. Metadata and snapshots become authoritative only after stores
   commit successfully.
5. **Git worktrees reuse data without leaking base content.** A worktree reads
   its repository's base index plus a small overlay of changes and tombstones.
6. **Clients share contracts, not one oversized executor.** CLI, daemon, MCP,
   TUI, and Web reuse search options and aggregation while retaining their own
   lifecycle, caching, progress, and cancellation behavior.

## Runtime map

```mermaid
flowchart LR
    User[User or coding agent]
    CLI[CLI]
    MCP[MCP stdio server]
    TUI[Terminal UI]
    Web[Web UI]
    Daemon[Local daemon]
    Search[Search service]
    Context[Context-pack builder]
    Indexer[Indexer]
    Stores[(SQLite + Tantivy + vector stores)]
    Files[(Repository files and Git state)]

    User --> CLI
    User --> MCP
    User --> TUI
    User --> Web
    CLI --> Daemon
    CLI --> Search
    MCP --> Daemon
    MCP --> Search
    TUI --> Search
    Web --> Daemon
    Daemon --> Search
    CLI --> Context
    MCP --> Context
    Search --> Stores
    Context --> Search
    Context --> Stores
    Daemon --> Indexer
    CLI --> Indexer
    Indexer --> Files
    Indexer --> Stores
```

CLI and MCP can use local execution paths when daemon routing is unavailable or
inappropriate. Daemon adds watchers, shared caches, background jobs, and Web
serving; it is not required for every read path.

## Entry points and client surfaces

| Surface | Primary code | Responsibility |
| --- | --- | --- |
| Binary entry | `src/main.rs`, `src/cli.rs` | Parse commands, choose daemon or local execution, format output |
| Daemon | `src/daemon.rs`, `src/ipc.rs`, `src/jobs.rs` | Watch workspaces, schedule jobs, cache search state, serve IPC |
| MCP | `src/mcp.rs` | Expose `ig_search` and `ig_status` over JSON-RPC stdio |
| TUI | `src/tui.rs` | Interactive search, navigation, and file preview |
| Web server | `src/web.rs` | Serve embedded frontend assets and authenticated local APIs |
| Web frontend | `web/src/` | Search, context, tree, and file-viewer interactions |
| Agent setup | `src/agent.rs` | Configure supported clients and verify one real MCP search |

Cargo builds one binary, `ig`. Default features include local neural retrieval.
`--no-default-features` produces a hash-only build. Platform features select
Accelerate, Metal, or CUDA support where available.

`build.rs` embeds `web/dist` into the binary. Frontend source changes therefore
require a deterministic `pnpm -C web build` and committed generated assets.

## Workspace identity and data location

`Workspace::resolve` canonicalizes the requested path, discovers its repository
root when applicable, and derives a stable workspace identifier. Indexes live
under:

```text
$IVYGREP_HOME/indexes/<workspace-id>/
```

Without `IVYGREP_HOME`, ivygrep uses `$XDG_DATA_HOME/ivygrep` or
`~/.local/share/ivygrep`. On Unix, ivygrep-owned index directories are restricted
to mode `0700` because stored chunks can contain private source text.

Git worktrees share a repository identifier. A secondary worktree records the
main worktree's index directory as its base and stores only divergent state in
its own index directory.

A checkout nested inside a Git workspace is a workspace of its own, as it is
for Git and for `Workspace::resolve`. The file walker, the request-local path
matcher, and the watcher event filter share one rule (`NestedCheckouts` in
`src/workspace.rs`): below a Git workspace root they skip a directory whose
`.git` entry marks a linked worktree (a `.git` file whose Git directory has a
`commondir`), a nested clone (a `.git` directory), or a clone made with `git
clone --separate-git-dir` (a `.git` file pointing at a full Git directory
elsewhere). Agent worktrees under `<repo>/.claude/worktrees/` therefore stay
out of the base index, base search results, and base watcher updates, including
worktrees created after the base was indexed; without the rule each live
worktree added a full copy of the repository to the base. Submodules stay in
the parent, which tracks them and has always indexed their sources: a path
listed in the root `.gitmodules`, or a `.git` file pointing into the `modules`
directory of the workspace's own Git directory, where Git keeps absorbed
submodules. A workspace root that is not a Git checkout keeps everything below
it, so a plain directory of clones still indexes as one workspace. The
recursive watch still covers nested checkouts; their events are dropped by the
filter, except for the `.git` entry itself. When a directory whose files are in
the index gets a `.git` entry (`git init`, or a clone into it), the watcher
reconciles the whole workspace and the files leave the index. When an entry
goes away, the directory is handed to the index update as a changed path: a
deleted worktree costs nothing, and a directory that is still there is scanned
and its files return. A checkout that arrives together with its directory, as
an agent worktree does, has no files in the index and costs one SQLite lookup,
not a scan.

Because index IDs follow canonical paths, replacing a directory can change its
checkout role while retaining its saved index. Health checks reject a saved
main/overlay layout that disagrees with the resolved role. Indexing clears that
obsolete layout under the index lock, preserves workspace settings, and rebuilds
the appropriate stores before local queries use them.

## Index lifecycle

### Initial indexing

1. Resolve workspace and acquire its index lock.
2. Inspect index health and stored format version.
3. Walk indexable files while applying Git ignore rules unless explicitly
   disabled.
4. Build a flat Merkle snapshot of relative paths and file fingerprints.
5. Send files through a bounded scanner/chunker producer.
6. Parse supported languages with Tree-sitter or a bounded text fallback. The
   parse budget counts parser operations, so a file takes the same path on any
   machine; a CPU-time allowance per operation only stops pathological inputs.
7. Extract symbols, imports, documentation relationships, tests, configuration,
   and unresolved dependency records.
8. Persist lexical documents and metadata to staging stores.
9. Commit SQLite and Tantivy, validate staged artifacts, then promote them.
10. Write workspace metadata, generation, format version, and Merkle snapshot.
11. Schedule hash and neural vector enhancement when configured.

Fresh indexing uses staging because SQLite, Tantivy, and vector stores cannot be
committed as one cross-store transaction. Promotion keeps rollback artifacts
until the new set is complete. A failed rebuild leaves the previous index
queryable.

### Incremental indexing

The Merkle snapshot compares current file fingerprints with the last committed
state. Its diff contains added or modified paths plus deleted paths. Incremental
updates replace affected chunks, graph edges, symbols, lexical documents, and
vector keys inside bounded transactions.

Watcher events use a targeted refresh only when path-level reconciliation is
safe. New directories, ignore-file changes, uncertain Git state, and similar
cases fall back to a full walk. A clean Git workspace whose recorded repository
state still matches can return without scanning every file. The fingerprint
includes ancestor `.ignore`/`.gitignore` controls. Reuse is disabled when
independent ignore rules whitelist files Git may ignore, or when assume-unchanged
flags or present skip-worktree files prevent Git status from observing source
edits. Those cases use the normal Merkle walk, including during base reuse.

No-op shortcuts still verify primary storage: SQLite and Tantivy chunk counts
must agree, and hash-vector headers and bounds must be readable. Worktree
indexing checks both overlay and inherited base stores. Failed validation forces
complete staged recovery, not a replay of an empty or partial Merkle delta.
These checks add store-opening/counting work even when source discovery is skipped;
they do not validate every vector payload or compare concurrent enhancement markers.

Main indexes and overlays mark live publication as incomplete before changing
stores. The marker survives failures and is cleared only after stores, metadata,
snapshot, and filter state publish successfully. Main recovery rebuilds in staging;
no-op Git checks, watcher shortcuts, and base reuse cannot trust an unfinished
publication. Fresh main builds also mark the interval between store promotion
and snapshot publication.

Hash and neural vector deletion journals prevent stale embeddings from becoming
visible when another store fails. Merkle state is saved only after committed
stores and metadata agree.

## On-disk stores

| Artifact | Purpose |
| --- | --- |
| `workspace.json` | Root, watch intent, timestamps, and index generation |
| `index_incarnation` | Store identity that changes after main-index or overlay replacement, even if a generation number is reused |
| `metadata.sqlite3` | Chunk text and metadata, symbols, graph edges, unresolved dependencies, statistics |
| `tantivy/` | BM25, path, signature, language, kind, and trigram postings |
| `vectors.usearch` | Lightweight hash-vector index |
| `vectors_neural.usearch` | Learned neural-vector index |
| `neural_profile` and model identity metadata | Vector compatibility contract |
| `merkle_snapshot.json`, `merkle_snapshot.verified` | Last committed path fingerprints and aggregate root hash; the sidecar records the size and mtime of the last snapshot that parsed successfully so health checks skip re-parsing it |
| `job.json`, locks, progress files | Index, enhancement, and watcher coordination |

SQLite stores compressed chunk text when compression is useful. Reads use a
fallible decompression path with a 32 MiB output limit. Corrupt or oversized data
returns a contextual error instead of being treated as source text.

Tantivy is the lexical candidate store. SQLite remains authoritative for rich
chunk metadata and graph relationships. USearch stores F16 vectors and validates
headers, dimensions, and file bounds before native loading. Callers open every
vector store with an explicit `VectorTier`: the hash tier uses a smaller HNSW
graph to bound background build cost, and the neural tier keeps USearch quality
defaults. Vector shape cannot select the tier because the default neural
profile shares the hash store's 256-dimensional F16 layout.

Neural metadata is optional for literal and hash retrieval. Unreadable identity
or profile metadata is reported but does not prevent those modes from loading
healthy primary stores. Neural requests still require readable, compatible
metadata. Doctor diagnoses malformed identities and, with `--fix`, removes only
their derived neural artifacts after acquiring index/job locks and checking that
enhancement is inactive. It keeps an invalid identity until cleanup finishes so
an interrupted repair remains retryable. Neural metadata files publish through
atomic replacement; this is not a power-loss durability guarantee.

Background enhancement serializes vector writers with `enhancement.lock`, which
survives index removal alongside the index and job locks. It takes `index.lock`
only to open its initial stores and to publish checkpoints, metadata, and journal
cleanup. Model inference and resource pauses leave lexical indexing unblocked.
Every publication checks the captured `index_incarnation`; staged main-index and
overlay replacements rotate this identity with their stores, and rollback restores
it. Incremental updates keep the incarnation, leave new deletion journals for the
next pass, and prevent the older worker from marking the new generation complete.

Neural enhancement saves a checkpoint after at least 16,384 new chunks since
the previous checkpoint. A non-divisor batch size crosses that boundary rather
than waiting to land on an exact multiple.

The current on-disk format version is defined by `INDEX_FORMAT_VERSION` in
`src/workspace.rs`. Incompatible schema, chunking, or vector-identity changes
must bump it and provide rebuild or migration behavior.

Format v25 refreshes Python and Objective-C dependency facts through a one-time
full index rebuild, including unchanged files. This is not a graph-only migration;
existing indexes pay the normal indexing and subsequent vector-enhancement costs.

Format v26 also invalidates payloads indexed before contained source reads.
Existing indexes rebuild from current workspace files before their stored text
is trusted as a fallback, even when the source snapshot itself is unchanged.
Direct indexed-source APIs reject incompatible local or inherited base formats
until that rebuild completes.

Format v28 rebuilds existing indexes once so Rust `crate::` and library-name
imports resolve within the owning Cargo package and target, replacing stale
cross-package dependency edges.

Format v29 rebuilds once more to retain resolved import specifications.
Incremental runs use them to refresh unchanged importers when a newly added file
takes precedence over an existing target, and Python package initializers take
precedence over same-named modules.

Format v30 rebuilds once so fallback-chunked files index lines before their
first declaration; Python decorators, decorators before a JavaScript or
TypeScript `export class`, and TypeScript member decorators stay in their
definition's chunk; signatures skip multi-line annotations; and Java records,
module-level JavaScript and TypeScript function bindings, and module-level Rust
`macro_rules!` macros register symbols. A macro declared inside a function stays
in that function's chunk and does not register a definition.

## Search pipeline

### Workspace selection

`src/search_service.rs` resolves one workspace or all registered workspaces and
aggregates results in deterministic score and path order. One broken workspace
produces a warning alongside valid hits. If every selected workspace fails, the
request returns an error rather than an empty result.

### Per-workspace execution

`SearchContext` opens compatible SQLite, Tantivy, and vector stores for one
workspace. Worktree contexts combine base and overlay stores while respecting
tombstones and shadowed paths.

`src/search_routing.rs` classifies query shape and assigns bounded candidate
budgets. Relevant signals include exact identifiers, literals, paths, natural
language, code-like syntax, note-like results, filters, and stored vector
availability.

A hybrid query whose letters and digits are all non-ASCII, such as CJK or
Cyrillic text, has none of the ASCII code tokens that lexical, path, and hash
signals use. Unless neural retrieval is forced, it runs exact substring
matching, the only pass that can find it.

`src/search_execution.rs` coordinates applicable retrieval passes:

- exact substring or regex candidates
- Tantivy BM25 over text, paths, signatures, and trigrams
- exact and inferred symbol candidates
- lightweight hash-vector ANN
- neural ANN when routing requests it and compatible vectors exist
- bounded memory probes for qualifying note-heavy implicit questions

Neural retrieval is conditional, and lexical results remain available while
neural vectors are incomplete. Identifier, path, and short literal routes skip
it. On the remaining routes a transformer profile runs only when lexical
confidence is low (top BM25 score under 2.0 or a top-two gap under 0.25),
because its query embedding costs a forward pass. A static token-mean profile
embeds a query with table lookups in about a millisecond; when its dense order
also has measured standing value (`potion-code-16m-v2`, the default), it runs
for every query on those routes and late dense fusion decides per query how far
to trust it. `static-retrieval-v1` keeps the lexical-confidence gate: its extra
first-stage votes lowered file-localization recall.
`--force-neural` requires compatible persisted neural vectors and makes neural
execution observable in structured output.

Hard visibility and request filters apply before bounded candidate admission.
Without a residual glob, a native Block-WAND traversal first collects the normal
bounded pool, even for daemon requests with cancellation tokens. If every
returned document is eligible, that pool is final. A rejected document triggers
a second traversal with eligibility checked before heap admission. This fallback
reads competitive stored metadata, retains bounded memory, and checks cancellation
per posting; the native probe retains ordinary pre/post cancellation checks.
Residual-glob requests use the cancellable filtered collector directly. If invisible or
missing ANN keys underfill a candidate pool, a cancellation-aware fallback
streams eligible SQLite keys and exactly scores fixed-size batches. This can
scan the eligible corpus, but does not allocate a corpus-sized ANN result set.
Ordinary ANN requests retain shared metadata hydration when no keys are rejected.

Candidate cutoffs do not depend on segment layout. Tantivy breaks equal scores
by document address, which follows indexing threads and merges, and Block-WAND
adds term scores in traversal order, so one document's BM25 score can move by a
few ULPs between layouts. Lexical, path, literal, and Boolean pools compare
scores by bucket (the top 13 of 23 f32 mantissa bits, `6e-5` to `1.2e-4`
relative) and order equal buckets by chunk key from a fast column. Collection
stays one Block-WAND pass: the pruning threshold sits just below the worst kept
bucket, so tied documents are scored, but no stored document is read to order
them. Candidates report bucket scores. Indexes written before the key column
existed keep address order among equal buckets.

Explicit Boolean requests are parsed before expansion. All retrieval signals
are restricted to a request-local pool of raw-query matches bounded by the
normal lexical candidate budget. Semantic scoring ranks only keys in that pool;
it cannot introduce an otherwise similar document that violates the constraint.
Unsupported structured queries, including phrases requiring unindexed positions,
fail explicitly when the request is a one-line lookup. Quoted or escaped operator
words and ordinary natural-language input keep their existing expansion behavior.

A prompt or paste that does not parse is not a Boolean request. The cut is the
one signature scoring uses: multi-line input, or at least 13 raw terms
(`is_prompt_shaped` in `search_routing.rs`). Its uppercase `AND`, `OR` or `NOT`
is emphasis or pasted SQL, so it takes the ordinary expansion path with the
operator words read as text, and the results carry a warning through the
`warnings` field of search responses. The lexical pass does not hand that text to
the Tantivy parser, which would apply the operators; it matches analyzer tokens,
as for any text the parser rejects. Input that parses is a Boolean request on any
shape, so the fallback changes results only for requests that used to fail.

Multi-line queries that read as pasted source score lexical matches without the
boosted signature field. Each pasted identifier would otherwise add a
near-maximal bonus to every one-line definition signature containing it and
bury the snippet's body evidence. Signature text stays searchable through the
body field, and an explicit `signature:` clause keeps its boost. The dotted
`owner.member` heuristic does not create exact-symbol lookups for pasted source,
but mixed-case identifiers inside it, such as `sendFile`, can still become
exact-symbol candidates.

A line reads as code when it ends in `;`, `{` or `}`, starts with `}`, is an
import statement (`import …`, `from … import …`, or `require`, `use` or
`package` with one argument), ends in `:` after a code-shaped token, has at
least as many code-shaped tokens as words, or is indented. An indented list
item is not code, and neither is a continuation: an indented line of words with
no code-shaped tokens that does not end in `:` and follows a prose line that
does not end in `:`, such as a wrapped list item or an indented paragraph.
Code-shaped tokens are operators and identifier shapes such as `=`,
`snake_case`, `camelCase`, `owner.member`, `a::b`, and `call(arg)`. Numbers,
versions, sizes, and issue references such as `1.84.0`, `v1.2.16`, `2.3GB`,
`12k`, and `#375` count as neither, and so do bullets, dashes, table pipes, and
Markdown rules and heading marks. Unicode punctuation around a word, such as
`？`, `。`, curly quotes, and `…`, is trimmed like ASCII punctuation, and `e.g.`
and `and/or` count as words. A multi-line query is pasted source when the
tokens of its code lines plus the code-shaped tokens of its other lines at least
match the words of those other lines. Multi-line pasted error output, described
below, keeps the pasted-source rules.

Multi-paragraph prompts, pasted issue text, and questions with a blank line are
prose. Their lexical matches include the signature field, but without the 5x
boost one-line queries get: a boosted bonus for every term of a long prompt
would lift short definitions that share a word above documents that explain the
task. A one-line prompt of 13 or more terms, the cut routing already uses for
natural-language queries, scores signatures the same way. In such a prompt a
word whose only capital is its first letter, such as "Create" or "Given",
starts a sentence and is not inferred as a symbol name; `parseConfig` and
`snake_case` names still are. Multi-line prose `owner.member` mentions become
exact-symbol lookups only on prose lines, and only when written as code: a call
such as `res.send(body)`, a backtick span, or a camelCase member such as
`res.sendFile`. File names, hosts, and missing spaces such as `go.sum`,
`go.dev`, or `it.The` look up nothing. One-line queries keep their existing
`owner.member` lookups. An explicit `signature:` term clause keeps 5x in any
query, including explicit Boolean queries. Tantivy applies field boosts to term
clauses only, so range and set clauses on the field score the same unboosted
value on every query shape.

`src/search_error_text.rs` recognizes pasted error output by line-leading error
labels (`Error:`, `Caused by:`, `ValueError:`, `java.lang.IllegalStateException:`,
`Uncaught TypeError:`), `[ERROR]` lines, traceback headers, `(os error N)`
causes, and `panicked at`. A label other than `Caused by:` counts only when the
line reads as a message: at least two words with letters, no trailing `,`, `;`,
`{`, `(`, `[` or `=`, no bare operator such as `|` or `=`, not a lone quoted
string, and not nested under a line ending in `:`, `{`, `(` or `[`. Struct and
interface fields, object keys, YAML keys, and docstring `Raises:` entries in
pasted source therefore do not count. Only the first 200 lines are classified;
later lines stay in the retrieval text unchanged. For detected queries,
lexical, path, hash-vector, fusion, and learned reranking signals use the text
with runtime values removed: absolute paths outside the workspace, URLs, hex
ids, numbers, and timestamps, including the values of `key=value` pairs, whose
keys stay. A quoted absolute path containing spaces, such as
`"/home/Jane Doe/app.lock"`, is one value. Absolute paths inside the workspace
keep their workspace-relative form. The static message text between those
values, split at quoted values, `key=value` pairs, colon chain separators,
bracketed groups, and sentence ends, joins the exact-substring pass, so code
containing the format string ranks first. Leading timestamps and bracketed
severities such as `[ERROR]` are not part of that text. At most eight runs are
kept: runs from message lines (a counted label, an error severity, or a cause
under `Caused by:`) first, then the longest. When a file's best chunk has no
exact match but another chunk of that file does, fusion shows that chunk for
the file instead, keeping the file's score. Neural query vectors keep the
original text because the daemon embeds the query before it resolves a
workspace. Pasted source without error framing is not affected.

### Fusion and presentation

`src/search_fusion.rs` combines candidates, source provenance, path and role
signals, literal coverage, and deterministic reranking. Fusion remains one
module because ordering and score interactions form one relevance contract.

A hash-vector match on a candidate that lexical, literal, path, or symbol search
already found hashes the same words BM25 scored. For prose, one line or several,
it gets no fusion vote unless the query names secondary sources such as tests,
docs, or examples. Pasted source and multi-line pasted error output keep a vote,
because hash overlap on pasted identifiers still separates the matching snippet.
When hash votes are discounted, neural corroboration votes from the neural
tier's own rank, and semantic-only discoveries keep full weight.

Lexical rank votes are nearly flat at the top (3.2/61 against 3.2/62) and the
BM25 score term is logarithmic, so a BM25 margin barely reaches the fused
score. For a lookup that is intended: structural boosts choose among near-tied
matches. A query of 13 or more terms, pasted source included, sums dozens of
term scores, and its margin is the strongest evidence available. Its lexical
candidates also vote with their share of the best BM25 score (`score / best
score`, weight 1.0). The weight sits at the low end of a plateau from 1 to 4 on
the tuning halves of the public benchmarks. The vote reorders results but must
not make the score filter return a shorter list: a vote worth several rank
votes to the leader would push low-share tail candidates under the score
filter's 35%-of-best cut. Files that clear that filter only without the vote
are therefore appended after the results the voted pass kept, one chunk per
file and ahead of backfill, so the kept order and the hits the learned reranker
scores per file stay as the voted pass produced them. `limit` then truncates in
voted order, so in a full list a file the vote ranks under the cutoff is
ordinary reranking. On a notes benchmark whose questions need several sessions
each, the vote without this rule returned shorter lists and lost recall@20.

Late dense fusion runs after the heuristic scoring and before result filtering
and the learned reranker. First-stage neural votes are one reciprocal-rank list
of weight 1.0 among lexical, literal, path, and symbol lists worth several
times more, and rank votes are nearly flat across fifty neighbours, so a better
embedding model barely moved fused results. The late stage works on files:

- Search execution scores every candidate chunk direct search found against
  the neural query vector (exact cosine, a few hundred vectors), so a file has
  a dense rank even when it is not among the nearest neighbours.
- A file takes part only if lexical, literal, path, or symbol search found it,
  it passes the authority test the result filter applies, and it has the most
  authoritative path role among the direct candidates. Secondary intent does
  not lift the role rule, because "test", "docs", or "example" in pasted source
  or long prose is usually incidental. Cosine alone never promotes a file: on
  the tuning split a dense leader direct search missed was the relevant file
  4% of the time for the static profiles and 31% for a small transformer. The
  role rule exists because prose queries sit closest to prose files: on the
  file-localization benchmark every file a first version wrongly moved to the
  top was a README, changelog, guide, or test. The authority test exists
  because a promoted file the filter then drops still sets its adaptive score
  threshold.
- Pasted source and multi-line or long prose take part. Short natural-language
  queries skip the stage: there a decisive dense leader was the relevant file
  36-39% of the time on the tuning split, against 70-90% for the other shapes.
- Eligible files are reordered among the positions they already hold; every
  other file keeps its place, so dense evidence never moves documentation or
  tests relative to the implementation in either direction. Each eligible file
  keeps a reciprocal-rank vote for its current rank and gains
  `weight / (60 + dense_rank)`, where `weight` is the profile's standing weight
  for the query shape. `DenseFusionCalibration` in `src/embedding.rs` holds the
  weights next to the profile: `potion-code-16m-v2` uses 0.25 for both shapes,
  and `static-retrieval-v1` and unmeasured profiles use 0, because no constant
  works across models.
- Per query, a dense leader whose best cosine beats the runner-up file by a
  ratio of 1.05 gains a full first-place vote and moves to the first eligible
  position. The ratio held for static and transformer score scales alike;
  smaller leads produced every measured loss.
- Scores are reassigned by position, so the adaptive score filter and the
  learned reranker see the distribution they were tuned on. For a long query a
  position has two scores, with and without the long-query lexical vote,
  because the score filter judges both; a moved file takes over both, so the
  stage changes the order of results and not which ones are returned. The file
  that holds a pinned exact-symbol definition is never moved down, except for
  pasted source. It is protected by file, not by position: the file-coherence
  boost runs after the pin and can put a file with more matching chunks above
  it, and that file is reordered like any other.

The weights were fit on the reranker-fit half of public-core plus half of the
stackoverflow-qa, codefeedback-mt, and apps samples of the `sota-challenge`
benchmark profile, and fit again, unchanged, after the long-query lexical vote
landed. Those corpora have one answer per file and no path roles, so
the eligibility rules above come from the file-localization benchmark and the
self-repository relevance gate. To calibrate another embedding profile, fit the
two weights the same way, keep a weight only when every tuning task stays
non-negative, and check both repository benchmarks.

`src/search_presentation.rs` selects representative spans, loads source text,
and builds explanations. Output records source signals and whether neural
retrieval was requested and executed.

Learned file reranking uses canonical two-line context (`-C 2`) for features
and line-based tie breaks. Requested display previews and spans are applied
after ranking, preserving the existing default-context results. Nondefault
context requests retain a second snippet until rendering; both snippets come
from the same file read, with no additional file I/O. Model weights, routing,
candidate budgets, and rerank gates are unchanged.

The published learned-reranker report is historical evidence from v0.10.1,
not an acceptance result for the current ranking stack. The learned stage
currently runs only on the literal/error route. Its public training corpora
use synthetic `documents/<position>.<extension>` paths, so path-derived
weights need repository-level validation before they can be changed.
Removing those weights improved issue-to-file localization but caused a
significant loss on a separate codefeedback-st holdout. The shipped weights
and quality floors therefore remain unchanged; use
`IVYGREP_RERANKER=deterministic` to compare repository queries without the
learned stage.

Setting `IVYGREP_RERANKER_CAPTURE=1` enables an opt-in diagnostic record at the
hybrid search rerank decision point. A single JSON line prefixed with
`IVYGREP_RERANKER_CAPTURE` and a tab is written to stderr, separate from normal
stdout. It includes the schema version, process ID, query, model identity,
feature schema, and actual accepted pre-learned file candidates with canonical
previews and native feature vectors. The query in the record is the requested
one, also for pasted error output, whose features use the text with runtime
values removed. Ineligible routes and rerank gates emit
an explicit skipped status. Records contain query text and source content,
including canonical context even when display context is zero. Training
collectors must verify a fresh matching process/query record and reject
missing, skipped, or ambiguous captures. With capture unset, no diagnostic
records or feature copies are allocated or written.

Live source reads use `workspace_file.rs` to open regular files beneath the
selected workspace without following child symlinks. Preview metadata and text
come from the same opened file; unavailable live previews use indexed text.

Literal and regex previews return lines up to 1 KiB unchanged. A longer line,
such as a minified bundle or source map, keeps a 1 KiB window around its first
match (context lines keep their start), cut on character boundaries and marked
with `…`, so one hit cannot return megabytes of text. Line numbers and the line
count stay unchanged.

Literal searches retain at most the requested hit count per file and per parallel
partial result set. They preserve path/span ordering without materializing every
matching snippet; source-file reads and explicit unbounded output retain their
existing memory costs.

Literal and regex searches also verify files the indexer walked but stored
without chunks, such as minified bundles. Candidates are Merkle snapshot paths
from the last completed publication that have no effective chunks (overlay
chunks plus untombstoned base chunks), so ignore and exclude decisions are the
indexer's own rather than a second walk's. Coverage is cached per publication
(index and base generations plus the snapshot file identity) and skipped while a
publication marker exists. Literal search drops empty files and files with a NUL
in the sniffed prefix once per publication. Regex search also walks once per
publication for files the snapshot never recorded: files over the 16 MiB
indexing limit, files created since publication, and ignored files when a query
skips ignore rules the index applied. Each query rechecks those walked files
against live ignore rules, so a new exclude hides them even when reindexing has
nothing to publish. Above 4,096 unindexed files, literal keeps indexed
candidates and regex walks the workspace.

Literal, regex, symbol, and caller commands have specialized paths where their
contracts differ from hybrid semantic search. They still reuse workspace,
filtering, grouping, and output types where appropriate.

Symbol rows (`symbols` in SQLite) store the case-folded lookup key, the
exact-case definition `name` when it differs from that key, and an optional
enclosing `owner` (class, impl, struct, module, or Go receiver); language and
kind are read from the joined chunk row, and a `chunk_key` index keeps file
removal proportional to the file's own symbols. Names come
from the Tree-sitter capture at chunk time; the line heuristic is only a
fallback for languages without a grammar, and continuation windows never
register symbols. Qualified lookups (`Owner.method`, `Owner::method`,
`Owner#method`, `Owner->method`) filter by owner, preferring exact-case matches
and falling back to the bare name; reference and caller scans are restricted
to the languages that define the symbol. Adding these columns bumped the index
format to v22.

Format v23 rebuilds existing indexes so unchanged Swift and Objective-C files
receive corrected structural chunks and parser-derived symbol names/owners.
This uses the normal full-index rebuild path, including vector regeneration;
there is no parser-specific partial migration.

Format v27 rebuilds previously truncated Unicode symbol keys, including unchanged
files. Symbol normalization preserves Unicode identifier characters, combining
marks, and JavaScript joiners; case-insensitive lookup still folds ASCII only.
Leading and trailing dollar sigils retain their existing bare-name aliases.

Reference searches use indexed identifier candidates, then verify source syntax.
`--refs` includes non-call uses such as callbacks and function values; `--callers`
returns chunks containing calls. Whitespace, newlines, and generic arguments do
not need to be adjacent to the name. Tree-sitter excludes declaration names,
including Go type, alias, and interface method names, comments, and literal
text; files without a usable parse use quote/comment masking and a conservative
declaration/call heuristic. Qualified references match the immediate textual
receiver name (ignoring scoped generic arguments), not an inferred receiver
type. This is best-effort syntax lookup, not compiler resolution of imports,
aliases, overloads, or shadowed bindings. Go generic calls
that parse as conversions or indexed expressions require a matching indexed
generic-function declaration; ambiguous calls to external generic functions
without indexed definitions remain references only. Bounded requests widen
indexed candidate batches after rejected matches, up to 25,000 chunks. CLI `--no-limit` retains its 50,000-candidate
ceiling; unbounded API requests (`limit: None`) scan all indexed literal candidates.
Each candidate file is parsed at most once for occurrence matching with the
chunker's parse budget. Go generic-function evidence is parsed separately
from matching indexed definition chunks.

## Context-pack pipeline

Context packs answer a different question from ranked search: which bounded set
of evidence helps an agent implement a task safely?

Context seeds and live graph expansion use the same hierarchical ignore policy as
indexing, including `.ignore`, Git excludes, and deleted-file paths. Request-local
matchers cache directory rules without scanning the repository.

1. `src/context_input.rs` parses task text, explicit paths, stack traces, Git
   changes since a base, staged changes, dirty files, and untracked files.
2. Search finds primary implementations and task anchors.
3. `src/context_graph.rs` expands bounded relationships for definitions,
   references, callers, dependencies, dependents, tests, configuration,
   documentation, and recent co-change.
4. `src/context.rs` assigns evidence roles, removes redundant spans, balances
   primary and supporting files, and trims rendered output to the requested
   token budget.
5. Markdown and JSON output include paths, line ranges, roles, reasons, signals,
   change coverage, and budget use.

Dependency extraction is deliberately bounded. Resolved and unresolved import
specifications are stored with lookup keys so newly added files can replace
lower-priority targets without reparsing every unchanged source. Content-only
target edits retain existing edges; deletions, restoration, and manifest changes
refresh affected owners. Missing an edge does not prove no relationship exists.

Python imports and Objective-C quoted local `#import`/`#include` directives are
extracted from parsed syntax. Strings, docstrings, and comments do not create
dependency facts; Objective-C++ also excludes directives inside C++ raw strings.

Rust `self::` and `super::` imports follow the module tree, not the directory
tree: `super::Config` in `src/a/b.rs` resolves to `src/a.rs` or `src/a/mod.rs`.
The line scanner cannot see inline module scope, so indented declarations such
as `use super::helper` inside `mod tests` create no edge. Files without a
conventional parent module file, such as `#[path]` modules, get no `self::` or
`super::` edges either; `self::` in a `mod.rs` still resolves beside the file.

Stack-trace frames under `node_modules`, `site-packages`, `dist-packages`, the
Cargo registry, or the Go module cache map only by their package-relative path,
and `rustc` toolchain frames are ignored. `--since` accepts one commit-ish, such
as `HEAD~3` or `@{upstream}`, and rejects ranges and negated references.

## Worktree overlays

A Git worktree does not copy its repository's complete index. It uses:

- base workspace SQLite, Tantivy, and vectors for unchanged content
- `overlay.sqlite3` for divergent chunks and tombstones
- `overlay_tantivy/` for divergent lexical documents
- `overlay_vectors.usearch` for divergent hash vectors
- `base_ref.json` to record base generation and identity

Search merges base and overlay results, hides deleted or shadowed base paths, and
rejects stale or malformed overlay references that could expose content absent
from the active worktree. Base generation or incarnation changes trigger
reconciliation before overlay content is trusted. A base rebuild can reuse a
generation number, so that counter alone is not an identity. Legacy references
without an incarnation reconcile once; unchanged files still use the shared base.

## Daemon, watchers, and background work

Web context generation follows the daemon's workspace-lease-before-CPU order,
retaining both resources through model preparation and context assembly.

Daemon owns long-lived state that should not be recreated for every query:

- workspace watchers and adaptive debounce
- bounded indexing and enhancement jobs
- reusable search contexts
- query-result and neural-query-vector caches
- Web server sessions
- status, progress, and repair information

Watchers coalesce bursts and cap continuous-event starvation. Successful changes
invalidate only cache entries involving affected workspaces. No-op indexing
preserves valid cache entries.

Cached Git workspace resolution checks filesystem identity and small Git metadata
files without launching Git on unchanged searches. Non-Git paths repeat root
discovery so a newly created ancestor repository is recognized. A changed cached
identity triggers an exclusive index scan before hybrid searches resume; switching
from a linked worktree to a main checkout also replaces the obsolete overlay
stores.

Watcher health is the daemon's job, not the client's. At startup and every 30
seconds a supervisor registers a watcher for each enabled, indexed workspace
that has none; a registration that fails (inotify limits, a missing root) is
recorded in the workspace job ledger and retried with exponential backoff (30 s
doubling to 15 min). The watcher heartbeat re-creates its ledger record when an
index rebuild wiped `job.json`, so a running watcher never reads as offline. A
client that sees `watch_enabled` without `watcher_alive` sends `EnsureWatcher`;
the daemon answers immediately and registers in the background. Clients restart
the daemon only on a protocol version mismatch or when it reports an older build
than the client; a newer daemon speaking the same protocol is used as-is, so
clients left over from before an upgrade do not keep killing it.

A watcher whose workspace root no longer exists, such as a deleted Git worktree,
is released. The watch worker releases it when an update fails because the root
is gone, and the supervisor pass covers roots that vanished without an event
reaching the worker. Release stops the watch backend with its thread and
descriptors, the heartbeat, and the retry loop, and drops the workspace's cached
contexts and its full-index bookkeeping. The same path can be checked out again
while that runs, so release never takes state away from a returning root: the
replacement marker and the resolution entry stay, because they are how a
different checkout at the same path is recognized (the resolution cache is an
LRU), a registration failure found after the release belongs to the returning
root, and if the root is already back only the stale watcher goes. Nothing
retries a missing root and it gets no backoff entry. The job ledger records
once, as a failed watcher
whose error says that the workspace directory no longer exists, so `ig --status`
keeps showing why the workspace is not watched; later passes leave the record
alone. The index and `watch_enabled` stay, so the same path checked out again is
watched by the next request or supervisor pass, and that registration replaces
the record. A root that exists but cannot be watched or read is not gone: it
keeps the recorded failure, the retries, and the 30 s to 15 min backoff.

Indexes of workspaces that no longer exist are garbage collected
(`src/index_gc.rs`). A pass removes an index directory only when its root has
been missing, not merely unreadable, for the grace period, no overlay with a
live root still reads it as its base, nobody holds its `index.lock` or
`enhancement.lock`, and no index or enhancement job is active. The first pass
that finds a root missing records the time in `.root_missing_since` inside the
index directory, so the grace period survives daemon restarts, and a root that
comes back clears it. The grace period is `IVYGREP_INDEX_GC_GRACE_SECS`, seven
days by default because a missing root can be a detached disk or a network
mount; `0` disables collection. The overlay of a linked worktree that `git
worktree list` in its repository no longer reports is gone for certain and
waits ten minutes, or the grace period if that is shorter. A worktree whose
directory vanished without `git worktree remove` stays listed as prunable and
keeps the full grace period. The daemon runs a pass every quarter of the grace
period, between five seconds and ten minutes. For each index that is due it
first takes the exclusive mutation lease of the workspace, as `Remove` does, so
a search that still reads the stores finishes and no request starts on them.
With the lease, `index.lock`, and `enhancement.lock` held it checks the root
once more, and for the ten-minute rule asks Git once more: listing the indexes
and waiting for the lease took time, and a checkout that came back meanwhile
keeps its index. Then it releases everything it keeps for that workspace ID
(the watcher, cached contexts, resolution entries, and a replacement marker
left by a root that never came back) and removes the stores. Whatever returns
at that path later starts from an empty index. `ig --gc` runs one pass with the
same rules and reports what it removed, what is still waiting, and what is in
use. It asks a running daemon to make the pass (`CollectOrphanedIndexes`),
because the daemon holds readers and watchers of the very indexes that go, and
makes it in its own process only when no daemon runs. A daemon from before the
request answers `invalid daemon request`; `ig --gc` stops that daemon, as a
protocol change would have, and then makes the pass itself. While an index
waits, `ig --status` lists the workspace as not watched with `workspace
directory no longer exists`, the watcher failure described above. Collected
workspaces leave `ig --status` and `ig_status`.

On Linux with glibc an idle daemon returns freed memory to the OS. glibc keeps
freed memory in one malloc arena per thread, up to eight per core, and a daemon
that served a burst from many sessions has about a hundred threads that each
touched one: after 32 sessions stopped calling, it kept 668 MiB of anonymous
memory, nearly all of it free space inside 127 arenas. The daemon counts
requests (IPC and Web UI), index runs, and watch updates, samples the count
every 30 seconds, and calls `malloc_trim` once after two quiet samples, 60 to
90 seconds without activity.
It never trims while requests arrive. In the same workload a daemon with the
trim went from 697 MiB to 228 MiB 80 seconds after the last request. Capping
the arenas instead (`MALLOC_ARENA_MAX=4`) kept memory low under load too but
cost 16% of the throughput and doubled the median search latency in the same
run. This concerns glibc builds, that is builds from source and the CUDA
archive. The static musl archives that `install.sh` and the Homebrew formula
install compile the trim out and do not need it: musl's allocator has no arenas
and gives memory back as it is freed, and an idle musl daemon stayed between
174 and 185 MiB through an hour of 64 busy sessions, at the price of an
allocator lock that every thread shares.

An auto-spawned daemon writes to `daemon.log` in the app home. A client rotates
a log over 10 MiB to `daemon.log.1` when it spawns a daemon, and on Unix a
running daemon checks once a minute and does the same, redirecting its own
stdout and stderr to the fresh file. Output that goes to a terminal or a
service manager is never redirected.

Search responses never wait on background enhancement bookkeeping. After the
hits are computed, the daemon schedules a blocking task that checks whether
hash or neural enhancement is needed and queues the workspace, at most once per
workspace and mode every ten seconds.

Enhancement runs in worker processes, and at most `IVYGREP_ENHANCE_MAX_WORKERS`
of them (default 2) run at once per lane, for all workspaces of the app home.
Hash workers and neural workers are separate lanes, and a workspace that waits
for a neural place gets its hash vectors from a hash worker meanwhile, so a
fresh worktree has semantic search without waiting behind a long neural run.
The daemon keeps the workspaces that need work in a queue
(`enhancement_queue.rs`) and starts a worker only when a worker place is free.
The workspace that was searched or edited last goes first, and a workspace
whose directory is gone leaves the queue. A worker takes its place through a
lock file under `$IVYGREP_HOME/enhancement-slots/` before it loads a model or
opens a store. So the limit also holds for workers that `ig` or an MCP session
starts without the daemon, a worker that dies frees its place, and a worker
that finds the host under memory, battery, or load pressure waits with nothing
loaded and without a place. The guards follow the pass, not the worker: a
neural worker starts with the hash pass under the hash tier's guards, so hash
vectors are built on battery, and checks the neural tier's guards before it
loads the model; held back there, it gives up its place and takes one again
when the guards allow. `--wait-for-enhancement` says when its worker is queued.

Heavy work is bounded by a CPU-permit semaphore sized to the core count. Index,
search, and watcher tasks take their per-workspace lease on the blocking pool
first and only then a CPU permit, so requests parked behind an exclusive index
lease never pin CPU capacity that other workspaces could use. Concurrent `Index`
requests for one workspace coalesce: the first request leads, identical requests
await its response, and a request that waited while the index generation
advanced skips the redundant rescan.

`ig --doctor` checks workspace health and can repair stale daemon state or broken
indexes. Status distinguishes lexical readiness, hash coverage, neural coverage,
active jobs, stalled work, watcher health, and compaction recommendations.

## Running ivygrep in many agent sessions

Every Claude Code or Codex session that configures `ig --mcp` starts its own
MCP process. All of them share one daemon per `IVYGREP_HOME`, which the first
call auto-spawns. Thirty-two sessions started at once with no daemon spawn
several daemon processes, and the single-instance lock leaves exactly one
within two seconds.

**Shared, in the daemon:** indexes, watchers (one per indexed workspace), the
query model, search contexts (32), query results (128), the preview cache
(64 MiB), CPU permits (one per core) for searches, context packs, and index
runs, and background enhancement workers, which are separate processes: at most
two hash and two neural workers at once, however many workspaces were edited.

**Per session:** one MCP process that frames JSON-RPC and forwards hybrid,
literal, and regex searches and context packs to the daemon. Symbol, reference,
and caller lookups and `ig_status` run in the MCP process and open stores only
for the call. A session holds no connection between calls: each call connects,
asks, and disconnects.

Measured on Linux x86_64 with a 33 MB corpus and the `static-retrieval-v1`
neural profile, the default at the time
([report](benchmarks/daemon-soak.md#many-mcp-sessions)):

| State of one `ig --mcp` process | Anonymous RSS | PSS | Threads | Descriptors |
| --- | --- | --- | --- | --- |
| after `initialize` | 1.4 MiB | 2.4 MiB | 3 | 9 |
| after 10,000 hybrid searches | 1.9 MiB | 3.5 MiB | 3 | 9 |
| after 501 context packs | 2.3 MiB | 4.1 MiB | 3 | 9 |
| after one search with no daemon (local fallback) | 46 MiB | 50 MiB | 28 | 9 |
| after local literal, regex, and context packs | 88 MiB | 92 MiB | 28 | 9 |

Busy sessions hold a little more than idle ones: 50 sessions requesting context
packs held 92 MiB of anonymous memory and 196 threads together, and 64 sessions
issuing mixed calls for two hours held 179 MiB and 880 descriptors, 2.8 MiB
each, without growth. The daemon is the large process: 290 MiB of anonymous
memory with one session and eight workspaces, about 800 MiB while 64 sessions
keep it busy, and about 230 MiB once it has been idle for 60 to 90 seconds on
Linux with glibc. The local fallback is the expensive session state, and a
process that entered it keeps the memory until the session ends. A session
falls back when no daemon answers: `IVYGREP_NO_AUTOSPAWN` is set, the daemon is
restarting, its 512 connection slots are taken, or a connection attempt times
out after two seconds.

Which Linux build runs the daemon decides both its memory and its latency, more
than any knob below
([measured](benchmarks/daemon-soak.md#allocators-the-shipped-musl-build-glibc-and-one-arena-per-core)).
The release archives are static musl builds. musl's own allocator takes one
lock for every thread, which made those builds two to five times slower than a
glibc build as soon as several threads allocate: a full index of this
repository took 30 s against 5 s, and a daemon under 64 busy sessions served 34
calls per second against 159. The 64-bit musl builds therefore use jemalloc
(`src/main.rs`): the same index takes 6 s, the daemon serves 65 calls per
second, it holds about 200 MiB when idle and 425 MiB under that load, and an
idle `ig --mcp` session costs 1.8 MiB instead of 1.2. C and C++ dependencies
(SQLite, the vector index) still allocate through musl, which is why a glibc
build remains faster. jemalloc fixes its page size when it is built and aborts
at startup on a kernel with larger pages, so the aarch64 musl build is made for
64 KiB pages (`.cargo/config.toml`), which also runs on 4 and 16 KiB kernels.
`ig hardware` reports the allocator and that page size, and the release and E2E
workflows assert them, because QEMU runs with 4 KiB pages and would accept any
build. A glibc build (from source, or the CUDA archive) keeps more memory:
about 600 to 800 MiB under the same load, about 230 MiB after the idle trim,
and an idle footprint that grows with sustained load, about 150 bytes per call
over two hours of saturating load, because freed memory fragments across about
a hundred arenas. Memory in use does not grow: malloc's own count stayed flat
over hundreds of thousands of calls of every kind. The macOS allocator was not
measured.

The daemon inherits the environment of the session that spawns it. Set the
variables below the same way in every session, or the daemon runs with
whatever the first caller had.

| Knob | Why it matters with many sessions |
| --- | --- |
| `IVYGREP_HOME` | Sessions share a daemon, indexes, and caches only when they share this. |
| `IVYGREP_INDEX_GC_GRACE_SECS` | How long the index of a deleted directory stays. Default seven days; overlays of worktrees removed with `git worktree remove` go after ten minutes. |
| `IVYGREP_ENHANCE_MAX_WORKERS` | Enhancement workers that run at once per lane (hash, neural), for all workspaces together. Default `2`. Twenty edited worktrees queue in the daemon, and the one searched or edited last goes first. Raise it on a large host if vectors lag behind edits. |
| `IVYGREP_ENHANCE_MAX_LOAD_RATIO` | Background hash and neural enhancement pause above this load average per core. Default `2.0`. A worker that has not started yet waits without a model or stores in memory. |
| `IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT` | Turns enhancement off entirely; searches stay lexical plus whatever vectors exist. |
| `IVYGREP_MCP_INDEX_WAIT_SECS` | How long a call waits for a first index before it answers `status: indexing`. |
| `IVYGREP_SEARCH_DEADLINE_SECS` | Bounds a daemon search so one slow query cannot hold a CPU permit for minutes. Context packs have no deadline. |
| `MALLOC_ARENA_MAX` (glibc builds only) | Not an ivygrep variable. Freed memory stays in the malloc arenas of the daemon's threads until the daemon goes idle and trims them, and what the trim cannot return grows with sustained load. One arena per core (`16` on the measured host) cost about 10% of the calls at unchanged median latency, lowered memory under load by a fifth, and halved the growth of the idle daemon. `4` cost 16% of the calls and doubled the median latency, `2` two thirds of the calls. Worth setting to the core count for a glibc daemon that stays busy for days. musl ignores it. |
| inotify limits (Linux) | Each watched workspace uses one inotify instance and one watch per directory. `fs.inotify.max_user_instances` defaults to 128 on many distributions. |

**Cleanup.** A session exits when its client closes stdin or stdout or is
killed; it has no state of its own to clean. The daemon never exits when idle.
It releases the watcher, thread, and descriptors of a workspace whose
directory is gone, stops retrying it, and removes its index after the grace
period; see [Daemon, watchers, and background work](#daemon-watchers-and-background-work).
Agent worktrees under `<repo>/.claude/worktrees/` are workspaces of their own:
they reuse the base index through an overlay and never enter the base index.
`daemon.log` rotates at 10 MiB while the daemon runs. `ig --gc` removes the
indexes of deleted directories by hand, and `ig --rm PATH` removes one index.

**Upgrades.** A session started before an upgrade keeps its old binary until
the client restarts it. The first call from a new session replaces an older
daemon (that one call runs in-process), and the new daemon keeps serving the
old sessions as long as it supports their protocol version, which
`MIN_DAEMON_PROTOCOL_VERSION` states. Old sessions keep their old behavior,
including in-process context packs, until they restart.

## Protocols and compatibility

### Daemon IPC

Daemon uses a versioned JSON-line request envelope. Protocol version 6 added
request IDs and explicit cancellation for hybrid, literal, and regex searches;
version 7 added the fire-and-forget `StartIndex` request (answered with
`IndexStarted`) and the `index_in_flight` runtime-status field; version 8 added
`EnsureWatcher`, which re-registers missing watchers instead of restarting the
daemon; version 9 adds `ContextPack`, which builds a context pack in the daemon
and answers with the serialized bundle. `DAEMON_PROTOCOL_VERSION` in
`src/protocol.rs` holds the current value, and `MIN_DAEMON_PROTOCOL_VERSION`
the oldest client version a daemon still serves. Version 9 only added a
request, so a daemon serves version 8 clients unchanged: an MCP session that
was running before an upgrade keeps using the new daemon. Without that, every
call of such a session would read the new daemon as incompatible and restart
it, taking down the daemon the new sessions use. A version 9 client that meets
a version 8 daemon restarts it, as before, and that one call runs in-process. Cancellation
also removes queued searches from daemon CPU backpressure. Existing requests
cover version/status, indexing, Web startup, workspace removal, collection of
orphaned indexes (`CollectOrphanedIndexes`), watcher
recovery (`EnsureWatcher`), restart, progress, and structured errors. A client that reaches a daemon speaking an
older protocol (for example a development build with the same build version)
gets a structured version error from the `Version` probe and restarts it.

Cancellation acknowledgements for active searches are sent after registered work
has stopped. Pre-registration cancellations use bounded tombstones so reordered
IPC connections cannot revive stale searches.

Every search also carries a server-side cancellation token. The connection
handler races the search against the client stream reaching EOF and cancels
abandoned work on disconnect; CLI and MCP searches send request IDs and issue
`CancelSearch` when they time out or drop the request. A per-request deadline
(`IVYGREP_SEARCH_DEADLINE_SECS`, default 60 s, `0` disables) cancels long
searches and returns the hits gathered so far with a `warnings` entry. Web
hybrid, literal, and regex searches carry the same token and deadline. A plain
`/api/search` request runs to completion and answers even a client that
half-closed its write side after sending the request. Event-stream searches
write a keep-alive comment every second and drop the search, cancelling it,
when a write fails. Web context requests have no deadline; dropping an
abandoned context stream ends its lease and CPU waits and sets the cancel token
its retrieval searches check.

`warnings` on search results is additive and omitted when empty, preserving
compatibility with older response readers. Unsupported protocol versions and
stale daemon build versions fail explicitly.

Unix uses a mode-`0600` local socket plus peer-UID checks. Windows uses a
loopback TCP endpoint protected by a per-daemon token, read in each
connection's own task so a stalled client cannot delay other accepts. Requests
are capped at 1 MiB and open connections at 512. A connection past the cap
waits up to 2 s for a slot, then gets a busy error in reply to its request.
CLI, TUI, and MCP searches fall back to local search on that error; index,
status, and `--web` requests report it. A `Version` probe still gets the real
version, because clients restart a daemon whose probe fails.
Beyond 64 waiting connections, new connections are closed without a reply.
A served connection that sends no complete request line within 10 s is closed
without a reply, so connections that never send a request cannot hold slots.
The bound covers only the wait for a request, not running requests such as
long searches or indexing.

### MCP

MCP uses JSON-RPC 2.0 over stdio and accepts newline-delimited or
`Content-Length` framing. A message may be a JSON-RPC batch, which protocol
2025-03-26 requires: the reply is one array without entries for notifications,
nothing when no entry remains, and a single Invalid Request error for an empty
batch.

One thread keeps reading stdin while a worker thread runs requests one at a
time in arrival order. The reader answers `ping` and undecodable messages at
once and applies `notifications/cancelled` as it arrives. At most 64 requests,
counting batch members, and 16 MiB of payload wait for the worker, though an
empty queue takes one message of any size. Past either limit the reader stops
reading until the worker takes the next message. A cancelled request gets no
response. If it is still queued, it never starts. If it is running, its cancel
token trips. Local hybrid, literal, and regex searches stop, including those
inside a context pack. Its daemon search gets `CancelSearch`, and the
first-index wait returns. `initialize` is never cancelled. Symbol, reference,
and caller lookups, and an in-process first index (used only when no daemon
answers), run to completion. A newline-delimited message over 16 MiB gets a
`-32700` parse error with a null id, and the server skips to the next line.
Malformed `Content-Length` framing still ends the session, because there is no
safe point to resume. At EOF, requests already read still run before the server
exits. A reply that can't be written ends the session at once, even while stdin
stays open. It exposes:

- `ig_search` for hybrid, literal, regex, symbol, caller, and context-pack work
- `ig_status` for indexed-workspace and runtime state

Context packs are built by the daemon through the `ContextPack` request, so
the query model, the preview cache, and the index thread pools live there once
instead of in every session, and the build takes the daemon's workspace lease
and a CPU permit. The pack travels as the JSON value an in-process build would
embed, which keeps the tool result byte-identical. A cancelled request cancels
the daemon build through its request id, and a client that disconnects ends
it, as for searches. The in-process builder is the fallback when no daemon
answers.

MCP can auto-index a requested workspace, so search is idempotent but not
read-only. A first index is never awaited for its full duration: `ig_search`
enqueues it on the daemon with `StartIndex` (coalesced with any in-flight run
for that workspace), polls `RuntimeStatus.index_in_flight` for at most
`IVYGREP_MCP_INDEX_WAIT_SECS` (default 20 s), and otherwise returns a non-error
`status: indexing` payload (`progress`, `elapsed_secs`, `retry_after_secs`) read
from the shared job ledger and progress file. The MCP process indexes in-process
only when no daemon is reachable, so it never duplicates a run the daemon owns.
Tool failures return structured MCP errors. Handler panics are isolated instead
of terminating the session.

### Web

Daemon serves embedded assets and APIs for status, search, streaming search,
file reads, editor launch, and workspace trees. File operations enforce tracked
workspace containment.

Loopback is default. Every listener, loopback included, requires a generated
per-daemon session token: any local user can reach a loopback port. API clients
may send it as a bearer token. Loopback listeners accept only loopback Host
names. Content Security Policy, security headers, and request, header, file,
and concurrency limits apply to every listener. Transport is still plain HTTP;
remote use requires a trusted network or encrypted tunnel.

`ig --web` prints the tokenized URL to its own stdout. To open a browser it
writes an HTML redirect page to the app home's `browser/` directory and passes
only that file to `xdg-open`, `open`, or `ShellExecuteW`, so the token never
reaches process arguments that other users can read through `/proc` or `ps`.
On Unix the directory must be owned by the user and is kept at mode `0700`, and
the file is created with mode `0600`; on Windows the file inherits the app
home's ACL. A later launch removes redirect files older than two minutes.
`IVYGREP_NO_BROWSER` skips both the file and the launch. Sandboxed browsers
that cannot read hidden directories under the home directory, such as
snap-packaged Firefox and Chromium on Ubuntu, fail to load the redirect file
under `~/.local/share`; open the printed URL instead. WSL gets no special
handling: the Linux `file://` URL goes to `xdg-open`. `wslview` and xdg-utils
1.2.1 convert it with `wslpath` for a Windows browser. An opener that passes
the URL to a Windows browser unchanged, such as `BROWSER` naming a Windows
executable, points the browser at a missing `C:` path; open the printed URL
instead.

A `/?token=...` request answers with a small same-origin page that sets the
HttpOnly `SameSite=Strict` cookie `ivygrep_session_<port>` and meta-refreshes to
the same URL without the token. A redirect would lose the cookie: opened from
the `file://` page, the navigation is cross-site, and the browser does not send
the new `SameSite=Strict` cookie on the redirected request. The refresh is a
same-origin navigation and needs no inline script under the CSP. The cookie name
carries the listener port because browsers share a host's cookies across ports;
other services on the same host still receive the cookie.

A daemon runs one Web listener. `ig --web` reuses it when `--host` and `--port`
match (port `0` matches any port, and any loopback address matches a loopback
listener) and otherwise fails naming the active address.

## Embeddings and build profiles

Every index can use lightweight hash vectors. Neural-enabled builds also support
pinned model-backed profiles:

- default 256-dimensional Model2Vec profile `potion-code-16m-v2` (33.5 MB download)
- `static-retrieval-v1`, the default through v1.2.16 (125.7 MB), and `potion-code` v1
- optional 384-dimensional Candle transformer profiles
- platform acceleration through Accelerate, Metal, or CUDA builds

Profile name, model revision, dimensions, pooling, normalization, and weight
digest form the neural identity. A mismatch prevents incompatible vectors from
being reused: search ignores them and answers from lexical, literal, symbol, and
hash evidence, `--force-neural` reports the incompatible model, and the next
neural enhancement deletes the store and re-embeds every chunk. A changed
default profile therefore needs no index format bump. Model-backed profiles
download pinned assets on first use unless the Hugging Face cache is already
populated; `scripts/cache_neural_model.py --profile potion-code-v2` fills a
cache for offline hosts.

## Environment variables

ivygrep has no configuration file. Flags control per-command behavior; these
variables tune runtime defaults. "Set" means present with any value, including
`0`. Invalid numeric values fall back to the default.

| Variable | Effect |
| --- | --- |
| `IVYGREP_HOME` | Data directory for indexes. Default `~/.local/share/ivygrep` on every OS, or `$XDG_DATA_HOME/ivygrep` when `XDG_DATA_HOME` is set. Empty values are ignored. |
| `IVYGREP_MODEL_PROFILE` | Neural profile: `potion-code-16m-v2` (default, also `potion-code-v2`; Model2Vec, 256 dimensions, CPU), the opt-in static profiles `static-retrieval-v1` (also `static`; the default through v1.2.16) and `potion-code-16m-v1`, or transformer profiles `general`, `code`, and `code-hq` (384 dimensions). CUDA and Metal builds accelerate only transformer profiles. Unknown values use the default. Vectors from another profile are not reused. |
| `IVYGREP_CUDA_LIBRARY_PATH` | Library search path `ig hardware` checks for CUDA runtime libraries, in place of `LD_LIBRARY_PATH`, before falling back to `ldconfig`. `install.sh` instead treats it as an exclusive colon-separated search path, without checking `ldconfig` or standard CUDA directories; an incomplete override can select the portable build. Empty values are ignored. |
| `IVYGREP_AGENT_HOME` | Home directory `ig agent install` and `ig agent doctor` use to find client configuration files. Default: the user's home directory. |
| `IVYGREP_RERANKER` | `learned` (default; also `auto`) or `deterministic` (also `disabled`, `off`). Unknown values report an error in status and use `learned`. |
| `IVYGREP_RERANK_LIMIT` | Fused candidates the reranker reorders per query. A positive integer overrides the routed default: with the learned reranker, 100 for natural-language, docs/tests/examples, and mixed queries and 30 for identifier, path, and literal or error queries; 30 for every query with the deterministic reranker. Other values are ignored. |
| `IVYGREP_SEARCH_DEADLINE_SECS` | Server-side daemon search deadline. Default `60`; `0` disables it. Hits gathered before the deadline return with a warning. |
| `IVYGREP_MCP_INDEX_WAIT_SECS` | Time an MCP call waits for a first index before returning `status: indexing`. Default `20`; `0` returns immediately. |
| `IVYGREP_INDEX_GC_GRACE_SECS` | How long a workspace root must stay missing before its index is removed. Default `604800` (seven days); `0` disables collection. Overlays of worktrees removed with `git worktree remove` wait at most ten minutes. |
| `IVYGREP_DISABLE_BACKGROUND_ENHANCEMENT` | Set to disable background hash and neural enhancement. `--wait-for-enhancement` fails. |
| `IVYGREP_NO_AUTOSPAWN` | Set to prevent daemon auto-start. Also disables background enhancement, so `--wait-for-enhancement` fails. |
| `IVYGREP_INDEX_THREADS` | Indexing worker threads. Default: physical cores, capped at logical cores. Background hash and neural enhancement insert vectors through at most four of these threads; `1`, or a store under 1,024 vectors, inserts serially. |
| `IVYGREP_NEURAL_THREADS` | Neural inference threads. Default: logical cores capped at 8 for foreground work; a quarter of logical cores (1 to 8) for background work. Maximum 32. |
| `IVYGREP_NEURAL_BATCH_SIZE` | Chunks per background neural enhancement batch. Default depends on backend (static, CPU, Metal, or CUDA); maximum 4096. |
| `IVYGREP_NEURAL_MEMORY_MB` | Memory budget that sizes transformer worker pools. Default: a quarter of available memory. |
| `IVYGREP_NEURAL_FOREGROUND_ACCELERATOR` | `0`, `false`, `no`, `off`, or `cpu` runs query-time neural embedding on CPU. Unset or any other value uses the preferred backend (Metal or CUDA when available). Background enhancement always uses the preferred backend. |
| `IVYGREP_NEURAL_ACCELERATOR_HANDLES` | Embedder handles in the background Metal or CUDA pool. Default `2`, capped by the neural thread count; positive values are clamped to 1 to 8. Foreground embedding uses one handle, and CPU backends size their pool from `IVYGREP_NEURAL_THREADS` and memory instead. |
| `IVYGREP_DISABLE_QUERY_CACHE` | Set to disable the daemon query-result cache. |
| `IVYGREP_ENHANCE_ON_BATTERY` | `1`, `true`, `yes`, or `on` keeps neural enhancement running on battery power (macOS). The hash tier never pauses for battery. |
| `IVYGREP_ENHANCE_MAX_LOAD_RATIO` | Load-average multiple of CPU count that pauses background enhancement on macOS and Linux. Default `2.0`; `0` or below disables the check. |
| `IVYGREP_ENHANCE_MAX_WORKERS` | Background enhancement worker processes that run at once, per lane (hash, neural), across all workspaces of the app home. Default `2`, at most `64`. Further workspaces wait in the daemon's queue, most recently searched or edited first. The daemon reads it from the environment it was started with. |
| `IVYGREP_WEB_EDITOR`, `IVYGREP_EDITOR` | Command the Web UI uses to open files, checked in that order before `EDITOR`, `VISUAL` (terminal editors skipped), and detected GUI editors. The TUI uses `EDITOR` or `VISUAL`. |
| `IVYGREP_NO_BROWSER` | Set to stop `ig --web` from opening a browser. |

Installers also read `IVYGREP_INSTALL_DIR`, `IVYGREP_VERSION`, `IVYGREP_BASE_URL`
(download base URL instead of the tagged GitHub release), and
`IVYGREP_INSTALL_ARCHIVE` with `IVYGREP_INSTALL_CHECKSUM` (install a local
archive and its checksum file instead of downloading). Set `IVYGREP_VERSION` to
the archive's release tag with a local archive: when it is unset, both installers
first look up the latest release on GitHub, so an offline install fails.
`install.sh` defaults the checksum to the archive path plus `.sha256`, and
additionally reads `IVYGREP_ACCELERATOR` and `IVYGREP_CUDA_LIBRARY_PATH`.

## Module ownership

| Concern | Modules |
| --- | --- |
| Configuration and paths | `config.rs`, `workspace.rs` |
| Walking and chunking | `walker.rs`, `chunking.rs`, `text.rs` |
| Index orchestration | `indexer.rs` |
| Index storage concerns | `src/indexer/compression.rs`, `src/indexer/git_state.rs`, `src/indexer/resources.rs`, `src/indexer/staging.rs`, `src/indexer/storage.rs` |
| Background enhancement | `src/indexer/enhancement.rs`: vector-writer lock, publication into the captured store incarnation, and the worker places of `IVYGREP_ENHANCE_MAX_WORKERS`; `enhancement_queue.rs`: the daemon's queue of workspaces that wait for a worker place |
| Job state | `jobs.rs`: background job ledger and status records |
| Index garbage collection | `index_gc.rs`: removes indexes whose workspace root stayed missing past the grace period |
| Change detection | `merkle.rs` |
| Contained source reads | `workspace_file.rs`: live reads beneath a selected workspace root, rejecting symlinks and non-regular files |
| Embeddings and vectors | `embedding.rs`, `vector_store.rs`, `vector_store/` |
| Neural metadata | `neural_metadata.rs`: read and atomically publish neural model identity files |
| Hybrid search | `search.rs`, `search_execution.rs`, `search_fusion.rs`, `search_presentation.rs`, `search_routing.rs`, `search_service.rs` |
| Search constraints | `search_eligibility.rs`: ignore, type, scope, glob, hidden-path, and key filters applied before bounded candidate heaps; `search_boolean.rs`: explicit `AND`/`OR`/`NOT` parsing and candidate pools; `search_semantic_visibility.rs`: exact-score refill when filters reject ANN hits; `path_glob.rs`: `--include`/`--exclude` glob matching |
| Query expansion | `query_aliases.rs`: token and phrase aliases generated from `assets/query_aliases.toml` |
| Learned reranking | `reranker.rs`: embedded linear model, deterministic fallback, and native capture records |
| Preview cache | `search_file_cache.rs`: byte-bounded LRU of file previews shared by search contexts |
| Exact and symbol search | `regex_search.rs`, `symbols.rs` |
| Context packs | `context_input.rs`, `context_graph.rs`, `context.rs` |
| Runtime surfaces | `cli.rs`, `daemon.rs`, `mcp.rs`, `tui.rs`, `web.rs`, `protocol.rs`, `ipc.rs` |
| Agent setup | `agent.rs`: `ig agent install` and `ig agent doctor` client configuration |
| Health and hardware | `doctor.rs`: `ig --doctor` inspection and `--fix` repair; `hardware.rs`: `ig hardware` report; `allocator.rs`: the allocator that report names, and jemalloc's page size; `system_resources.rs`: available-memory probes |
| Editor and browser launch | `launcher.rs`: TUI and Web editor commands and browser opening |
| Frontend | `web/src/main.ts` plus focused API, type, rendering, viewer, icon, clipboard, and UI modules |

Several orchestration modules remain large because they encode coupled ranking,
graph, daemon, or workspace invariants. Split them only around a tested behavior
boundary. File size alone is not sufficient reason for a refactor.

## Correctness and evidence gates

Use the narrowest relevant check while iterating, then run repository gates:

```bash
./test.sh --quick
./test.sh
./scripts/e2e_all.sh --binary target/release/ig
./bench.sh
```

Coverage includes fresh and incremental indexing, worktree overlays, storage
migrations, corrupt artifacts, retrieval quality, deterministic ranking, daemon
recovery, MCP sessions, Web APIs, browser behavior, installer artifacts, and
release workflows. Neural backend acceptance separately forces neural retrieval
and requires `neural_executed: true`; model caches can be pre-populated for
offline checks.

The daemon/local equivalence harness also runs a seeded worktree lifecycle
campaign (nine steps by default). Every step checks the base and both worktrees
against freshly built standalone indexes, including overlay/tombstone storage
invariants. Live edits and offline edits followed by a daemon restart must become
visible without an explicit reindex. For a longer reproducible campaign:

```bash
python3 scripts/check_daemon_equivalence.py --skip-build --binary target/release/ig \
  --bench-home /tmp/ivygrep-lifecycle --worktree-seed 42 --worktree-seed 20260902 \
  --worktree-steps 90
```

Operation journals under the benchmark home identify the seed and failing step.
Literal and regex comparisons are exhaustive; randomized vector comparisons
cover each visible or deleted path separately. Broad-query top-k equality is
not an invariant because BM25 statistics differ between layers and a full index.
This campaign checks content visibility, not equivalence of relevance scores.

Performance and relevance changes need comparable before/after evidence. Keep
hardware, corpus, model, build profile, query set, warmup, and concurrency fixed.
Synthetic scale measurements are not semantic-quality evidence.

## Safe change rules

- Treat index format, neural identity, and daemon protocol as compatibility
  contracts.
- Preserve lexical availability when changing enhancement or model loading.
- Preserve staging, journaling, and commit order when touching persistence.
- Test base, overlay, deletion, and stale-generation behavior for worktree work.
- Keep partial-workspace warnings visible across CLI, daemon, MCP, TUI, and Web.
- Measure relevance before changing routing, fusion, candidate limits, or score
  ordering.
- Validate generated `web/dist` bytes whenever frontend source changes.
- Update this document when module ownership or a cross-store invariant changes.
