<p align="center">
  <img src="assets/logo.png" alt="ivygrep logo" width="180" />
</p>

<p align="center">
  <strong>Semantic code search that never uploads your code.</strong><br/>
  Ask questions in English. Get answers in code. Local inference.
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

## ⚡ Quick Start

**Install via Homebrew (recommended):**
```bash
brew tap bvolpato/tap
brew install bvolpato/tap/ivygrep
```

**Install a pre-built binary:**
```bash
tag=$(curl -fsSL https://api.github.com/repos/bvolpato/ivygrep/releases/latest | sed -n 's/.*"tag_name": "\(v[^"]*\)".*/\1/p')
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=linux-x86_64-musl ;;
  Linux-aarch64|Linux-arm64) target=linux-aarch64-musl ;;
  Darwin-x86_64) target=macos-x86_64 ;;
  Darwin-arm64) target=macos-aarch64 ;;
  *) echo "unsupported platform: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac
curl -fsSL "https://github.com/bvolpato/ivygrep/releases/download/${tag}/ivygrep-${tag}-${target}.tar.gz" \
  | tar xz --strip-components=1 "ivygrep-${tag}-${target}/ig"
mkdir -p ~/.local/bin
install -m 0755 ig ~/.local/bin/ig
```

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

That's it. No config files, no setup wizards, no prompts, no API keys. On first run, `ig` auto-indexes the workspace and spawns a background daemon for incremental updates. Neural mode downloads its model artifacts into the Hugging Face cache on first use; `--hash` and hash-only builds require no model download.

<p>
  <img src="assets/ig-demo.gif" alt="ivygrep demo — searching the opencode repo" width="700" />
</p>

---

## 🤖 MCP Server — Supercharge Your AI Agent

ivygrep is the **retrieval layer your coding agent is missing**. Instead of stuffing entire files into context, your agent pulls only the relevant code chunks natively.

```bash
ig --mcp    # starts MCP server on stdio
```

### One-line setup for agents:

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
<summary><b>OpenCode & Codex</b></summary>

**OpenCode:** `opencode mcp add` -> Choose `Local` and set command to `ig --mcp`.

**Codex:** Run `codex mcp add ig -- ig --mcp` or add to `~/.codex/config.toml`.
</details>

---

## 🤔 What is ivygrep?

**ivygrep (`ig`)** is a local-first code search tool that understands natural language. It combines lexical search (like `grep`/`rg`) with semantic vector search — so you can search your code the way you *think* about it.

Traditional tools require you to know _exactly_ what you're looking for. ivygrep lets you search with intent.

| Feature | `grep` / `rg` | GitHub Search | **ivygrep** |
|---------|:---:|:---:|:---:|
| Works offline | ✅ | ❌ | ✅ |
| Natural language queries | ❌ | ⚠️ | ✅ |
| Semantic understanding | ❌ | ❌ | ✅ |
| Warm indexed query latency | ✅ | ❌ | ✅ |
| Privacy-first (no upload) | ✅ | ❌ | ✅ |
| Git-native (worktrees, branches) | ❌ | ❌ | ✅ |
| Structural code chunking | ❌ | ❌ | ✅ |
| Incremental indexing | ❌ | ❌ | ✅ |
| MCP server for AI agents | ❌ | ❌ | ✅ |

### 🌍 44 Language/File Types Supported
ivygrep indexes and structurally chunks 44 language/file types today:

- **Tree-sitter AST chunking (20 languages):** Rust, Python, Go, JavaScript, TypeScript, Java, C, C++, C#, Scala, PHP, Ruby, Swift, Bash, Haskell, OCaml, Lua, Dart, Objective-C, Perl
- **Heuristic structural chunking:** the remaining supported languages below

- **Systems:** Rust, C, C++, Zig, Nim
- **Backend:** Python, Go, Java, Kotlin, Scala, C#, Ruby, PHP, Perl, Groovy
- **Web & Mobile:** JavaScript, TypeScript, HTML, CSS, GraphQL, Swift, Dart, Objective-C
- **Functional:** Haskell, OCaml, Elixir, Erlang, Clojure
- **Data, Scripting & Config:** R, Julia, Bash/Shell, PowerShell, Lua, SQL, Protobuf, Thrift, Terraform, Dockerfile, Makefile, Markdown, XML, TOML/YAML/INI/env config, JSON, plain text

Unknown extensions are auto-detected and indexed as text.

---

## 🚀 Performance & Speed

Benchmarked on the **Linux kernel** (93,493 indexed files, 4,666,431 chunks) and **2GB+ monorepos** (289K files, 3.8M chunks):

| Scenario | Metric | Result |
|------|------|-----:|
| Fresh Linux kernel index | full rebuild | ~27.3 min |
| Cold semantic query | process-cold CLI | ~402 ms |
| Warm daemon semantic query | p95 latency | ~4.9 ms |
| Warm daemon correctness guard | daemon/local hits | 20 / 20 |

The latest benchmark loop reduced Linux kernel fresh-index primary score by 10.6% and daemon hot-query p95 from ~455 ms to single-digit milliseconds. Benchmark writeups and charts live under [`docs/benchmarks/`](docs/benchmarks/).

Indexing is sub-second for most small projects. Large repos return hash/BM25 results immediately and upgrade in the background via the locally cached Candle model (`AllMiniLML6V2`). macOS release builds use Accelerate-backed CPU math; Metal is available as an opt-in local build while its background throughput is tuned.

