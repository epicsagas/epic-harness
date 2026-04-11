---
title: "epic-harness — Codex CLI Integration Install Guide"
---

# epic-harness Codex CLI Integration

## Prerequisites

Ensure the `epic-harness` binary is in your PATH:

```bash
which epic-harness
# Should print a path — if not, install from the project root first
```

## Installation

### 1. Hooks

Copy `hooks.json` to your Codex config directory or the project root:

```bash
# Global (applies to all projects)
cp hooks.json ~/.codex/hooks.json

# Project-local
cp hooks.json .codex/hooks.json
```

### 2. Commands

Copy the slash commands so Codex can invoke them:

```bash
# Global
cp -r commands/ ~/.codex/commands/

# Project-local
cp -r commands/ .codex/commands/
```

### 3. Skills

Copy the skill definitions:

```bash
# Global
cp -r skills/ ~/.codex/skills/

# Project-local
cp -r skills/ .codex/skills/
```

### 4. Agents

Copy the agent definitions:

```bash
# Global
cp -r agents/ ~/.codex/agents/

# Project-local
cp -r agents/ .codex/agents/
```

## Hook Event Mapping

| Codex Event | epic-harness Subcommand | Purpose |
|-------------|------------------------|---------|
| SessionStart | `resume` | Restore session context, load evolved skills |
| PreToolUse (Bash) | `guard` | Block dangerous commands |
| PostToolUse (Edit, Write) | `polish` | Auto-format + lint after edits |
| PostToolUse (*) | `observe` (async) | Record tool results for evolution loop |
| Stop | `reflect` | Analyze session, evolve skills |

> Note: Codex has no PreCompact event — `snapshot` is not wired. Session state is preserved via `resume`/`reflect`.

## Verify Installation

Start a new Codex session. You should see output from the `resume` hook:

```
[harness] Session resumed — loaded N evolved skills, M memory entries
```

If you see `[harness] epic-harness not found`, ensure the binary is in PATH.

## Data Location

All per-project harness data lives in `$(epic-harness path)/` at your project root:

```
.harness/
├── memory/       # Project patterns and rules
├── sessions/     # Session snapshots
├── obs/          # Tool usage observations (JSONL)
├── evolved/      # Auto-evolved skills
├── specs/        # /spec output
├── team/         # /team output
└── metrics.json  # Aggregate stats and score history
```
