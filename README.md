<p align="center">
  <img src="assets/logo.png" alt="ivygrep logo" width="150" />
</p>

<p align="center">
  <strong>Search code and notes. Build context for coding tasks.</strong><br/>
  Files and queries stay on your machine. The default model downloads its pinned assets on first use.
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
Budget: 7642 / 8000 estimated tokens
Coverage: 7 files | 2 primary | 1 definitions | 1 dependencies | 0 dependents | 2 callers | 0 references | 1 tests | 0 config | 0 docs
Candidates: 31 retrieved | 14 selected
## Evidence
### 1. src/auth/refresh.rs:118-166 [primary, definition]
Why: task anchor; changed implementation.
Signals: lexical, symbol, git change.
```

Search returns ranked file paths, line numbers, and previews. Context packs add related code and explain why each item was selected.

The context command uses your task, file paths in pasted text, and staged or uncommitted changes.
With `--since`, it also uses commits since the selected revision.
The pack can include definitions, callers, dependencies, tests, configuration, and documentation.
It trims the rendered Markdown to the requested estimated token budget. It does not generate an answer or modify files.

Use `--since main`, `--since HEAD~3`, or `--since '@{upstream}'` inside a Git worktree.
For a directory without Git, omit `--since`.

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

Installers select a compatible archive, verify its SHA-256 checksum, and install `ig`.
Apple Silicon gets a Metal build. Compatible NVIDIA Linux hosts get a CUDA build.
CUDA requires CUDA 13 and compute capability 8.0 or newer. Other hosts use the portable build.

The default `potion-code-16m-v2` model runs on CPU in every build.
Metal and CUDA accelerate the optional transformer profiles: `general`, `code`, and `code-hq`.
Run `ig hardware` to check compatibility and see the matching reinstall command.

Build from source on macOS or Linux:

```bash
git clone https://github.com/bvolpato/ivygrep.git && cd ivygrep
./build.sh
mkdir -p ~/.local/bin
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

Standalone uppercase `AND`, `OR`, and `NOT` are Boolean operators.
For example, `ig "settings NOT render"` excludes results that contain `render`.
Short malformed expressions, such as a trailing `OR`, return an error.

If an invalid Boolean query spans multiple lines or has at least 13 words, ivygrep searches it as plain text.
The results include a warning. The CLI writes it to stderr. MCP and the Web UI return it in `warnings`.
Valid Boolean expressions keep their Boolean meaning, including in long prompts.
To search operator words as text, use lowercase letters or wrap the words in backticks or quotes.

Multi-line queries that read as pasted source rank the code that contains the snippet above one-line
definition signatures that share a few of its identifiers. Multi-paragraph prompts, pasted issue text,
and questions with a blank line rank as prose, with signature matches scored like body text. For pasted
error output, such as a traceback, a `Caused by:` chain, or a `panicked at` line, lexical, path,
hash-vector, and reranking signals ignore runtime values such as paths outside the workspace, ids, and
timestamps, and the static message text is matched against the code that raises it. Neural query vectors
still embed the original text.

On macOS laptops, background neural enhancement pauses on battery power (`ig --status`
shows `Paused: Battery Power`); set `IVYGREP_ENHANCE_ON_BATTERY=1` to keep it running.
The lightweight hash tier keeps computing on battery so semantic results stay available.
However many workspaces change at once, at most two hash and two neural enhancement
workers run at a time (`IVYGREP_ENHANCE_MAX_WORKERS`); the rest wait in the daemon, and the
workspace searched or edited last goes first.

## Search notes and memories

Index notes once. Watcher keeps them current, and queries use local semantic + lexical search by default:

```bash
ig --add ~/notes --wait-for-enhancement
ig -n 20 "what did we decide about cache invalidation?" ~/notes
```

