<p align="center">
  <img src="assets/logo.png" alt="ivygrep logo" width="180" />
</p>

<p align="center">
  <strong>Local semantic code search. No code upload.</strong><br/>
  Ask English questions, get ranked code answers, run inference locally.
</p>

<p align="center">
  <a href="https://github.com/bvolpato/ivygrep/actions"><img src="https://github.com/bvolpato/ivygrep/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://github.com/bvolpato/ivygrep/actions/workflows/security.yml"><img src="https://github.com/bvolpato/ivygrep/actions/workflows/security.yml/badge.svg" alt="Security" /></a>
  <a href="https://github.com/bvolpato/ivygrep/actions/workflows/relevance.yml"><img src="https://github.com/bvolpato/ivygrep/actions/workflows/relevance.yml/badge.svg" alt="Relevance" /></a>
  <a href="https://github.com/bvolpato/ivygrep/releases/tag/v1.1.2"><img src="https://img.shields.io/badge/release-v1.1.2-34d058" alt="Latest Release" /></a>
  <a href="https://github.com/bvolpato/ivygrep/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT" /></a>
  <a href="https://github.com/bvolpato/ivygrep/releases"><img src="https://img.shields.io/github/downloads/bvolpato/ivygrep/total?color=%23ff6f00" alt="Downloads" /></a>
</p>

<p align="center">
  <img src="assets/hero-banner.png" alt="ivygrep semantic code search" width="600" />
</p>

<p align="center">
  <a href="https://bvolpato.github.io/ivygrep/">Website</a> ·
  <a href="https://bvolpato.github.io/ivygrep/benchmarks/">Benchmarks</a> ·
  <a href="AGENT_INTEGRATION.md">AI Agents</a> ·
  <a href="ARCHITECTURE.md">Architecture</a> ·
  <a href="CONTRIBUTING.md">Contributing</a>
</p>

---

## Quick Start

**Homebrew:**
```bash
brew install bvolpato/tap/ivygrep
```

**Linux and macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/bvolpato/ivygrep/main/install.sh | sh
```

**Windows PowerShell:**
```powershell
irm https://raw.githubusercontent.com/bvolpato/ivygrep/main/install.ps1 | iex
```

Installers choose the right archive, verify SHA-256, install `ig`, and print
the installed version. PowerShell also updates the user `PATH`. Windows uses
the same USearch ANN backend as Linux and macOS, Rust-managed persistence,
long-path support, and a statically linked Visual C++ runtime.

Every release ships checksums, SPDX JSON SBOMs, and provenance sidecars. CI
extracts and runs the exact archive bytes before publishing:

| Target | Release behavior | Offline fallback |
|---|---|---|
| Linux x86_64 musl | Static binary, baseline x86-64 exercised under QEMU `qemu64` | Hash search, no model or service |
| Linux aarch64 musl | Static binary exercised under ARM64 QEMU in Alpine | Hash search, no model or service |
| macOS Intel | Native archive with Accelerate-backed local neural inference | Hash search |
| macOS Apple Silicon | Native archive with Accelerate-backed local neural inference | Hash search |
| Windows x86_64 | Native USearch ANN plus local CPU neural inference | Hash search |

Release archive checks cover startup, indexing, hybrid/hash/literal/regex
search, daemon equivalence, status/doctor, stale-index rebuild, and removal.
`ig` needs no Python, compiler, system database, or external service. Neural
mode may download its pinned model once; `--hash` and hash-only builds do not.

Quality, latency, footprint, release size, unavailable comparisons, and the
claim policy live in the
[evidence dashboard](https://bvolpato.github.io/ivygrep/benchmarks/evidence-dashboard.html).

**Build from source:**
```bash
git clone https://github.com/bvolpato/ivygrep.git && cd ivygrep
./build.sh
install -m 0755 ./target/release/ig ~/.local/bin/ig
```

**Developer targets:**
```bash
./build.sh --help
./test.sh --help
./bench.sh --help

