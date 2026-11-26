# ivygrep user guide

This guide explains the rules behind `ig`. For a first run, start with the [README](../README.md). For internals, read the [architecture](architecture.md).

- [Install, upgrade, and remove](#install-upgrade-and-remove)
- [Indexing](#indexing)
- [Search](#search)
- [Context packs](#context-packs)
- [Agents and MCP](#agents-and-mcp)
- [Notes](#notes)
- [Web UI](#web-ui)
- [Data and configuration](#data-and-configuration)
- [Troubleshooting](#troubleshooting)

## Install, upgrade, and remove

### Builds

The shell installer selects a Metal build on Apple Silicon. It selects CUDA only on Linux x86_64 when an NVIDIA GPU with compute capability 8.0 or newer is visible and the CUDA 13 runtime is available. Other supported hosts use the portable build.

Homebrew installs the Metal build on Apple Silicon and portable builds on Intel macOS and Linux.

CUDA and Metal accelerate optional transformer profiles. The default model runs on CPU in every build. Run `ig hardware` to check compatibility and to see the matching reinstall command.

Release archives include checksums, SBOMs, and provenance.

### Build from source

On macOS or Linux:

```bash
git clone https://github.com/bvolpato/ivygrep.git && cd ivygrep
./build.sh
mkdir -p ~/.local/bin
install -m 0755 target/release/ig ~/.local/bin/ig
```

`./build.sh --hash-only` and `cargo build --locked --no-default-features` build without the neural feature and without model downloads.

### Upgrade

Use the channel that you installed from: `brew upgrade ivygrep`, `winget upgrade --id BrunoVolpato.ivygrep --exact`, or run the installer again.

WinGet updates are submitted separately and can lag GitHub releases. Use the Windows release installer when you need the latest published version.

The next command restarts a daemon from an older build. An index format change rebuilds existing indexes once, on first use.

### Remove

Run `brew uninstall ivygrep` or `winget uninstall --id BrunoVolpato.ivygrep --exact`. If you used an installer, delete `ig` from the installer directory. The directory is `~/.local/bin`, or `%LOCALAPPDATA%\ivygrep\bin` on Windows, unless `IVYGREP_INSTALL_DIR` was set.

Then stop any running `ig --daemon` process and delete the [data directory](#data-and-configuration).

## Indexing

The first query indexes the current repository. The daemon then watches for changes and updates the index incrementally. `ig --add PATH` indexes and registers a directory without a search.

Fresh indexing publishes keyword results before vector enhancement. To wait for vectors, pass `--wait-for-enhancement` to `ig --add`.

### Missing files

Since v1.3.1, search immediately excludes a file that the safe file reader confirms is missing, even before the watcher reconciles. Reconciliation then removes the entries that the workspace owns. Permission, unavailable, and symlink failures are not treated as deletion.

### Worktrees

Linked Git worktrees share a base index. Each worktree stores only changed chunks and tombstones. An overlay tombstone hides a base-backed file that was deleted on that branch, and it does not change what other worktrees can search. A deleted file that only the worktree had needs no tombstone. Its overlay entries are removed.

### Broad directories

Since v1.3.1, a CLI search or context request, `ig --interactive`, or `ig --add` asks before it builds an index for your home directory, a directory that contains it, or the filesystem root. Confirm with `y` or `yes`. Without an interactive terminal, the command stops unless you pass `--yes`.

A healthy directory that is already indexed does not ask on a plain `ig --add`. `ig --add --force` rebuilds it, and `--skip-gitignore` can add ignored files, so those cases ask too.

The same confirmation applies when `--all --skip-gitignore` expands a broad index, or when a linked worktree needs to build or expand one for its main checkout.

### Background work

On macOS laptops, background neural enhancement pauses on battery power. `ig --status` shows `Paused: Battery Power`. Set `IVYGREP_ENHANCE_ON_BATTERY=1` to keep it running. The lightweight hash tier keeps computing on battery, so semantic results stay available.

At most two hash workers and two neural workers run at a time (`IVYGREP_ENHANCE_MAX_WORKERS`), regardless of how many workspaces change. The other jobs wait in the daemon. The workspace that was searched or edited last goes first.

### Languages

ivygrep supports 45 language and file types. Twenty-four use Tree-sitter AST chunking: Rust, Python, Go, JavaScript, TypeScript, Java, C, C++, C#, Kotlin, Scala, PHP, Ruby, Swift, Elixir, Zig, Bash, Haskell, OCaml, Lua, Dart, Objective-C, Perl, and Starlark.

## Search

```bash
ig "where is authentication handled?"       # hybrid semantic and keyword
ig --literal "handleAuth"                    # exact indexed lookup
ig --symbol calculate_tax                    # definitions
ig --refs calculate_tax                      # references and calls
ig --callers calculate_tax                   # chunks that call the symbol
ig "database migrations" src/api/            # path scope
ig --all "retry policy"                      # all indexed projects
```

`-n` limits result files. `-C` sets context lines. `--type` filters by language. `--include` and `--exclude` take path globs. `--lexical-only` skips vectors. `--hash` uses lightweight local embeddings, which start faster and need no model download, with lower semantic quality. `--json` prints machine-readable output.

### Literal search

Literal search ranks files by matching lines. A bounded search shows up to three previews per file and reports omitted matches. Use `--no-limit` to return every match. Use `--file-name-only` to return paths without building previews.

### Boolean operators

Standalone uppercase `AND`, `OR`, and `NOT` are Boolean operators. For example, `ig "settings NOT render"` excludes results that contain `render`. A short malformed expression, such as a trailing `OR`, returns an error.

If an invalid Boolean query spans multiple lines or has at least 13 words, ivygrep searches it as plain text. The results include a warning. The CLI writes it to stderr. MCP and the Web UI return it in `warnings`. Valid Boolean expressions keep their Boolean meaning, including in long prompts.

To search for an operator word as text, use lowercase letters or put the word in backticks or quotes.

### Pasted code, prose, and errors

A multi-line query that reads as pasted source ranks the code that contains the snippet above one-line definition signatures that share a few of its identifiers.

Multi-paragraph prompts, pasted issue text, and questions with a blank line rank as prose. Signature matches are scored like body text.

For pasted error output, such as a traceback, a `Caused by:` chain, or a `panicked at` line, keyword, path, hash-vector, and reranking signals ignore runtime values such as paths outside the workspace, ids, and timestamps. The static message text is matched against the code that raises it. Neural query vectors still embed the original text.

### Warnings

A partial workspace failure returns warnings with valid hits. A complete failure is an error.

## Context packs

```bash
ig context "fix refresh-token races" --since main --budget 8000
printf 'panic at src/auth.rs:42' | ig context -
```

The command uses your task, file paths in pasted text, and staged or uncommitted changes. The pack can include definitions, callers, references, dependencies, dependents, tests, configuration, and documentation. It trims the rendered Markdown to the requested estimated token budget. It does not generate an answer or modify files.

With `--since`, the command resolves the selected ref and finds its Git merge base with `HEAD`. The pack includes changes committed on the current branch since that base, plus staged, unstaged, and untracked files. The selected ref does not act as the diff base itself.

Use `--since main`, `--since HEAD~3`, or `--since '@{upstream}'` inside a Git worktree. For a directory without Git, omit `--since`.

Context packs can include Git history for deleted files, labeled `git_deleted`. Search excludes files that it confirms are missing from the workspace.

Abridged output of `ig context` on the ripgrep 16.0.0 source:

```text
# ivygrep context
Task: expand a tilde in the global core.excludesFile path
Budget: 980 / 1000 estimated tokens (truncated)
Anchors: core.excludesFile, excludesFile
Coverage: 2 files | 2 primary | 0 definitions | 1 dependencies | 1 dependents | 0 callers | 0 references | 1 tests | 0 config | 0 docs
Candidates: 15 retrieved | 3 selected
## Evidence
### 1. crates/ignore/src/gitignore.rs:986-1014 [primary, test]
Why: rank 1 for primary retrieval; rank 2 for anchor test retrieval.
Signals: lexical, neural, semantic.
```

## Agents and MCP

Agents call `ig_search` for discovery. Set `output=context_pack` and `budget_tokens=8000` when the task needs implementation context. Pass the absolute path to the active repository or worktree.

Automatic setup detects the client, preserves existing configuration, writes the absolute `ig` path, verifies the MCP handshake, and runs a search. Restart an open client after installation. `ig agent doctor` checks the connection.

The [README](../README.md#connect-a-coding-agent) lists the setup commands, and the [MCP page](https://bvolpato.github.io/ivygrep/integrations/mcp.html) shows requests with the main fields. `tools/list` returns the complete schema.

## Notes

```bash
ig --add ~/notes --wait-for-enhancement
ig -n 20 "what did we decide about cache invalidation?" ~/notes
```

Queries use local semantic and keyword search by default. Public [MemoryQuest results](https://bvolpato.github.io/ivygrep/benchmarks/public-memory-retrieval.html) for v1.2.7 show 74.9% recall@20 at 87.63 ms warm p95. The benchmark measures retrieval only. Answer accuracy is outside its scope.

## Web UI

```bash
ig --web
ig --web "auth flow" .
ig --web --host 0.0.0.0 --port 4747
```

`ig --web` binds to loopback by default. Its Web API requires a session token, including on loopback. The printed URL contains that token. Keep terminal output and logs that capture it private.

The browser opens through an owner-only redirect file in the app home, which keeps the token out of process arguments. To skip the browser, set `IVYGREP_NO_BROWSER=1`.

Web searches can select only registered workspaces. To make a new directory available, run `ig --add PATH` from the CLI.

Non-loopback listeners use plain HTTP. Use a trusted network, Tailscale, or an encrypted tunnel. Never expose the listener directly to the internet. To open the UI from another device, replace only `127.0.0.1` in the printed URL and keep the complete `token=...` query string. Restart the daemon to invalidate access.

Indexed file contents can appear in snippets. This includes dotfiles that your ignore rules permit.

## Data and configuration

Indexes and daemon state live in `~/.local/share/ivygrep` on every OS (under `%USERPROFILE%` on Windows). `IVYGREP_HOME` overrides that path. Otherwise a non-empty `XDG_DATA_HOME` selects `$XDG_DATA_HOME/ivygrep`.

Model assets use the Hugging Face cache (`HF_HOME`, default `~/.cache/huggingface`), which other tools can share. Neural profiles download pinned model assets on first use unless the cache already has them.

There is no configuration file. Use CLI flags and the [environment variables](architecture.md#environment-variables).

```bash
ig --status          # tracked workspaces, index health, vector coverage, disk usage
ig --rm ~/notes      # remove a saved index, defaults to the current directory
ig --gc              # remove indexes whose directory has been gone past the grace period
```

The daemon also removes the index of a directory that stays missing for seven days (`IVYGREP_INDEX_GC_GRACE_SECS`), and the overlay of a worktree removed with `git worktree remove` after ten minutes.

The implementation limits cached results to 64 MiB and cached neural queries to 4 MiB. Parsed context input has a shared 32 MiB cache. Idle search contexts share a 256 MiB estimated budget. Each index run has a 32 MiB estimated budget for queued payloads, and a large file can exceed it when it runs alone. These budgets do not cap total process memory. See [memory budgets and diagnostics](architecture.md#memory-budgets-and-performance-diagnostics).

## Troubleshooting

```bash
ig --doctor          # diagnose index, daemon, watcher, and model health
ig --doctor --fix    # repair a broken or stale index
ig --add . --force   # rebuild the current workspace index from scratch
ig hardware          # inspect detected hardware and the matching build
```

- **The Web UI returns 401.** Open the URL that `ig --web` prints. A bare `http://127.0.0.1:4747/` returns 401 until the browser has the session cookie that URL sets.
- **The browser cannot load its redirect file.** Snap-packaged browsers on Ubuntu cannot read files under hidden directories. Open the printed URL instead.
- **WSL opens nothing useful.** A Windows browser can load the redirect file only through an opener that translates Linux paths, such as `wslview`. Otherwise open the printed URL.
