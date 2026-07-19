<p align="center">
  <img src="assets/logo.png" alt="ivygrep logo" width="150" />
</p>

<p align="center">
  <strong>Local code search and task context for coding agents.</strong><br/>
  Search by intent, inspect exact matches, and build context packs without uploading source.
</p>

<p align="center">
  <a href="https://github.com/bvolpato/ivygrep/actions/workflows/ci.yml"><img src="https://github.com/bvolpato/ivygrep/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://github.com/bvolpato/ivygrep/actions/workflows/security.yml"><img src="https://github.com/bvolpato/ivygrep/actions/workflows/security.yml/badge.svg" alt="Security" /></a>
  <a href="https://github.com/bvolpato/ivygrep/actions/workflows/relevance.yml"><img src="https://github.com/bvolpato/ivygrep/actions/workflows/relevance.yml/badge.svg" alt="Relevance" /></a>
  <a href="https://github.com/bvolpato/ivygrep/releases/latest"><img src="https://img.shields.io/github/v/release/bvolpato/ivygrep?color=34d058" alt="Latest release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT license" /></a>
</p>

<p align="center">
  <img src="assets/hero-banner.png" alt="ivygrep semantic search returning matching code" width="800" />
</p>

<p align="center">
  <a href="https://bvolpato.github.io/ivygrep/">Website</a> ·
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

```text
src/auth/refresh.rs:118
fn rotate_refresh_token(...)

Context pack: 14 snippets / 7,642 estimated tokens
Coverage: changed files, definitions, callers, dependents, tests
```

The search finds relevant code without requiring an identifier. The context command combines the task with branch changes, dirty files, and code relationships in a bounded Markdown pack. Each snippet says why it was selected and how it relates to the task.

## Install

```bash
brew install bvolpato/tap/ivygrep
```

```bash
curl -fsSL https://raw.githubusercontent.com/bvolpato/ivygrep/main/install.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/bvolpato/ivygrep/main/install.ps1 | iex
```

Installers select a compatible archive, verify its SHA-256 checksum, install `ig`, and report the selected backend. Apple Silicon uses Metal. NVIDIA Linux hosts use the Linux x86_64 CUDA build when CUDA 13 and compute capability 8.0 or newer are available. Other systems use portable local inference. Run `ig hardware` to see detected hardware, compatibility limits, and the matching reinstall command.

Build from source:

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

Useful controls include `-n` for result files, `-C` for context lines, `--type` for language, `--include` and `--exclude` for path globs, plus `--lexical-only`, `--hash`, `--json`, and `--no-index`. Run `ig --help` for the full reference.

## Connect coding agents

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

Recommended agent instruction:

```text
Use ivygrep before broad filesystem scans. Pass absolute active worktree path.
Use natural-language queries for concepts and literal=true for identifiers.
For implementation, request output=context_pack with budget_tokens=8000.
```

## What ivygrep adds

| Capability | `rg` | Hosted search | ivygrep |
|---|:---:|:---:|:---:|
| Natural-language intent | No | Limited | Yes |
| Exact and semantic retrieval | Exact | Varies | Yes |
| Task/diff context packs | No | No | Yes |
| Definitions, callers, tests, config, docs | No | Varies | Yes |
| Git worktree overlays | No | No | Yes |
| Local-only code and queries | Yes | No | Yes |
| CLI, MCP, and Web | CLI | Web/API | Yes |

ivygrep combines Tantivy BM25, exact lookup, USearch ANN, Tree-sitter chunks, a SQLite relationship graph, local Candle embeddings, and Git-aware incremental indexes. Lexical results are available while neural vectors build in the background. Ranking is deterministic.

ivygrep supports 45 language and file types. Twenty-four use Tree-sitter AST chunking, including Rust, Python, Go, JavaScript, TypeScript, Java, C/C++, C#, Kotlin, Scala, PHP, Ruby, Swift, Elixir, Zig, Bash, Haskell, OCaml, Lua, Dart, Objective-C, Perl, and Starlark.

## Measured performance

The deterministic one-million-chunk benchmark currently records 15.07 ms warm p95, 109,006 chunks/s controlled indexing, and a 0.46 GiB final index. Hardware, repository shape, model, and index state affect results. See the [benchmark dashboard](https://bvolpato.github.io/ivygrep/benchmarks/evidence-dashboard.html) and [raw evidence](https://bvolpato.github.io/ivygrep/benchmarks/evidence-dashboard.json).

## Local and private

Code, queries, embeddings, and indexes stay local. Neural mode downloads pinned model assets once. Use `--hash` or a hash-only build to avoid model downloads.

`ig --web` binds to loopback by default. A non-loopback listener prints an authenticated URL but still uses plain HTTP. Use a trusted network, Tailscale, or an encrypted tunnel, and never expose the listener directly to the internet. File contents, including non-ignored dotfiles, can appear in the local index and snippets.

Report vulnerabilities through a [private security advisory](SECURITY.md). Release archives include checksums, SBOMs, and provenance.

## Contribute

```bash
./test.sh --quick
./test.sh
./bench.sh
```

Start with a [good first issue](https://github.com/bvolpato/ivygrep/labels/good%20first%20issue), read [CONTRIBUTING.md](CONTRIBUTING.md), or discuss an idea in [Discussions](https://github.com/bvolpato/ivygrep/discussions).

MIT licensed. Maintained by [Bruno Volpato](https://github.com/bvolpato).
