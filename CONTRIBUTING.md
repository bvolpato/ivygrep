# Contributing to ivygrep

Small fixes, tests, docs, platform validation, and focused features are welcome.

## Find work

- Start with [`good first issue`](https://github.com/bvolpato/ivygrep/labels/good%20first%20issue) or [`help wanted`](https://github.com/bvolpato/ivygrep/labels/help%20wanted).
- Use [Discussions](https://github.com/bvolpato/ivygrep/discussions) for questions and early ideas.
- Use an [issue form](https://github.com/bvolpato/ivygrep/issues/new/choose) for reproducible bugs or concrete features.
- Comment before starting an existing issue so work does not overlap.

Open an issue or discussion before large architecture, dependency, storage-format,
ranking, or CLI compatibility changes.

## Development setup

Required:

- Git
- stable Rust through [rustup](https://rustup.rs/)
- Python 3 for repository harnesses
- ShellCheck for full script validation
- Node.js and pnpm 10 only when changing `web/`

Repository toolchain configuration installs `rustfmt` and Clippy automatically.

```bash
git clone https://github.com/bvolpato/ivygrep.git
cd ivygrep

# Fast, offline-capable hash build
cargo build --locked --no-default-features

# Focused local validation
./test.sh --quick
```

Default build includes local neural search and may download pinned model assets
on first use. Hash-only builds need no model download.

Useful environment variables:

- `IVYGREP_HOME`: isolate indexes and configuration during testing.
- `IVYGREP_NO_AUTOSPAWN=1`: prevent daemon auto-start.
- `CARGO_BUILD_JOBS`: cap local build concurrency.
- `RUST_TEST_THREADS`: cap test concurrency.

## Project map

Read [ARCHITECTURE.md](ARCHITECTURE.md) before changing storage, indexing,
search, worktrees, daemon IPC, context packs, or MCP contracts.

| Area | Primary files | Strong validation |
|---|---|---|
| Indexing and storage | `src/indexer.rs`, `src/workspace.rs`, `src/merkle.rs` | incremental CRUD, worktree, benchmark tests |
| Search and relevance | `src/search.rs`, `src/reranker.rs`, `src/embedding.rs` | relevance fixture and public benchmark harnesses |
| Context graph | `src/context.rs`, `src/context_graph.rs` | CLI, incremental, worktree, MCP tests |
| CLI and daemon | `src/cli.rs`, `src/daemon.rs`, `src/protocol.rs` | CLI snapshots, IPC, recovery tests |
| MCP and agents | `src/mcp.rs`, `src/agent.rs` | MCP unit/E2E and agent setup tests |
| Web UI | `src/web.rs`, `web/` | Vitest, TypeScript, web server tests |
| Documentation | `README.md`, `docs/`, agent and architecture guides | documentation contract tests and local browser check |

## Change workflow

1. Fork repository and branch from current `main`.
2. Add smallest useful test before or with behavior change.
3. Match nearby error handling, naming, and abstraction patterns.
4. Run focused tests while iterating.
5. Run full required validation before opening pull request.
6. Update user-facing docs and changelog for observable behavior changes.
7. Open focused pull request using repository template.

Keep commits reviewable. Use conventional commit subjects such as
`fix(search): preserve filtered candidate recall`.

## Validation

Before every pull request:

```bash
./test.sh --quick
```

Before requesting merge for Rust, scripts, workflows, or cross-cutting changes:

```bash
./test.sh
./bench.sh
```

Web changes:

```bash
pnpm -C web install --frozen-lockfile
pnpm -C web check
pnpm -C web build
git diff --exit-code -- web/dist
```

Docs or website changes:

```bash
python3 -m unittest discover -s tests -p 'test_*.py' -v
python3 -m http.server 8765 --directory docs
```

Open `http://127.0.0.1:8765/` and inspect desktop/mobile layout, console errors,
and changed links. Stop server afterward.

Release and platform acceptance procedures live in [AGENTS_TESTING.md](AGENTS_TESTING.md).

## Tests

- Bug fixes need regression test that fails without fix.
- Features need behavior-level test at public boundary.
- Storage changes need fresh, incremental, deletion, migration, and worktree coverage.
- MCP changes need schema, structured-content, error, and stdio-session coverage.
- Platform fixes need guarded test that runs on affected target in CI.
- Avoid tests that duplicate implementation or depend on timing, network, user home, or test order.

Temporary fixtures must use isolated directories and `IVYGREP_HOME`.

## Performance and relevance changes

Claims need comparable before/after evidence:

- same hardware, corpus, build profile, model, query set, warmup, and concurrency
- multiple runs when host noise can affect conclusion
- latency distribution, memory, and index-size impact when relevant
- relevance metrics and per-task regressions for search changes
- held-out/public datasets instead of repository-specific ranking rules
- discarded alternatives recorded when they explain final design

Keep only changes with useful measured tradeoffs. Do not encode benchmark names,
expected paths, or corpus-specific aliases in production ranking code.

## Code style

- Run `cargo fmt`.
- Fix every Clippy warning.
- Prefer `Result` and `?` over `unwrap()` in library code. `unwrap()` is fine in tests.
- Explain non-obvious reasons, not obvious operations.
- Preserve offline hash search and local-only code handling.
- Avoid new dependencies when standard library or existing crate is sufficient.

## Adding a Tree-sitter language

1. Add grammar crate to `Cargo.toml`.
2. Register parser and language detection in `src/chunking.rs`.
3. Add representative parser and retrieval tests.
4. Add context-graph import handling when language has local imports.
5. Update supported-language counts and lists in README and website.

## Review and release

Maintainers may request smaller scope, stronger tests, platform proof, or neutral
benchmark evidence. Maintainers own version bumps, tags, release notes, and
published artifacts.

Security vulnerabilities follow [SECURITY.md](SECURITY.md). Community behavior
follows [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Project decisions follow
[GOVERNANCE.md](GOVERNANCE.md).

By contributing, you agree contributions are licensed under [MIT License](LICENSE).
