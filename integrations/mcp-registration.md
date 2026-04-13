# harness-mem MCP Server

The unified memory store is exposed as a native MCP server built directly into
the `epic-harness` binary. No Node.js or external runtime is required.

Transport: JSON-RPC 2.0 over stdio (MCP protocol version `2024-11-05`).

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

## Standalone registration for Claude Code

```bash
# Register in ~/.claude/settings.json
epic-harness mem mcp-install

# Preview without writing
epic-harness mem mcp-install --dry-run
```

The resulting entry in `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "harness-mem": {
      "command": "/path/to/epic-harness",
      "args": ["mem", "mcp"]
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
      "command": "epic-harness",
      "args": ["mem", "mcp"]
    }
  }
}
```

### Gemini CLI — `~/.gemini/settings.json`

```json
{
  "mcpServers": {
    "harness-mem": {
      "command": "epic-harness",
      "args": ["mem", "mcp"]
    }
  }
}
```

### OpenCode — `~/.config/opencode/config.json`

```json
{
  "mcpServers": {
    "harness-mem": {
      "command": "epic-harness",
      "args": ["mem", "mcp"]
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
# Start MCP server manually (reads JSON-RPC from stdin)
epic-harness mem mcp

# Initialize handshake
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"test","version":"1.0"}}}' \
  | epic-harness mem mcp

# List available tools
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  | epic-harness mem mcp
```