---

## 🏗️ Architecture & Git-Native Intelligence

ivygrep deeply understands git. This is a core design decision, not an afterthought:
- **Worktree overlays:** Doesn't duplicate indexes contextually. Creates thin overlays mapping divergent chunks.
- **Branch-switch deltas:** Targets Merkle-diff re-indexes of *only* changed files upon branch switch.
- **Content-based deduplication:** Byte-identical files are never re-indexed across branches.
- **`.gitignore` native:** Respects rules automatically at every level.

**Tech stack:** `tantivy` (BM25), `usearch` (vector store), `tree-sitter` (AST), `candle_embed` / `candle-core` (local neural embeddings), `xxh3` (SIMD hashes).

---

## 🔒 Security & Privacy

ivygrep runs search and embedding inference locally and never sends your code, queries, or index data to an external service. A few things worth knowing:

- **Where data lives:** the index (which stores the *decompressed source text* of every indexed file) and the daemon socket live under `~/.local/share/ivygrep` (or `$XDG_DATA_HOME`/`$IVYGREP_HOME`). The index directory is `0700` and the daemon socket `0600`, and the daemon verifies the connecting peer's uid — so other local users on a shared host can't read your indexed code or reach the daemon.
- **Model download:** neural mode uses `hf-hub` to download AllMiniLM-L6-v2 model assets on first use and caches them under `$HF_HOME` or `~/.cache/huggingface`. Use `--hash` or a `--no-default-features` build when no model-network access is permitted.
- **Inference backend:** macOS release binaries execute locally with Accelerate-backed CPU math; portable Linux release binaries execute locally on CPU. Source builds can opt into local Metal with `--features accelerate,metal` or CUDA with `--features cuda` on a compatible installation. `ig --status` reports the recorded backend that last generated neural vectors.
- **Secrets in your repo:** ivygrep indexes file *contents*, including config/dotfiles (e.g. `.env`) unless they're gitignored. Those contents are stored in the local index and can appear in search snippets. Keep secrets out of the workspace or in `.gitignore`.
- **MCP scope:** the `ig_search` MCP tool only searches the workspace at the provided `path` — it cannot search across other indexed projects.

---

## 🔧 CLI Reference

```bash
# Core workflow
ig "your query"                    # search current workspace
ig "query" ~/other/project         # search a different workspace
ig --add .                         # register & index a workspace
ig --rm .                          # unregister a workspace
ig --status                        # show workspace health & embedding status
ig --doctor                        # inspect index health for the current workspace
ig --doctor --fix                  # rebuild a broken or stale index

# Search modes
ig --interactive "query"             # interactive TUI with file/snippet browsing
ig --literal "fn_name"               # fast exact-match search (index-backed)
ig --hash "query"                  # force hash embeddings (skip neural)

# Output control
ig -n 5 "query"                    # limit to 5 files
ig -C 4 "query"                    # 4 lines of context
ig --type rust "query"             # filter by language
ig --include "*.rs,*.go" "query"   # include globs
ig --exclude "vendor/**" "query"   # exclude globs
ig --json "query"                  # machine-readable JSON
ig --first-line-only "query"       # compact grep-style output
ig --file-name-only "query"        # file paths only

# Daemon & server
ig --daemon                        # start background watcher
ig --mcp                           # start MCP server (stdio)
```

---

## 🧪 Development

```bash
./test.sh           # fmt, ShellCheck, clippy, unit/integration tests
./build.sh --locked # release binary, Cargo.lock unchanged
./build.sh --locked --features accelerate,metal  # opt-in macOS Metal neural binary
./bench.sh          # critical Criterion benchmark, no stale local baseline comparison
```
The test suite covers unit tests, CLI snapshots, concurrency, golden queries, labeled relevance metrics, incremental CRUD, MCP, daemon recovery, git/worktree behavior, property-based Merkle invariants, and benchmark guards.
Benchmark output reports per-operation latency; short-looking numbers are repeated inside Criterion so actual timed samples remain long enough to be stable.

### End-to-end procedures
```bash
./build.sh
./scripts/e2e_procedures.sh --binary ./target/release/ig

# Opt-in macOS Metal backend validation (downloads local model artifacts on first run)
./build.sh --locked --features accelerate,metal
./scripts/e2e_neural_backend.sh --binary ./target/release/ig --expect-backend "Candle Metal"
```
These smoke tests run against throwaway projects and isolated `IVYGREP_HOME` directories; the neural backend check embeds fixture text locally and verifies recorded backend reporting.

### Stress testing
```bash
./scripts/bootstrap_stress_fixtures.sh
./test.sh --stress
```

## Roadmap

- **More Tree-sitter languages:** expand the AST pipeline to Kotlin, SQL, and additional grammars as high-quality tree-sitter parsers mature.
- **Symbol retrieval:** store symbol tables during chunking, add a second index for definitions, references, and call edges. Enable `symbol`, `refs`, and `callers` workflows without replacing the current hybrid text retrieval.

- **Editor integrations:** VS Code extension and Neovim telescope plugin for in-editor semantic search.
- **Windows support:** resolve usearch/simsimd MSVC compatibility for native Windows builds.
- **Background job resilience:** richer queue diagnostics and resumable worker state across daemon restarts.

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

---

<p align="center">
  Built by <a href="https://github.com/bvolpato">@bvolpato</a> · Released under the MIT License
</p>