Public [MemoryQuest results](https://bvolpato.github.io/ivygrep/benchmarks/public-memory-retrieval.html) (v1.2.7): 74.9% recall@20 at 87.63 ms warm p95. Benchmark measures retrieval only; answer accuracy is outside scope.

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
3. SQLite stores metadata and relationships. Tantivy stores lexical postings. USearch stores hash vectors and model vectors.
4. Query routing runs bounded exact, lexical, symbol, hash, and optional neural passes before fusion.
5. Context expands primary hits through code relationships and recent changes,
   then trims rendered evidence to requested token budget.

Fresh indexing publishes lexical results before vector enhancement. Worktrees
reuse base index and store only divergent chunks and tombstones. Partial
workspace failures return warnings with valid hits; complete failure errors.

ivygrep supports 45 language and file types. Twenty-four use Tree-sitter AST chunking:
Rust, Python, Go, JavaScript, TypeScript, Java, C, C++, C#, Kotlin, Scala, PHP,
Ruby, Swift, Elixir, Zig, Bash, Haskell, OCaml, Lua, Dart, Objective-C, Perl, and Starlark.

Read [architecture](docs/architecture.md) for storage, commit order, retrieval,
worktrees, protocols, security boundaries, and module ownership.

## System performance

The historical v1.2.7 benchmark used a deterministic synthetic corpus with one million chunks.
Across three hash-only trials, median warm CLI p95 was 6.19 ms. Controlled indexing reached 150,576 chunks/s.
The final index used 0.42 GiB. These results measure scale and footprint, not retrieval quality or coding-task accuracy.
They do not describe the current neural model under concurrent load.

The [resource and latency report](docs/benchmarks/resource-load.html) measures the released binary with both static model profiles.
It includes repeated enhancement measurements, forced-neural p99, eight MCP clients, RSS, CPU, and disk writes.
The model-screening report remains separate because it used one repetition.

[Historical scale measurements (v1.2.7)](https://bvolpato.github.io/ivygrep/benchmarks/public-million-current.json) · [Million-chunk methodology](https://bvolpato.github.io/ivygrep/benchmarks/public-million.html) · [Full benchmark dashboard](https://bvolpato.github.io/ivygrep/benchmarks/evidence-dashboard.html)

The source implementation limits cached results to 64 MiB and cached neural queries to 4 MiB.
Parsed context input has a shared 32 MiB cache. Content digests prevent reuse after a file changes.
Idle search contexts share a 256 MiB estimated budget. Each index run has a 32 MiB estimated budget for queued payloads.
A large file can exceed the indexing target when it runs alone. These budgets do not cap total process RSS.
See [memory budgets and diagnostics](docs/architecture.md#memory-budgets-and-performance-diagnostics) for scope and timing commands.

## Local and private

Runtime source, queries, embeddings, results, and indexes stay local. Neural
profiles download pinned model assets on first use unless cache is already
populated. Use `--hash`, `./build.sh --hash-only`, or
`cargo build --locked --no-default-features` to avoid model downloads.

`ig --web` binds to loopback by default. Its Web API requires a session token, including on loopback.
The printed URL contains that token. Keep terminal output and logs that capture it private.
The browser opens through an owner-only redirect file in the app home. This keeps the token out of process arguments.
To skip the browser, set `IVYGREP_NO_BROWSER=1`.

Non-loopback listeners use plain HTTP. Use a trusted network, Tailscale, or an encrypted tunnel.
Never expose the listener directly to the internet.
Indexed file contents can appear in snippets. This includes dotfiles that your ignore rules permit.

Report vulnerabilities through a [private security advisory](SECURITY.md). Release archives include checksums, SBOMs, and provenance.

## Data and configuration

Indexes and daemon state live in `~/.local/share/ivygrep` on every OS (under `%USERPROFILE%` on Windows).
`IVYGREP_HOME` overrides that path; otherwise a non-empty `XDG_DATA_HOME` selects `$XDG_DATA_HOME/ivygrep`.
Model assets use the Hugging Face cache (`HF_HOME`, default `~/.cache/huggingface`), which other tools may share.

There is no configuration file. Use CLI flags and the [environment variables](docs/architecture.md#environment-variables).

```bash
ig --status          # tracked workspaces, index health, vector coverage, disk usage
ig --rm ~/notes      # remove a saved index; defaults to current directory
ig --gc              # remove indexes whose directory has been gone past the grace period
```

The daemon also removes the index of a directory that stays missing for seven days (`IVYGREP_INDEX_GC_GRACE_SECS`), and the overlay of a worktree removed with `git worktree remove` after ten minutes.

## Troubleshooting, upgrade, and uninstall

```bash
ig --doctor          # diagnose index, daemon, watcher, and model health
ig --doctor --fix    # repair a broken or stale index
ig --add . --force   # rebuild current workspace index from scratch
ig hardware          # inspect detected hardware and matching build
```

Open the URL `ig --web` prints: a bare `http://127.0.0.1:4747/` returns 401 until
the browser has the session cookie that URL sets.
If `ig --web` opens a page that cannot load its redirect file, as snap-packaged
browsers on Ubuntu do for files under hidden directories, open the URL it prints.
Under WSL, a Windows browser can load the redirect file only through an opener
that translates Linux paths, such as `wslview`; otherwise open the printed URL.

Upgrade through the channel you installed from: `brew upgrade ivygrep`,
`winget upgrade --id BrunoVolpato.ivygrep --exact`, or rerun the installer. The
next command restarts a daemon from an older build; index format changes rebuild
existing indexes once on first use.

To uninstall, run `brew uninstall ivygrep` or `winget uninstall --id BrunoVolpato.ivygrep --exact`,
or delete `ig` from the installer directory (`~/.local/bin` or `%LOCALAPPDATA%\ivygrep\bin` unless
`IVYGREP_INSTALL_DIR` was set). Then stop any running `ig --daemon` process and delete the data directory.

## Contribute

```bash
./test.sh --quick
./test.sh
./bench.sh
```

Start with a [good first issue](https://github.com/bvolpato/ivygrep/labels/good%20first%20issue), read [CONTRIBUTING.md](CONTRIBUTING.md) and [architecture](docs/architecture.md),
or discuss an idea in [Discussions](https://github.com/bvolpato/ivygrep/discussions).

MIT licensed. Maintained by [Bruno Volpato](https://github.com/bvolpato).