./build.sh          # release binary
./build.sh --features accelerate,metal  # opt-in macOS Metal neural inference
./build.sh --features cuda  # opt-in Linux CUDA neural inference
./test.sh --quick   # fast local check
./test.sh           # fmt, clippy, unit/integration tests
./bench.sh          # critical Criterion benchmark, no stale local baseline comparison
```

**Your first search:**
```bash
ig "authentication flow"            # auto-indexes on first run, then searches
ig "error handling" src/api/         # scope to a directory
ig --all "database migrations"      # search across all indexed projects
```

**Web UI:**
```bash
ig --web                            # open http://127.0.0.1:4747
ig --web "authentication flow" .    # open with query + current workspace
ig --web --host 0.0.0.0 --port 4747 # bind beyond loopback
```

The web UI runs from the daemon and embeds into the `ig` binary. By default it
binds `127.0.0.1:4747`, opens in your browser, and searches all indexed
workspaces. Use `--host`/`--port` to choose the listener. `ig --web "query" .`
enables web on the current daemon when possible and opens with that folder
selected.

Web UI capabilities:

- Search all tracked workspaces or focus one workspace/folder.
- See daemon status and workspace health.
- Stream result updates while the search runs.
- Browse indexed folders in the sidebar.
- Open tracked files with syntax highlighting and focused result lines.
- Preview Markdown files, with a source toggle.
- Highlight selected results, show language icons, and keep sidebar/results/file
  panes independently scrollable.

No config or API key is required. First run auto-indexes the workspace and
starts a background daemon for incremental updates. Neural mode may download
model artifacts once; `--hash` and hash-only builds do not.

<p>
  <img src="assets/ig-demo.gif" alt="ivygrep demo: searching the opencode repo" width="700" />
</p>

---

## MCP Server for Agents

Use `ig --mcp` when an agent needs code search without loading whole files into
context.

```bash
ig --mcp    # starts MCP server on stdio
```

Before connecting an agent, run `ig --version` in the same environment that
launches it. GUI applications may not inherit your interactive shell's `PATH`;
use the absolute path to `ig` or `ig.exe` in that case.

### Setup for coding agents

<details>
<summary><b>Claude Code</b></summary>

```bash
claude mcp add -s user ig -- ig --mcp
```
Or add to `~/.claude.json`:
```json
{
  "mcpServers": {
    "ig": { "type": "stdio", "command": "ig", "args": ["--mcp"] }
  }
}
```
</details>

<details>
<summary><b>Cursor</b></summary>

Add to `.cursor/mcp.json` or `~/.cursor/mcp.json`:
```json
{
  "mcpServers": {
    "ig": { "type": "stdio", "command": "ig", "args": ["--mcp"] }
  }
}
```
Then refresh MCP servers in Cursor settings.
</details>

<details>
<summary><b>Gemini</b></summary>

```bash
gemini mcp add --scope user --transport stdio ig ig --mcp
```
Or add to `~/.gemini/settings.json`:
```json
{
  "mcpServers": {
    "ig": { "command": "ig", "args": ["--mcp"] }
  }
}
```
</details>

<details>
<summary><b>Codex</b></summary>

```bash
codex mcp add ig -- ig --mcp
codex mcp get ig --json
```

The CLI and IDE extension share `~/.codex/config.toml`. Trusted repositories
can instead use a project-scoped `.codex/config.toml`.
</details>

<details>
<summary><b>OpenCode</b></summary>

Add to `opencode.json`:
```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "ig": {
      "type": "local",
      "command": ["ig", "--mcp"],
      "enabled": true
    }
  }
}
```
</details>

### Recommended agent behavior

Give the agent this persistent instruction in `AGENTS.md`, `CLAUDE.md`,
`GEMINI.md`, or the equivalent rules file:

```text
Use the ivygrep MCP tools for code discovery before broad filesystem scans.
Pass the absolute current repository or worktree path to ig_search.
Use natural-language queries for concepts and literal=true for exact identifiers.
Use limit to choose retrieval breadth and context to choose source lines per hit.
Start with limit=5-10 and context=2. Increase context when a promising hit needs
more evidence; increase limit when you need more candidate files.
Use ig_status when indexing health is unclear.
```

`ig_search` is restricted to the supplied workspace, auto-indexes on first use,
starts incremental watching, and accepts subdirectory or file paths for narrower
scope. In a Git worktree, pass that worktree's root: ivygrep reuses the shared
base index and stores only overlay deltas and tombstones.

See [Coding agent integration](AGENT_INTEGRATION.md) for verified configs,
tool-selection guidance, worktree behavior, and troubleshooting.

---

## What ivygrep Does

**ivygrep (`ig`)** is local semantic code search. It mixes BM25/literal lookup
with vector search, so queries can describe intent instead of exact tokens.

| Feature | `grep` / `rg` | GitHub Search | [zoekt](https://github.com/google/zoekt) | **ivygrep** |
|---------|:---:|:---:|:---:|:---:|
| Works offline | ✅ | ❌ | ✅ | ✅ |
| Natural language queries | ❌ | ⚠️ | ❌ | ✅ |
| Semantic intent search | ❌ | ❌ | ❌ | ✅ |
| Warm indexed query latency | ✅ | ❌ | ✅ | ✅ |
| No code upload | ✅ | ❌ | ✅ | ✅ |
| Git-native worktrees/branches | ❌ | ❌ | ❌ | ✅ |
| Structural code chunking | ❌ | ❌ | ⚠️ | ✅ |
| Incremental indexing | ❌ | ❌ | ❌ | ✅ |
| MCP server for AI agents | ❌ | ❌ | ❌ | ✅ |

### 45 Language and File Types

ivygrep indexes 45 language/file types. 24 use Tree-sitter AST chunking:
Rust, Python, Go, JavaScript, TypeScript/TSX, Java, C, C++, C#, Scala, Kotlin,
PHP, Ruby, Swift, Elixir, Zig, Bash, Haskell, OCaml, Lua, Dart, Objective-C,
Perl, and Starlark macros/targets in very large BUILD-like sources.

| Group | Languages and formats |
|---|---|
| Systems | Rust, C, C++, Zig, Nim |
| Backend | Python, Go, Java, Kotlin, Scala, C#, Ruby, PHP, Perl, Groovy |
| Web and mobile | JavaScript, TypeScript, HTML, CSS, GraphQL, Swift, Dart, Objective-C |
| Functional | Haskell, OCaml, Elixir, Erlang, Clojure |
| Data, scripting, config | R, Julia, Bash/Shell, PowerShell, Lua, SQL, Protobuf, Thrift, Terraform, Starlark/Bazel, Dockerfile, Makefile, Markdown, XML, TOML/YAML/INI/env config, JSON, plain text |

Unknown extensions are auto-detected and indexed as text.

---

## Performance Evidence

Current retained public evidence:

| Benchmark | Metric | Result |
|------|------|-----:|
| Public million-chunk search | warm distinct-query p95 | 53.77 ms -> 15.07 ms, 3.57x faster |
| Public million-chunk indexing | controlled throughput | 4,963 -> 109,006 chunks/s, 21.96x faster |
| Public million-chunk index size | final index footprint | 1.06 GiB -> 0.46 GiB, -57.0% |
| Public retrieval quality | nDCG@10 / MRR@10 / precision@5 | 0.2666 / 0.2220 / 0.0601 |
| Public retrieval recall | recall@20 / no-hit queries | 0.4890 / 0 |
| v1.0.0 Tantivy postings A/B | hybrid 200 / literal 200 / simple symbol / complex phrase | 3.66 -> 3.50 ms / 2.41 -> 2.05 ms / 3.19 -> 2.96 ms / 4.45 -> 2.79 ms |
| v0.12.20 generated Rust index A/B | fresh index / warm p95 | 6505.5 -> 6354.9 ms / 0.655 -> 0.632 ms |

Large Linux-kernel validation used a checkout with 93,502
indexed files and 4,419,660 chunks:

| Scenario | Metric | Result |
|------|------|-----:|
| Fresh lexical-first Linux kernel index | full rebuild | ~270 sec |
| Large-repo natural query | process-cold p95 | ~137 ms |
| Warm daemon identical-query replay | end-to-end p95 | ~79 ms |
| Warm daemon distinct queries | end-to-end p95 | ~116 ms |
| Portable Linux intent relevance | 13 labeled queries | 41.20 |
| Best retained dedicated-host daemon run | identical-query p95 | ~4.9 ms |
| Historical eager-vector Linux kernel index | full rebuild | ~27.3 min |
| Lexical-first scoped stress probe | 10,501 files | ~3 sec |
| Warm daemon correctness guard | daemon/local hits | 20 / 20 |

Latency depends on CPU, storage, repository shape, index state, and
virtualization. Public quality, latency, refresh, and resource evidence lives
under [`docs/benchmarks/`](docs/benchmarks/).

Indexing publishes BM25/literal search first. A load-aware background process
builds hash ANN vectors, then upgrades to the portable 256-dimensional
`static-retrieval-v1` model selected by the public embedding bake-off. Optional
profiles remain available through `IVYGREP_MODEL_PROFILE`: `potion-code`,
`general`, `code`, and `code-hq`. Model identity is stored with the index, so
incompatible vectors are rebuilt.

Resource knobs:

- `IVYGREP_INDEX_THREADS`: foreground parser workers; defaults to physical cores.
- `IVYGREP_NEURAL_THREADS`: desired transformer worker ceiling.
- `IVYGREP_NEURAL_MEMORY_MB`: smaller explicit memory budget for worker sizing.
- `IVYGREP_NEURAL_BATCH_SIZE`: local benchmark override for background batches.
- `IVYGREP_NEURAL_ACCELERATOR_HANDLES`: shared-model CUDA/Metal concurrency.
- `IVYGREP_NEURAL_FOREGROUND_ACCELERATOR=0`: force CPU query embedding.

CUDA builds read `nvidia-smi` free VRAM, total VRAM, and utilization before
choosing batch size. Linux memory accounting honors effective cgroup limits,
including containers.

Relevance evaluation separates foreground readiness from post-background hash
quality:

```bash
uv run scripts/eval_relevance.py
uv run scripts/eval_relevance.py --enhance-hash
uv run scripts/run_public_benchmark_matrix.py \
  --profile public-core \
  --datasets-root /tmp/ivygrep-public-datasets \
  --work-root /tmp/ivygrep-public-results \
  --output public-code-retrieval-results.json
