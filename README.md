<p align="center">
  <img src="assets/logo.png" alt="ivygrep logo" width="180" />
</p>

<p align="center">
  <strong>Semantic code search that never phones home.</strong><br/>
  Ask questions in English. Get answers in code. 100% local.
</p>

<p align="center">
  <a href="https://github.com/bvolpato/ivygrep/actions"><img src="https://github.com/bvolpato/ivygrep/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://github.com/bvolpato/ivygrep/releases/latest"><img src="https://img.shields.io/github/v/release/bvolpato/ivygrep?color=%2334d058&label=release" alt="Latest Release" /></a>
  <a href="https://github.com/bvolpato/ivygrep/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT" /></a>
  <a href="https://github.com/bvolpato/ivygrep/releases"><img src="https://img.shields.io/github/downloads/bvolpato/ivygrep/total?color=%23ff6f00" alt="Downloads" /></a>
</p>

<p align="center">
  <img src="assets/hero-banner.png" alt="ivygrep semantic code search" width="600" />
</p>

---

## ⚡ What is ivygrep?

**ivygrep (`ig`)** is a local-first code search tool that understands natural language. It combines lexical search (like `grep`/`rg`) with semantic vector search — so you can search your code the way you *think* about it.

```bash
# Ask in English, find the code
ig "where is tax calculated?"
# → finds calculateTaxes(), applyVAT(), computeWithholding()

ig "database connection retry logic"
# → finds reconnect(), exponentialBackoff(), handleDbTimeout()
```

> **No API keys. No cloud. No telemetry. Your code never leaves your machine.**

---

## 🤔 Why ivygrep?

Traditional code search tools require you to know _exactly_ what you're looking for. ivygrep lets you search with intent.

| Feature | `grep` / `rg` | GitHub Search | **ivygrep** |
|---------|:---:|:---:|:---:|
| Works offline | ✅ | ❌ | ✅ |
| Natural language queries | ❌ | ⚠️ | ✅ |
| Semantic understanding | ❌ | ❌ | ✅ |
| Sub-100ms latency | ✅ | ❌ | ✅ |
| Privacy-first (no upload) | ✅ | ❌ | ✅ |
| AST-aware chunking | ❌ | ❌ | ✅ |
| Incremental indexing | ❌ | ❌ | ✅ |
| MCP server for AI agents | ❌ | ❌ | ✅ |

---

## 🚀 Quick Start

### Install

**Homebrew** (recommended):
```bash
brew tap bvolpato/tap
brew install bvolpato/tap/ivygrep
```

**From source**:
```bash
git clone https://github.com/bvolpato/ivygrep.git && cd ivygrep
cargo build --release
install -m 0755 ./target/release/ig ~/.local/bin/ig
```

