<p align="center">
  <img src="assets/logo.png" alt="ivygrep logo" width="220" />
</p>

<p align="center"><strong>Semantic grep that never phones home.</strong><br/>Feels like <code>rg</code>, understands English.</p>

## Superpower Your LLM

Your coding agent is only as strong as its retrieval toolchain. `ivygrep` is designed to be that retrieval layer.

- Natural-language code search: `where is tax calculated?` can still find `calculateTaxes(...)`.
- Hybrid ranking: lexical BM25 + semantic vectors + RRF fusion.
- Token-efficient context: your agent pulls only relevant chunks instead of stuffing full files into prompts.
- Local-only privacy: no cloud indexing, no code upload.
- Incremental freshness: Merkle-based updates keep search results aligned with current code.

In practice: the agent stops guessing and starts grounding edits in real, scoped code references.

## Why ivygrep

`ivygrep` is a local-first hybrid code search tool:

- Lexical search with Tantivy (BM25)
- Semantic search with local embeddings
- RRF hybrid fusion for high-quality ranking
- Incremental indexing via Merkle-style file fingerprints
- Regex fallback path for grep-like workflows
- Lightweight daemon over Unix socket (opt-in)

No network calls are required for indexing and searching.

## Install

### Homebrew tap (standalone, no cargo)

```bash
brew tap bvolpato/tap
brew install bvolpato/tap/ivygrep
```

### From source (developer workflow)

```bash
git clone https://github.com/bvolpato/ivygrep.git
cd ivygrep
cargo build --release
./target/release/ivygrep --help
```

### Standalone binary path

If you build from source, the standalone executable is generated at:

```text
./target/release/ivygrep
```

You can move it into your PATH:

```bash
install -m 0755 ./target/release/ivygrep ~/.local/bin/ivygrep
```

## Quick Start

```bash
ivygrep --add .
ivygrep "authentication for MCPs"
ivygrep "authentication for MCPs" ~/githubworkspace/opencode
```

`--add` registers the current workspace for indexing and daemon watch updates.

If daemon mode is running, a plain query also auto-indexes the current workspace before searching.

When no daemon is running, first query in a non-indexed workspace prompts:

```text
This folder is not indexed. Index it now? [y/N]
(-f to force, --no-watch to skip daemon)
```

### Usage Demo

`ivygrep` searching the `opencode` repo for `"authentication for MCPs"` in a real terminal session:

<p>
  <img src="assets/ivygrep-opencode.gif" alt="ivygrep demo on opencode" />
</p>

## MCP Server (Agent Integration)

`ivygrep` ships with an MCP server over stdio:

```bash
ivygrep --mcp
```

### Exposed tool

- `ivygrep_search(query, path?, limit?, context?, type?, regex?, include?, exclude?, first_line_only?, file_name_only?, verbose?)`

Behavior:

- If the workspace is not indexed, `ivygrep_search` auto-indexes it on first call.
- If `path` points to a subdirectory or a file, results are restricted to that scope only.
- `.gitignore` is respected during indexing and regex scans.
- Unknown extensions are indexed when content looks like text; binary content is skipped.
- MCP tool output is compact JSON in `content[0].text` (no duplicated text rendering).
- `reason` fields are omitted by default; pass `verbose=true` only when needed.
- `include` / `exclude` accept comma-separated globs (for example `*.md,src/**/*.rs`).

### Claude Code

```bash
claude mcp add -s user ivygrep -- ivygrep --mcp
```

Equivalent user-scope config (`~/.claude.json`):

```json
{
  "mcpServers": {
    "ivygrep": {
      "type": "stdio",
      "command": "ivygrep",
      "args": ["--mcp"],
      "env": {}
    }
  }
}
```

### Cursor