```

The public matrix pins 20 CoIR task/language variants plus a compact
1,000-query, 48-language baseline. Reports include checksums, quality, variance,
latency, memory, and index size under [`docs/benchmarks/`](docs/benchmarks/).

---

## Architecture and Git

Git behavior is part of the index design:

| Behavior | Current implementation |
|---|---|
| Worktree overlays | One base search index, with per-worktree SQLite, lexical, and vector stores for divergent chunks and tombstones. |
| Branch-switch deltas | Merkle reconciliation re-indexes changed files instead of rebuilding the whole search index. |
| Content-based overlay diff | Byte-identical files do not create worktree overlay chunks. |
| `.gitignore` support | Repository ignore rules apply during file walks. |

**Tech stack:** `tantivy` (BM25), `usearch` (ANN), `tree-sitter` (AST), SQLite
symbol/call graph storage,
`candle_embed` / `candle-core` (local neural embeddings), and `xxh3` hashes.

---

## Security and Privacy

ivygrep runs search and embedding inference locally. It never sends code,
queries, or index data to an external service.

- Compressed source chunks live under `~/.local/share/ivygrep` or the configured
  `$XDG_DATA_HOME`/`$IVYGREP_HOME`. Unix uses an owner-only `0600` socket plus
  peer-uid verification. Windows uses loopback TCP with a per-daemon token
  beside the user-owned index. Keep custom `IVYGREP_HOME` paths private.
- Neural mode downloads revision-pinned assets with `hf-hub` on first use and
  caches them under `$HF_HOME` or `~/.cache/huggingface`. Use `--hash` or a
  `--no-default-features` build when model assets must never be downloaded.
- Release binaries run locally: Accelerate-backed CPU math on macOS, CPU on
  Linux/Windows. Source builds can opt into Metal (`--features
  accelerate,metal`) or CUDA (`--features cuda`). CUDA does not require cuDNN.
  Set `CUDA_COMPUTE_CAP` explicitly when auto-detection is wrong; `ig --status`
  reports the backend that last generated neural vectors.
- Indexing refuses to start below 512 MiB available memory. Background
  enhancement pauses below 1 GiB. Optional transformer workers share model
  weights plus an adaptive memory budget. These checks use native
  available-memory reporting on macOS and Windows and cgroup-aware reporting on
  Linux.
- ivygrep indexes file *contents*, including config/dotfiles such as `.env`
  unless they're gitignored. Those contents are stored in the local index and
  can appear in search snippets. Keep secrets out of the workspace or in
  `.gitignore`.
- `ig_search` only searches the workspace at the supplied `path`.

---

## CLI Reference

```bash
# Core workflow
ig "your query"                    # search current workspace
ig "query" ~/other/project         # search a different workspace
ig --add .                         # register & index a workspace
ig --rm .                          # unregister a workspace
ig --status                        # show workspace health & embedding status
ig --doctor                        # inspect index health for the current workspace
ig --doctor --deep                 # run full cross-store integrity scans
ig --doctor --fix                  # rebuild a broken or stale index

