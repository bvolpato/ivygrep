# Coding Agent Integration

ivygrep exposes local code search over Model Context Protocol (MCP). The server
uses stdio, uploads no source or query data, and limits each `ig_search` call to
one explicitly selected workspace.

## Before You Connect

Install `ig`, then verify that the environment launching your agent can resolve
it:

```bash
ig --version
ig --status
```

Desktop applications do not always inherit the `PATH` from an interactive
shell. If the MCP server fails with an executable-not-found error, replace
`"ig"` with an absolute path such as `/home/me/.local/bin/ig`,
`/opt/homebrew/bin/ig`, or `C:\\Users\\me\\AppData\\Local\\ivygrep\\bin\\ig.exe`.

## Client Configuration

### Automatic setup

Each installer preserves unrelated client settings and MCP servers, writes the
absolute path to the running `ig`, verifies MCP initialization and tool
discovery, then runs one real indexed search.

```bash
ig agent install claude
ig agent install codex
ig agent install cursor
ig agent doctor
```

Restart an open client after installation. `ig agent doctor` reports detected,
missing, malformed, and working configurations with a remediation command.

Use manual configuration below for project-scoped servers, custom environment
variables, Gemini CLI, or OpenCode.

### Manual setup

#### Claude Code

```bash
claude mcp add -s user ig -- ig --mcp
claude mcp get ig
```

The equivalent user configuration is:

```json
{
  "mcpServers": {
    "ig": {
      "type": "stdio",
      "command": "ig",
      "args": ["--mcp"]
    }
  }
}
```

#### Codex

```bash
codex mcp add ig -- ig --mcp
codex mcp get ig --json
```

Codex stores user configuration in `~/.codex/config.toml`. The CLI and IDE
extension share that file. A trusted repository can use
`.codex/config.toml` instead:

```toml
[mcp_servers.ig]
command = "ig"
args = ["--mcp"]
```

#### Cursor

Create `.cursor/mcp.json` for one repository or `~/.cursor/mcp.json` globally:

```json
{
  "mcpServers": {
    "ig": {
      "type": "stdio",
      "command": "ig",
      "args": ["--mcp"]
    }
  }
}
```

Refresh MCP servers in Cursor settings after changing the file.

#### Gemini CLI

```bash
gemini mcp add --scope user --transport stdio ig ig --mcp
gemini mcp list
```

The equivalent user configuration in `~/.gemini/settings.json` is:

```json
{
  "mcpServers": {
    "ig": {
      "command": "ig",
      "args": ["--mcp"]
    }
  }
}
```

Gemini disables user MCP servers in untrusted folders. Trust the project before
testing the connection.

#### OpenCode

