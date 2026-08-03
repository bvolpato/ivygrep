<p align="center">
  <img src="assets/logo.png" alt="ivygrep logo" width="150" />
</p>

<p align="center">
  <strong>Turn coding tasks into bounded, branch-aware context.</strong><br/>
  Search, indexing, and context generation run locally. Optional model profiles download pinned assets on first use.
</p>

<p align="center">
  <a href="https://github.com/bvolpato/ivygrep/actions/workflows/ci.yml"><img src="https://github.com/bvolpato/ivygrep/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://github.com/bvolpato/ivygrep/actions/workflows/security.yml"><img src="https://github.com/bvolpato/ivygrep/actions/workflows/security.yml/badge.svg" alt="Security" /></a>
  <a href="https://github.com/bvolpato/ivygrep/actions/workflows/relevance.yml"><img src="https://github.com/bvolpato/ivygrep/actions/workflows/relevance.yml/badge.svg" alt="Relevance" /></a>
  <a href="https://github.com/bvolpato/ivygrep/releases/latest"><img src="https://img.shields.io/github/v/release/bvolpato/ivygrep?color=34d058" alt="Latest release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT license" /></a>
</p>

<p align="center">
  <img src="assets/hero-workflow.svg" alt="ivygrep search followed by a bounded task-context pack" width="800" />
</p>

<p align="center">
  <a href="https://bvolpato.github.io/ivygrep/">Website</a> ·
  <a href="docs/architecture.md">Architecture</a> ·
  <a href="https://bvolpato.github.io/ivygrep/benchmarks/">Benchmarks</a> ·
  <a href="CONTRIBUTING.md">Contributing</a> ·
  <a href="https://github.com/bvolpato/ivygrep/discussions">Discussions</a>
</p>

## Search and build context

```bash
# Find code by intent
ig "where is refresh token rotated?"

# Build context from code and current changes
ig context "fix refresh-token races" --since main --budget 8000
```

Abridged output:

```text
# ivygrep context
Budget: 7,642 / 8,000 estimated tokens
Coverage: 7 files | 2 primary | 1 definitions | 1 dependencies | 0 dependents | 2 callers | 0 references | 1 tests | 0 config | 0 docs
Candidates: 31 retrieved | 14 selected
## Evidence
### 1. src/auth/refresh.rs:118-166 [primary, definition]
Why: task anchor; changed implementation.
Signals: lexical, symbol, git change.
```

Search answers where. Context answers what an agent needs to change safely.

The context command combines task anchors with commits since the branch point, staged and dirty files,
issue or trace paths, and indexed relationships. It returns one bounded Markdown pack with path, lines,
role, reason, and retrieval signals. `--since` requires a Git worktree; omit it for non-Git directories.

## Install

```bash
# Homebrew on macOS or Linux
brew install bvolpato/tap/ivygrep

# Release installer on macOS or Linux
curl -fsSL https://raw.githubusercontent.com/bvolpato/ivygrep/main/install.sh | sh
```

```powershell
# WinGet on Windows
winget install --id BrunoVolpato.ivygrep --exact

# Release installer on Windows
irm https://raw.githubusercontent.com/bvolpato/ivygrep/main/install.ps1 | iex
```

Installers select a compatible archive, verify its SHA-256 checksum, install `ig`, and report the selected backend. Apple Silicon uses Metal. NVIDIA Linux hosts use the Linux x86_64 CUDA build when CUDA 13 and compute capability 8.0 or newer are available. Other systems use portable local inference. Run `ig hardware` to see detected hardware, compatibility limits, and the matching reinstall command.

Build from source on macOS or Linux:

```bash
git clone https://github.com/bvolpato/ivygrep.git && cd ivygrep
./build.sh
install -m 0755 target/release/ig ~/.local/bin/ig
```

## Search

The first query indexes the current repository. The daemon then watches for changes and updates the index incrementally.

```bash
ig "where is authentication handled?"       # hybrid semantic + lexical
ig --literal "handleAuth"                    # exact indexed lookup
ig --symbol calculate_tax                    # definitions
ig --refs calculate_tax                      # references and calls
ig --callers calculate_tax                   # caller chunks
ig "database migrations" src/api/           # path scope
ig --all "retry policy"                      # all indexed projects
ig --interactive "auth flow"                 # terminal UI
ig --web "auth flow" .                       # local Web UI
```

Useful controls include `-n` for result files, `-C` for context lines, `--type`
for language, `--include`/`--exclude` path globs, `--lexical-only`, `--hash`, and `--json`. `--hash`
uses lightweight local embeddings for faster startup and no model download,
with lower semantic quality. Run `ig --help` for full reference.

## Search notes and memories

ivygrep also indexes Markdown, text, JSON, and other document files. Precompute local vectors once, then search a notes directory by meaning:

```bash
ig --add ~/notes --wait-for-enhancement
ig -n 20 "what did we decide about cache invalidation?" ~/notes
```

Default daemon-backed queries across CLI, MCP, Web, and TUI blend semantic and
lexical retrieval. Implicit questions with overwhelmingly note-like initial
results add two bounded local memory probes. Index stays live as notes change;
after model assets are present, queries, note contents, embeddings, and results
stay local.