# Search modes
ig --interactive "query"             # interactive TUI with file/snippet browsing
ig --literal "fn_name"               # fast exact-match search (index-backed)
ig --lexical-only "query"          # BM25/path/signature retrieval only
ig --hash "query"                  # force hash embeddings (skip neural)
ig --symbol calculate_tax          # exact definitions
ig --refs calculate_tax            # indexed references/calls
ig --callers calculate_tax         # caller chunks

# Output control
ig -n 5 "query"                    # at most 5 ranked result files
ig -C 4 "query"                    # up to 4 lines before and after each match
ig -n 5 -C 8 "query"               # 5 files with richer snippets
ig --type rust "query"             # filter by language
ig --include "*.rs,*.go" "query"   # include globs
ig --exclude "vendor/**" "query"   # exclude globs
ig --json "query"                  # machine-readable JSON
ig --first-line-only "query"       # compact grep-style output
ig --file-name-only "query"        # file paths only

# Daemon and server
ig --daemon                        # start background watcher
ig --web                           # start daemon + local browser UI
ig --web "query" .                 # preload query and selected workspace
ig --web --host 0.0.0.0 --port 4747
ig --mcp                           # start MCP server (stdio)
```

`--limit` controls retrieval breadth. `--context` controls snippet size.
Neither is a relevance threshold.

| Control | What it changes | Ranking |
|---|---|---|
| `-n N`, `--limit N` | Searches a candidate pool sized for the request and returns at most `N` ranked files | The same relevance signals apply; a deeper pool can slightly change ranks |
| `--no-limit` | Uses maximum candidate budgets and returns every result that survives relevance filtering | Can change ranks and is slower |
| `-C N`, `--context N` | Shows up to `N` source lines before and after each focused match | Unchanged |
| `--first-line-only` | Reduces each result to one preview line after retrieval | Unchanged |
| `--file-name-only` | Returns paths only; without `-n`, the CLI also uses maximum candidate budgets | Unchanged with `-n`; without `-n`, the deeper pool can change ranks |

- Smaller limits truncate ranked files. Larger limits search deeper and can
  slightly rerank top results.
- `--no-limit` uses maximum candidate budgets and can be much slower.
- `-C`, `--first-line-only`, and `--file-name-only -n N` change presentation
  after retrieval.
- `--file-name-only` without `-n` also uses maximum candidate budgets.
- Agents should start with `-n 5` to `-n 10` and `-C 2`. Increase context for
  more lines from the same file; increase limit for more candidate files.
- Scores order one query's results. They are not global confidence values.
- ivygrep does not expose a total token-budget parameter.

---

## Development

```bash
./test.sh           # fmt, ShellCheck, clippy, Rust and Python harness tests
./build.sh --locked # release binary, Cargo.lock unchanged
./build.sh --locked --features accelerate,metal  # opt-in macOS Metal neural binary
./build.sh --locked --features cuda  # opt-in Linux CUDA neural binary
./bench.sh          # critical Criterion benchmark, no stale local baseline comparison
```
The web UI source lives under `web/` and is managed with pnpm. `pnpm -C web
build` writes committed assets to `web/dist/`; Cargo embeds those assets into
the `ig` binary at build time.

Tests cover unit behavior, CLI snapshots, concurrency, golden queries, public
retrieval metrics, symbols/callers, incremental CRUD, MCP, daemon recovery,
git/worktrees, Merkle properties, and benchmark guards. Criterion repeats short
operations inside stable timed samples.

### End-to-end procedures
```bash
./build.sh
./scripts/e2e_procedures.sh --binary ./target/release/ig
python3 scripts/check_daemon_equivalence.py \
  --skip-build \
  --binary ./target/release/ig \
  --bench-home /tmp/ivygrep-daemon-equivalence