**Binary downloads**: grab the latest from [Releases](https://github.com/bvolpato/ivygrep/releases/latest) — available for Linux (x86/ARM) and macOS (Intel/Apple Silicon).

### First search in 10 seconds

```bash
ig "authentication flow"            # auto-indexes on first run, then searches
ig "error handling" src/api/         # scope to a directory
ig --all "database migrations"      # search across all indexed projects
```

That's it. No config files, no setup wizards, no prompts, no API keys. On first run, `ig` auto-indexes the workspace and spawns a background daemon for incremental updates.

> **Tip**: Use `ig --add .` to explicitly register a workspace for watch-based reindexing, or `ig --hash` for fast startup without the neural model download.

<p>
  <img src="assets/ig-demo.gif" alt="ivygrep demo — searching the opencode repo" width="700" />
</p>

---

## 🧠 How It Works

ivygrep uses a **hybrid search architecture** — combining the precision of keyword matching with the intelligence of semantic understanding:

```mermaid
flowchart TD
    Q["Your Query\n'retry logic for payments'"]
    Q --> L["Lexical BM25\n(Tantivy)"]
    Q --> S["Semantic\n(Embeddings)"]
    L --> F["RRF Hybrid Fusion"]
    S --> F
    F --> R["Ranked Results\nwith AST-aware context"]
```

- **Lexical path** — BM25 scoring via [Tantivy](https://github.com/quickwit-oss/tantivy) catches exact keyword matches
- **Semantic path** — vector embeddings (neural ONNX or lightweight hash) capture meaning
- **AST chunking** — [tree-sitter](https://tree-sitter.github.io) splits code into precise function/class boundaries (35+ languages)
- **Incremental indexing** — Merkle-style fingerprints mean re-index only touches changed files

---

## ⚡ Performance

Benchmarked under concurrent AI agent workloads (8 threads, sustained saturation):

| Metric | Value |
|--------|-------|
| **Average latency** | ~26 ms |
| **p95 latency** | ~62 ms |
| **Max latency** | ~98 ms |

> Faster than a network roundtrip. Your agent never waits.

---

## 🤖 MCP Server — Supercharge Your AI Agent

ivygrep is the **retrieval layer your coding agent is missing**. Instead of stuffing entire files into context, your agent pulls only the relevant code chunks.

```bash
ig --mcp    # starts MCP server on stdio
```

### One-line setup for every major agent:

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
    "ig": { "command": "ig", "args": ["--mcp"] }
  }
}
```
Then refresh MCP servers in Cursor settings.
</details>

<details>
<summary><b>Codex</b></summary>

```bash
codex mcp add ig -- ig --mcp
```

Or add to `~/.codex/config.toml`:
```toml
[mcp_servers.ig]
command = "ig"
args = ["--mcp"]
```
</details>

<details>
<summary><b>OpenCode</b></summary>

```bash
opencode mcp add
```

Choose `Local` and set command to `ig --mcp`.

Or add to `opencode.json`:
```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "ig": { "type": "local", "command": ["ig", "--mcp"] }
  }
}
```
</details>

### Example agent prompt

> *"Refactor the payment flow. First call ig_search with path=src/payments to find where tax is computed."*

The agent searches, finds the exact function, and edits grounded in real code — not hallucinations.

### MCP tool

`ig_search(query, path?, limit?, context?, type?, regex?, include?, exclude?, first_line_only?, file_name_only?, verbose?)`

- Auto-indexes on first call
- Scopes to subdirectory or file
- Respects `.gitignore`
- Compact JSON output (token-efficient for LLMs)

---

## 🌍 35+ Languages Supported

ivygrep provides AST-aware chunking for functions, classes, and modules:

| Category | Languages |
|----------|-----------|
| **Systems** | Rust, C, C++, Zig |
| **Web** | JavaScript, TypeScript, HTML, CSS, GraphQL |
| **Backend** | Python, Go, Java, Kotlin, Scala, C#, Ruby, PHP, Perl |
| **Functional** | Haskell, OCaml, Elixir, Erlang, Clojure |
| **Mobile** | Swift, Dart, Objective-C |
| **Scientific** | R, Julia |
| **Shell** | Bash/Zsh, PowerShell, Lua |
| **Data/Infra** | SQL, Protobuf, Terraform, TOML, YAML, JSON |
| **Other** | Markdown, Dockerfile, Makefile, and more |

> Unknown extensions are auto-detected — if it looks like text, it gets indexed.

---

## 🔧 CLI Reference

```bash
# Core workflow
ig "your query"                    # search current workspace
ig "query" ~/other/project         # search a different workspace
ig --add .                         # register & index a workspace
ig --rm .                          # unregister a workspace
ig --status                        # show indexed workspaces
ig --all "query"                   # search all indexed workspaces

# Search modes
ig --regex "fn\s+\w+_tax"          # regex mode (like rg)
ig --hash "query"                  # use fast hash embeddings (no model download)

# Output control
ig -n 5 "query"                    # limit to 5 files
ig -C 4 "query"                    # 4 lines of context
ig --type rust "query"             # filter by language
ig --include "*.rs,*.go" "query"   # include globs
ig --exclude "vendor/**" "query"   # exclude globs
ig --json "query"                  # machine-readable JSON
ig --first-line-only "query"       # compact grep-style output
ig --file-name-only "query"        # file paths only
ig --verbose "query"               # include match reasons

# Daemon & server
ig --daemon                        # start background watcher
ig --mcp                           # start MCP server (stdio)
```

> **Tip**: The daemon auto-spawns on your first search — no manual startup needed. Set `IVYGREP_NO_AUTOSPAWN=1` to disable (useful in CI).

---

## 🏗️ Architecture

```
ivygrep
├── tantivy        — lexical BM25 index
├── usearch        — vector similarity index
├── tree-sitter    — AST-based code chunking (Rust, Python, Go, JS, TS)
├── fastembed      — ONNX neural embeddings (all-MiniLM-L6-v2, 384-dim)
├── notify         — filesystem watcher for live re-indexing
├── SQLite         — metadata store per workspace
└── Unix socket    — daemon IPC (auto-spawned)
```

**Index location**: `${IVYGREP_HOME:-${XDG_DATA_HOME:-~/.local/share}/ivygrep}/indexes/<workspace-id>/`

**Neural embeddings**: The default build bundles ONNX Runtime for high-quality semantic search. The model (~23 MB) downloads automatically on first use. For a minimal binary without ONNX: `cargo build --release --no-default-features`.

---

## 🧪 Development

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
```

The test suite includes **90+ tests** across 6 categories:

| Suite | Tests | Description |
|-------|------:|-------------|
| Unit | 52 | Core logic, chunking, embedding, search, MCP, text |
| CLI snapshots | 9 | End-to-end CLI behavior |
| Concurrency | 6 | Thread safety, parallel search/index |
| Golden queries | 3 | Semantic accuracy validation |
| Incremental CRUD | 13 | Add/update/delete indexing correctness |
| Property-based | 1 | Merkle diff invariants |

### Stress testing

```bash
./scripts/bootstrap_stress_fixtures.sh
cargo test --test stress_harness -- --ignored --nocapture
```

Fixtures include the Linux kernel (ripgrep), Shakespeare corpus, and Tantivy source.

---

## 📄 License

MIT — use it however you want.

---

<p align="center">
  Built by <a href="https://github.com/bvolpato">@bvolpato</a> · Contributions welcome
</p>