On the public [MemoryQuest benchmark](https://bvolpato.github.io/ivygrep/benchmarks/public-memory-retrieval.html), default CLI search retrieved 74.9% of required memories in the top 20 and retrieved every required memory for 44.9% of questions. Warm CLI p95 was 87.63 ms across 535 implicit questions and 3,878 preindexed sessions. This is synthetic personal-assistant data and measures session retrieval, not answer quality. Report pins the v1.2.7 binary SHA-256 and release-tag equivalence evidence.

## Connect coding agents

Codex and Claude Code packages install MCP configuration plus focused task-context skill:

```bash
codex plugin marketplace add bvolpato/ivygrep
codex plugin add ivygrep@ivygrep

claude plugin marketplace add bvolpato/ivygrep
claude plugin install ivygrep@ivygrep
```

Automatic setup detects the client, preserves existing configuration, writes the absolute `ig` path, verifies the MCP handshake, and runs a search:

```bash
ig agent install claude
ig agent install codex
ig agent install cursor
ig agent doctor
```

Restart an open client after installation. Manual MCP setup is also available:

```bash
claude mcp add -s user ig -- ig --mcp
codex mcp add ig -- ig --mcp
gemini mcp add --scope user --transport stdio ig ig --mcp
```

Cursor `.cursor/mcp.json`:

```json
{"mcpServers":{"ig":{"type": "stdio", "command": "ig", "args": ["--mcp"]}}}
```

OpenCode `opencode.json`:

```json
{"mcp":{"ig":{"type": "local", "command": ["ig", "--mcp"], "enabled": true}}}
```

Agents call `ig_search` for discovery. Set `output=context_pack` and `budget_tokens=8000` when the task needs implementation context. Pass the absolute path to the active repository or worktree. Worktrees reuse the base index and store only changed chunks and tombstones.

Context packs can include definitions, callers, references, dependencies, dependents, tests, configuration, and docs.

Setup guides: [Codex](https://bvolpato.github.io/ivygrep/integrations/codex.html), [Claude Code](https://bvolpato.github.io/ivygrep/integrations/claude-code.html), [Cursor](https://bvolpato.github.io/ivygrep/integrations/cursor.html), [Gemini CLI](https://bvolpato.github.io/ivygrep/integrations/gemini-cli.html), [OpenCode](https://bvolpato.github.io/ivygrep/integrations/opencode.html), and [MCP](https://bvolpato.github.io/ivygrep/integrations/mcp.html).

Recommended agent instruction:

```text
Use ivygrep before broad filesystem scans. Pass absolute active worktree path.
Use natural-language queries for concepts and literal=true for identifiers.
For implementation, request output=context_pack with budget_tokens=8000.
```

## How it works

1. A Git-aware walker finds changed or indexable files.
2. Tree-sitter and bounded text fallbacks produce structural chunks.
3. SQLite stores metadata and relationships; Tantivy stores lexical postings; USearch stores hash and optional model vectors.
4. Query routing runs bounded exact, lexical, symbol, hash, and optional neural passes before fusion.
5. Context expands primary hits through code relationships and recent changes,
   then trims rendered evidence to requested token budget.

Fresh indexing publishes lexical results before vector enhancement. Worktrees
reuse base index and store only divergent chunks and tombstones. Partial
workspace failures return warnings with valid hits; complete failure errors.

ivygrep supports 45 language and file types. Twenty-four use Tree-sitter AST chunking:
Rust, Python, Go, JavaScript, TypeScript, Java, C/C++, C#, Kotlin, Scala, PHP,
Ruby, Swift, Elixir, Zig, Bash, Haskell, OCaml, Lua, Dart, Objective-C, Perl, and Starlark.

Read [architecture](docs/architecture.md) for storage, commit order, retrieval,
worktrees, protocols, security boundaries, and module ownership.

## System performance

On the deterministic synthetic one-million-chunk CC0 corpus, v1.2.7 median hash-only warm CLI p95 is 6.19 ms, controlled indexing reaches 150,576 chunks/s, and the final index is 0.42 GiB across three sequential trials. This is a scale and footprint measurement, not semantic quality or agent-task performance. Hardware, repository shape, index state, and load affect absolute results.

[Current-release evidence](https://bvolpato.github.io/ivygrep/benchmarks/public-million-current.json) · [Million-chunk methodology and historical paired study](https://bvolpato.github.io/ivygrep/benchmarks/public-million.html) · [Full benchmark dashboard](https://bvolpato.github.io/ivygrep/benchmarks/evidence-dashboard.html)

## Local and private

Runtime source, queries, embeddings, results, and indexes stay local. Neural
profiles download pinned model assets on first use unless cache is already
populated. Use `--hash`, `./build.sh --hash-only`, or
`cargo build --locked --no-default-features` to avoid model downloads.

`ig --web` binds to loopback by default. A non-loopback listener prints an authenticated URL but still uses plain HTTP. Use a trusted network, Tailscale, or an encrypted tunnel, and never expose the listener directly to the internet. File contents, including non-ignored dotfiles, can appear in the local index and snippets.

Report vulnerabilities through a [private security advisory](SECURITY.md). Release archives include checksums, SBOMs, and provenance.

## Contribute

```bash
./test.sh --quick
./test.sh
./bench.sh
```

Start with a [good first issue](https://github.com/bvolpato/ivygrep/labels/good%20first%20issue), read [CONTRIBUTING.md](CONTRIBUTING.md) and [architecture](docs/architecture.md),
or discuss an idea in [Discussions](https://github.com/bvolpato/ivygrep/discussions).

MIT licensed. Maintained by [Bruno Volpato](https://github.com/bvolpato).
