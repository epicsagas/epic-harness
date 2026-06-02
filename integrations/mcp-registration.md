# harness-mem CLI Commands

The unified memory store is accessed via `epic-harness mem` CLI commands built directly into
the `epic-harness` binary. All commands output JSON.

## Usage

```bash
# Add a memory node
epic-harness mem add --title "Decision: use Redis" --type decision --importance 0.9 --body "Context..."

# Smart contextual recall
epic-harness mem recall "auth refactor" --project myapp --limit 5

# Full-text search
epic-harness mem search "database migration" --limit 10

# List/filter nodes
epic-harness mem list --type decision --project myapp --limit 5

# Project-scoped recall (use at session start)
epic-harness mem context --project myapp --limit 5

# Graph traversal from a node
epic-harness mem related NODE_ID --depth 2
```

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `HARNESS_ROOT` | `~/.harness` | Memory store root directory |

## Available commands (6)

| Command | Description |
|---------|-------------|
| `epic-harness mem add` | Add a new memory node (decisions, patterns, errors, etc.) |
| `epic-harness mem list` | Query nodes by tag / type / project filter |
| `epic-harness mem search` | Full-text keyword search across all nodes |
| `epic-harness mem related` | BFS traversal of the knowledge graph to find related nodes |
| `epic-harness mem context` | Load project context at session start |
| `epic-harness mem recall` | Smart contextual recall with hint + graph neighbors |

## Testing

```bash
# Verify CLI is working
epic-harness mem list --limit 1

# Add a test node
epic-harness mem add --title "test node" --type concept --body "test body"

# Search for it
epic-harness mem search "test node"
```
