# harness-mem MCP Server

`hooks/scripts/mem-mcp.cjs` is a JSON-RPC 2.0 over stdio MCP server.
It exposes the unified memory store as native tools to any MCP-compatible agent
(Claude Code, Cursor, Gemini CLI, OpenCode, and others).

## Automatic registration (via `install`)

Running `epic-harness install <tool>` **automatically** injects
`mcpServers.harness-mem` into the tool's settings file.

| Tool | Settings file | Auto-registered |
|------|--------------|-----------------|
| gemini | `~/.gemini/settings.json` | ✓ |
| cursor | `~/.cursor/mcp.json` | ✓ |
| opencode | `~/.config/opencode/config.json` | ✓ |
| codex | n/a (no mcpServers concept) | — |
| cline | workspace-level only | — |
| aider | no MCP support | — |

The path to `mem-mcp.cjs` is resolved automatically:
1. `<bin-dir>/hooks/scripts/mem-mcp.cjs` (installed release)
2. `<repo-root>/hooks/scripts/mem-mcp.cjs` (dev build)
3. `~/.harness/bin/mem-mcp.cjs` (manual placement)

## Standalone registration for Claude Code

```bash
# Auto-detect path and register in ~/.claude/settings.json
epic-harness mem mcp-install

# Specify path explicitly
epic-harness mem mcp-install --path /path/to/mem-mcp.cjs

# Preview without writing
epic-harness mem mcp-install --dry-run
```

The resulting entry in `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "harness-mem": {
      "command": "node",
      "args": ["/path/to/hooks/scripts/mem-mcp.cjs"]
    }
  }
}
```

## Manual registration for other tools

### Cursor — `~/.cursor/mcp.json`

```json
{
  "mcpServers": {
    "harness-mem": {
      "command": "node",
      "args": ["/path/to/hooks/scripts/mem-mcp.cjs"]
    }
  }
}
```

### Gemini CLI — `~/.gemini/settings.json`

```json
{
  "mcpServers": {
    "harness-mem": {
      "command": "node",
      "args": ["/path/to/hooks/scripts/mem-mcp.cjs"]
    }
  }
}
```

### OpenCode — `~/.config/opencode/config.json`

```json
{
  "mcpServers": {
    "harness-mem": {
      "command": "node",
      "args": ["/path/to/hooks/scripts/mem-mcp.cjs"]
    }
  }
}
```

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `HARNESS_ROOT` | `~/.harness` | Memory store root directory |

## Available tools (5)

| Tool | Description |
|------|-------------|
| `mem_add` | Add a new memory node (decisions, patterns, errors, etc.) |
| `mem_query` | Query nodes by tag / type / project filter |
| `mem_search` | Full-text keyword search across all nodes |
| `mem_related` | BFS traversal of the knowledge graph to find related nodes |
| `mem_context` | Load project context at session start |

## Testing

```bash
# Unit tests
node hooks/scripts/mem-mcp.test.cjs

# Initialize
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"test","version":"1.0"}}}' \
  | node hooks/scripts/mem-mcp.cjs

# List available tools
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  | node hooks/scripts/mem-mcp.cjs
```
