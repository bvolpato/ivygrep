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
ivygrep index .
ivygrep "where is the tax calculated?"
```

First query in a non-indexed workspace prompts:

```text
This folder is not indexed. Index it now? [y/N]
(-f to force, --no-watch to skip daemon)
```

## CLI

```bash
ivygrep "where is the tax calculated?"
ivygrep index .
ivygrep add .
ivygrep rm .
ivygrep status
ivygrep daemon
```

Useful flags:

- `-f, --force`: skip prompt and index now
- `--regex`: regex mode
- `--type <lang>`: language filter (`rust`, `python`, `typescript`, ...)
- `-C, --context <n>`: context lines around hits
- `--json`: machine-readable output
- `--no-watch`: skip daemon watcher registration

## When to use the daemon

Use `ivygrep daemon` when you want the best steady-state latency in an active repo:

- You run many queries in sequence and want warm index/search state in memory.
- You want file-watch updates continuously while editing code.
- You want indexing/search shared across terminals and scripts.

Skip daemon mode if you run one-off queries occasionally. The CLI works directly in-process without it.

## Architecture

- `tantivy` for lexical index/search
- `usearch` for vector index/search
- `notify` for file watching
- SQLite metadata store per workspace
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
