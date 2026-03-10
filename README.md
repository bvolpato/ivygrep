<p align="center">
  <img src="assets/logo.png" alt="ivygrep logo" width="220" />
</p>

<h1 align="center">ivygrep</h1>

<p align="center"><strong>Semantic grep that never phones home.</strong><br/>Feels like <code>rg</code>, understands English.</p>

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
ivygrep "where is the tax calculated?"
```

`--add` registers the current workspace for indexing and daemon watch updates.

If daemon mode is running, a plain query also auto-indexes the current workspace before searching.

When no daemon is running, first query in a non-indexed workspace prompts:

```text
This folder is not indexed. Index it now? [y/N]
(-f to force, --no-watch to skip daemon)
```

## CLI

```bash
ivygrep "where is the tax calculated?"
ivygrep --index .
ivygrep --add .
ivygrep --rm .
ivygrep --status
ivygrep --daemon
ivygrep applyFilter ~/githubworkspace/trino
```

Useful flags:

- `-f, --force`: skip prompt and index now
- `--regex`: regex mode
- `--type <lang>`: language filter (`rust`, `python`, `typescript`, ...)
- `-C, --context <n>`: context lines around the focused pointer line (default: `2`, i.e. up to 5 lines total)
- `-n, --limit <n>`: max number of files in output (no default limit)
- `--index [path]`: explicit index/reindex workspace (defaults to `.`)
- `--add [path]`: register/index/watch workspace (defaults to `.`)
- `--rm [path]`: remove workspace index/watch registration (defaults to `.`)
- `--status`: show indexed workspaces
- `--daemon`: run daemon process
- `--first-line-only`: print only the first non-empty line of each hit snippet
- `--file-name-only`: print only matching file paths
- `--verbose`: include detailed `reason` pointers for each hit
- `--json`: machine-readable grouped output
- `--no-watch`: skip daemon watcher registration

Action/query split:

- Workspace actions are explicit flags (`--add`, `--rm`, `--status`, `--daemon`, `--index`), so query text like `add` is never ambiguous.
- `ivygrep <query> <path>` runs semantic search against another workspace without `cd`.

## When to use the daemon

Use `ivygrep --daemon` when you want the best steady-state latency in an active repo:

- You run many queries in sequence and want warm index/search state in memory.
- You want file-watch updates continuously while editing code.
- You want indexing/search shared across terminals and scripts.

Skip daemon mode if you run one-off queries occasionally. The CLI works directly in-process without it.

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
- Workspace index root: `~/.local/share/ivygrep/indexes/<workspace-id>/`

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

## License

MIT