Add this to `opencode.json`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "ig": {
      "type": "local",
      "command": ["ig", "--mcp"],
      "enabled": true
    }
  }
}
```

Then run `opencode mcp list`.

## Tools

### `ig_search`

Use `ig_search` for semantic, lexical, or exact code discovery.

Important arguments:

| Argument | Use |
|---|---|
| `query` | Required natural-language query, keywords, identifier, or regex |
| `path` | Absolute repository, worktree, subdirectory, or file path |
| `literal` | Exact identifier or text search backed by the index |
| `regex` | Regex search; prefer `literal` when regex syntax is unnecessary |
| `type` | Language name, extension, or alias such as `rust`, `rs`, or `python` |
| `include` / `exclude` | Comma-separated path globs |
| `limit` | Retrieval breadth and maximum result files; larger values search deeper and may improve recall |
| `context` | Lines before and after each focused match; changes snippet size, not ranking |
| `first_line_only` | Keep one preview line per hit without changing ranking |
| `file_name_only` | Return paths without snippets |

### Result Count and Context

`limit` and `context` are independent controls. `limit` chooses retrieval
breadth and caps ranked files; `context` changes source text per hit. Neither is
a relevance threshold:

| Control | What it changes | Ranking |
|---|---|---|
| `limit=N` | Searches a candidate pool sized for the request and returns at most `N` ranked files | The same relevance signals apply; a deeper pool can slightly change ranks |
| omitted `limit` | Uses normal bounded candidate budgets without an explicit final file cap | Normal ranking |
| `context=N` | Shows up to `N` source lines before and after each focused match | Unchanged |
| `first_line_only` / `file_name_only` | Reduces output text after retrieval | Unchanged |

- `limit=5` returns at most five result files. It is a cap, not a confidence
  threshold, so fewer files may be returned after relevance filtering. Exact
  modes can also group multiple matches into one file.
- The ranker always optimizes the same relevance signals. A larger limit
  improves recall opportunity by searching deeper, but progressively
  lower-ranked candidates can also enter the response.
- Results remain score-ordered. A smaller limit truncates the response to the
  highest-ranked files found for that request; a relevant file below the cutoff
  is not shown. `limit` is not a line, token, or confidence budget.
- If `limit` is omitted, normal candidate retrieval remains bounded but no
  explicit final result-file cap is applied. Agents should set it explicitly.
- Increasing `limit` searches a deeper lexical, literal, semantic, and symbol
  candidate pool. Deeper results are usually less confident, and the larger
  pool can slightly change the top ranks.
- `context=2` returns up to two lines before and two lines after the focused
  line, for at most five lines per hit when file boundaries allow.
- Increasing `context` does not alter retrieval scoring or ranking. It only
  returns more surrounding source for the selected hits.
- `first_line_only` and `file_name_only` are presentation controls. They do not
  change retrieval or ranking.

Recommended agent flow:

1. Start with `limit=5` to `10` and `context=2`.
2. Use `literal=true` for an exact identifier, and use `type`, `path`,
   `include`, or `exclude` to remove known noise.
3. Increase `context` when the right file is present but its snippet lacks
   enough evidence.
4. Increase `limit` when the result set lacks enough distinct implementations
   or supporting files. Narrow `path`, `type`, `include`, or `exclude` when the
   results are topically correct but too broad.

MCP `ig_search` has no total `max_tokens` parameter. Use CLI
`ig context "task" --budget TOKENS` for one task-aware, budgeted bundle.
Otherwise control MCP result count and per-hit context separately.

Fewer snippet tokens do not mean fewer result files, better relevance, or worse
relevance. They mean less source text was returned for the selected files.
Evaluate relevance from the ranking and task outcome, not payload size alone.

Do not use the internal fused score as a cross-query confidence threshold. It
orders one query's candidates, but it is not calibrated across queries or
repositories. Prefer rank, path/type filters, and whether the returned evidence
is sufficient for the task.

Always pass `path`. Relying on the MCP process working directory is less
portable across desktop clients, terminals, containers, and remote workspaces.
The path also provides the security boundary: MCP search intentionally does not
offer cross-workspace `--all` search.

The first search creates a hash index quickly, starts incremental filesystem
watching, and schedules neural enhancement in the background. Subsequent
searches reuse the index and reflect changed-file deltas. Cached neural model
artifacts continue to work offline.

### `ig_status`

Use `ig_status` to inspect indexed workspaces, watcher health, indexing state,
and background enhancement state. A workspace is usable as soon as it reports
`ready_to_query: true`; neural enhancement can still be running.

## Recommended Agent Instruction

Place this in the repository's persistent instruction file:

```text
Use the ivygrep MCP tools for code discovery before broad filesystem scans.
Pass the absolute current repository or worktree path to ig_search.
Use natural-language queries for concepts and literal=true for exact identifiers.
Use limit to choose retrieval breadth and context to choose source lines per hit.
Start with limit=5-10 and context=2. Increase context when a promising hit needs
more evidence; increase limit when you need more candidate files.
Use ig_status when indexing health is unclear.
```

Do not require ivygrep for every file read. It is most useful for locating
implementations, callers, tests, configuration, and related concepts before the
agent opens the small set of relevant files.

## Git Worktrees

Pass the active worktree root, not the main checkout:

```text
ig_search(query="where is request authentication enforced?", path="/repo-feature")
```

ivygrep keeps one reusable base index for the repository. Each worktree stores
only divergent SQLite, lexical, and vector chunks plus deletion tombstones and
Merkle metadata. It does not duplicate the full base index.

## Troubleshooting

- **Executable not found:** use the absolute `ig` or `ig.exe` path and restart
  the client after changing `PATH`.
- **First search is slower:** initial indexing is synchronous; neural
  enhancement continues in the background.
- **No network is allowed:** use the default cached model after its first
  download, or use hash search from the CLI with `ig --hash`.
- **Index health is unclear:** call `ig_status`, then run `ig --doctor --deep`
  in a terminal if repair details are needed.
- **Results are too broad:** pass a subdirectory or file as `path`, or set
  `type`, `include`, and `exclude`.
- **Exact name is missed:** retry with `literal=true`.

## Vendor References

- [Claude Code MCP](https://code.claude.com/docs/en/mcp)
- [Codex MCP](https://developers.openai.com/codex/mcp)
- [Cursor MCP](https://cursor.com/docs/mcp)
- [Gemini CLI MCP](https://geminicli.com/docs/tools/mcp-server/)
- [OpenCode MCP](https://opencode.ai/docs/mcp-servers/)
