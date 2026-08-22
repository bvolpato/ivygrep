---
name: use-ivygrep
description: Gather focused local repository context with ivygrep before implementing, debugging, reviewing, or explaining code. Use for coding tasks involving unfamiliar paths, branch changes, stack traces, architectural relationships, callers, dependents, tests, configuration, or documentation. Prefer one bounded context pack over broad filesystem scans.
---

# Use ivygrep

Start with one context request. Pass absolute active repository or worktree path.

When MCP tool `ig_search` is available, call it with:

- `query`: concrete task, failure, or question
- `path`: absolute repository or worktree path
- `output`: `context_pack`
- `budget_tokens`: `8000`
- `since`: base branch when current changes matter

CLI fallback:

```bash
ig context "<task>" --since main --budget 8000 /absolute/repository/path
```

On a large repository the first call may return `status: indexing` with
progress instead of results. That is not an error: the index keeps building in
the background. Wait `retry_after_secs`, then repeat the same call. Do not retry
immediately or fall back to broad filesystem scans.

Use returned selection reasons and relationships to choose files. Verify relevant
snippets against source before editing. Treat missing expected code as retrieval
evidence, not proof that code does not exist.

Use one focused search only when context pack leaves a concrete gap:

```bash
ig --literal "<identifier>" -n 10 /absolute/repository/path
```

MCP `ig_search` with `output: hits` returns at most `limit` files (default 10)
and `hits_per_file` hits per file (default 3). Check `truncated`,
`total_matches`, and `more_hits_in_file`; narrow `query` or `path`, or raise
`limit`, instead of repeating broad queries.

Do not repeat broad synonymous queries. Continue with normal code inspection,
implementation, and project validation after ivygrep identifies working set.
