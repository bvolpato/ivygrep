# Contributing to ivygrep

Thanks for your interest in contributing to ivygrep! Here's how to get started.

## Quick Start

```bash
# Clone the repo
git clone https://github.com/bvolpato/ivygrep.git
cd ivygrep

# Build (hash-only mode, no model download)
cargo build --no-default-features

# Build with neural search (requires ~134MB model download on first run)
cargo build

# Run tests
cargo test

# Run benchmarks
cargo bench
```

## Development Setup

- **Rust toolchain:** Install via [rustup](https://rustup.rs/). The project uses Rust 2024 edition.
- **Tree-sitter grammars:** Bundled as Cargo dependencies — no external setup needed.
- **Environment variables:**
  - `IVYGREP_HOME` — Override the default index/config directory (useful for testing).
  - `IVYGREP_NO_AUTOSPAWN=1` — Prevent daemon auto-spawning in CI/test environments.

## Project Structure

```
src/
├── main.rs          # Binary entry point
├── cli.rs           # CLI argument parsing & dispatch
├── daemon.rs        # Background daemon (file watching, IPC)
├── indexer.rs       # Core indexing pipeline
├── search.rs        # Hybrid BM25 + vector search engine
├── chunking.rs      # Tree-sitter AST + heuristic code chunking
├── merkle.rs        # Merkle tree for incremental re-indexing
├── mcp.rs           # Model Context Protocol server
├── embedding.rs     # Hash & neural embedding models
├── vector_store.rs  # USearch ANN vector index
├── workspace.rs     # Workspace resolution (git, worktrees)
├── walker.rs        # .gitignore-aware file walker
├── doctor.rs        # Index health inspection & repair
├── jobs.rs          # Job ledger (watcher, indexer, enhancer)
├── ipc.rs           # Unix socket IPC protocol
├── protocol.rs      # Daemon request/response types
├── config.rs        # Configuration management
├── text.rs          # Text utilities (camelCase split, etc.)
├── path_glob.rs     # Glob pattern matching
└── regex_search.rs  # Regex search fallback

tests/
├── cli_snapshot.rs      # CLI E2E with insta snapshots
├── concurrency.rs       # Thread safety & race condition tests
├── git_branch_switch.rs # Branch switch + worktree overlay tests
├── incremental_crud.rs  # Incremental indexing CRUD
├── golden_queries.rs    # Cross-language relevance tests
├── merkle_prop.rs       # Property-based Merkle tests (proptest)
├── mcp_e2e.rs           # MCP server integration
├── stress_harness.rs    # Large-repo stress tests
└── ...
```

## How to Contribute

### Bug Reports
Open an issue with reproduction steps, OS/arch, and `ig --version` output.

### Feature Requests
Open an issue describing the use case, expected behavior, and how it fits the project's local-first, privacy-focused philosophy.

### Code Contributions

1. **Fork & branch:** Create a feature branch from `main`.
2. **Write tests:** Every bugfix needs a regression test. Features need integration tests.
3. **Run the suite:**
   ```bash
   cargo test
   cargo clippy -- -D warnings
   ```
4. **Keep commits focused:** One logical change per commit. Use conventional commit messages.
5. **Open a PR:** Describe what changed and why. Reference any related issues.

### Adding a Tree-sitter Language

1. Add the grammar crate to `Cargo.toml` (e.g., `tree-sitter-kotlin`).
2. Add the language variant to `chunking.rs` (follow the pattern of existing languages).
3. Write a test in `golden_queries.rs` or `cli_snapshot.rs` with a representative code sample.
4. Update the language count in `README.md` and `docs/index.html`.

## Code Style

- Follow `rustfmt` defaults. Run `cargo fmt` before committing.
- Fix all `clippy` warnings. CI will reject PRs with warnings.
- Prefer descriptive variable names over comments explaining *what* — save comments for *why*.
- No `unwrap()` in library code — use `?` or explicit error handling. `unwrap()` is fine in tests.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