# Opt-in macOS Metal backend validation. Downloads local model artifacts once.
./build.sh --locked --features accelerate,metal
./scripts/e2e_neural_backend.sh --binary ./target/release/ig --model-profile general --expect-backend "Candle Metal"

# Opt-in Linux CUDA backend validation. Downloads local model artifacts once.
./build.sh --locked --features cuda
./scripts/e2e_neural_backend.sh --binary ./target/release/ig --model-profile general --expect-backend "Candle CUDA"
```
Smoke tests use throwaway projects and isolated `IVYGREP_HOME` directories. The
neural backend check embeds fixture text locally and verifies backend reporting.

### Stress testing
```bash
./scripts/bootstrap_stress_fixtures.sh
./test.sh --stress
```

## Roadmap

- More Tree-sitter languages: add SQL and other mature grammars.
- Evidence-backed search program: track the quality,
  latency, footprint, and portability work in
  [#128](https://github.com/bvolpato/ivygrep/issues/128).
- Learned reranking: evaluate compact local cross-encoders against the
  bounded deterministic reranker without weakening offline portability.
- Editor integrations: VS Code and Neovim Telescope.
- Background job resilience: better queue diagnostics and resumable worker state.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines.

---

<p align="center">
  Built by <a href="https://github.com/bvolpato">@bvolpato</a> · Released under the MIT License
</p>
