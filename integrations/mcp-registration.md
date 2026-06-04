# harness-mem CLI Commands

The unified memory store is accessed via `epic mem` CLI commands built directly into
the `epic-harness` binary. All commands output JSON.

## Usage

```bash
# Add a memory node
epic mem add --title "Decision: use Redis" --type decision --importance 0.9 --body "Context..."

# Smart contextual recall
epic mem recall "auth refactor" --project myapp --limit 5

# Full-text search
epic mem search "database migration" --limit 10

# List/filter nodes
epic mem list --type decision --project myapp --limit 5

# Project-scoped recall (use at session start)
epic mem context --project myapp --limit 5

# Graph traversal from a node
epic mem related NODE_ID --depth 2
```

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `HARNESS_ROOT` | `~/.harness` | Memory store root directory |

## Available commands (6)

| Command | Description |
|---------|-------------|
| `epic mem add` | Add a new memory node (decisions, patterns, errors, etc.) |
| `epic mem list` | Query nodes by tag / type / project filter |
| `epic mem search` | Full-text keyword search across all nodes |
| `epic mem related` | BFS traversal of the knowledge graph to find related nodes |
| `epic mem context` | Load project context at session start |
| `epic mem recall` | Smart contextual recall with hint + graph neighbors |

## Testing

```bash
# Verify CLI is working
epic mem list --limit 1

# Add a test node
epic mem add --title "test node" --type concept --body "test body"

# Search for it
epic mem search "test node"
```