Project or global config (`.cursor/mcp.json` or `~/.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "ivygrep": {
      "command": "ivygrep",
      "args": ["--mcp"]
    }
  }
}
```

Then refresh MCP servers in Cursor settings.

### Codex

If your Codex build supports CLI registration:

```bash
codex mcp add ivygrep -- ivygrep --mcp
```

Equivalent config (`~/.codex/config.toml`):

```toml
[mcp_servers.ivygrep]
command = "ivygrep"
args = ["--mcp"]
```

### OpenCode

```bash
opencode mcp add
```

Then choose `Local` and set command to `ivygrep --mcp`.

Equivalent config (`opencode.json` in project root, or `~/.config/opencode/opencode.json` globally):

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "ivygrep": {
      "type": "local",
      "command": ["ivygrep", "--mcp"]
    }
  }
}
```

### Example agent prompt

`Refactor payment flow. First call ivygrep_search with path=src/payments and find where tax is computed.`

### MCP vs Daemon

- `ivygrep --mcp` starts an MCP server on stdio (for Claude/Cursor/Codex/OpenCode tool calls).
- `ivygrep --daemon` starts the background workspace watcher/indexer over Unix socket for CLI workflows.
- They are independent: MCP does not require daemon, and daemon does not require MCP.
- If you want continuous file-watch reindexing across terminals, run daemon.
- If you only need agent tool calls, run `--mcp` only.

## CLI

```bash
ivygrep "authentication for MCPs"
ivygrep --add .
ivygrep --rm .
ivygrep --status
ivygrep --daemon
ivygrep "authentication for MCPs" ~/githubworkspace/opencode
```

Useful flags:

- `-f, --force`: skip first-query prompt; with `--add`, rebuild from scratch
- `--regex`: regex mode
- `--type <lang>`: language filter (`rust`, `python`, `typescript`, ...)
- `--include <globs>`: comma-separated include globs (for example `*.txt,*.md`)
- `--exclude <globs>`: comma-separated exclude globs (for example `target/**,*.lock`)
- `-C, --context <n>`: context lines around the focused pointer line (default: `2`, i.e. up to 5 lines total)
- `-n, --limit <n>`: max number of files in output (no default limit)
- `--add [path]`: register/index/watch workspace (defaults to `.`)
- `--rm [path]`: remove workspace index/watch registration (defaults to `.`)
- `--status`: show indexed workspaces
- `--daemon`: run daemon process
- `--mcp`: run MCP server on stdio
- `--first-line-only`: print only the first non-empty line of each hit snippet
- `--file-name-only`: print only matching file paths
- `--verbose`: include detailed `reason` pointers for each hit
- `--json`: machine-readable grouped output
- `--no-watch`: skip daemon watcher registration

Action/query split:

- Workspace actions are explicit flags (`--add`, `--rm`, `--status`, `--daemon`), so query text like `add` is never ambiguous.
- `ivygrep <query> <path>` runs semantic search against another workspace without `cd`.

## When to use the daemon

Use `ivygrep --daemon` when you want the best steady-state latency in an active repo:

- You run many queries in sequence and want warm index/search state in memory.
- You want file-watch updates continuously while editing code.
- You want indexing/search shared across terminals and scripts.

Skip daemon mode if you run one-off queries occasionally. The CLI works directly in-process without it.
The daemon is the process that watches registered workspaces and performs background incremental updates.
If you started it and saw no logs before, run it in a terminal and you should now see startup/watch/update lines on stderr.

Typical daemon workflow:

```bash
ivygrep --daemon
ivygrep --add .
ivygrep "where is split assignment handled?"
```

## Result Ranking & Output

- Results are grouped by file by default (not line-first).
- File score is the sum of chunk scores in that file.
- Files are sorted by descending file score.
- By default, each hit prints a concise focused snippet (about 5 lines).
- Use `--verbose` to include `reason` pointer lines.
- Exact lexical/literal matches are weighted higher than fuzzy semantic-only matches.
- A relevance threshold is applied automatically so low-signal chunks are dropped.
- Use `--first-line-only` if you want compact grep-style previews.
- Use `--file-name-only` to list only files and feed them into other tools.
- If you want hard truncation, use `-n`.

## Architecture

- `tantivy` for lexical index/search
- `usearch` for vector index/search
- `notify` for file watching
- SQLite metadata store per workspace
- `.gitignore` rules are respected by default during indexing and regex scans.
- Unknown extensions are indexed when content is detected as text; binary files are skipped.
- Workspace index root: `${IVYGREP_HOME:-${XDG_DATA_HOME:-~/.local/share}/ivygrep}/indexes/<workspace-id>/`
- Path precedence:
  1. `IVYGREP_HOME` (if non-empty)
  2. `XDG_DATA_HOME/ivygrep` (if non-empty)
  3. `~/.local/share/ivygrep`

## Development

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

Test harness includes:

- fixture repositories in `tests/fixtures`
- golden semantic query tests
- CLI snapshot tests
- property-based Merkle diff tests

### Larger stress harnesses

Use medium-size canonical corpora to stress indexing and hybrid retrieval without checking large assets into git.

Included bootstrap targets:

- Project Gutenberg Shakespeare corpus (`pg100`, complete works)
- Project Gutenberg Alice in Wonderland (`pg11`)
- `BurntSushi/ripgrep` (depth-1 clone)
- `quickwit-oss/tantivy` (depth-1 clone)

Bootstrap fixtures locally:

```bash
./scripts/bootstrap_stress_fixtures.sh
```

Run ignored stress tests:

```bash
cargo test --test stress_harness -- --ignored --nocapture
```

Optional custom fixture root:

```bash
IVYGREP_STRESS_ROOT=/tmp/ivygrep-stress ./scripts/bootstrap_stress_fixtures.sh /tmp/ivygrep-stress
IVYGREP_STRESS_ROOT=/tmp/ivygrep-stress cargo test --test stress_harness -- --ignored --nocapture
```

## License

MIT
