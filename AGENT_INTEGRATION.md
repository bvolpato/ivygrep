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

### Claude Code

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

### Codex

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

### Cursor

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

### Gemini CLI

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

### OpenCode

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
| `limit` | Maximum number of returned files |
| `context` | Context lines around the focused match |
| `file_name_only` | Return paths without snippets |

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
