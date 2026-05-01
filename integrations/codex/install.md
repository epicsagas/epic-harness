---
title: "epic-harness — Codex CLI Integration Install Guide"
---

# epic-harness Codex CLI Integration

**CRITICAL**: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

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

### 2. Prompts

Copy the prompt definitions so Codex can invoke them:

```bash
# Global
cp -r prompts/ ~/.codex/prompts/

# Project-local
cp -r prompts/ .codex/prompts/
```

### 3. Skills

> **Coming soon** — skill definitions will be added in a future release.

### 4. Agents

> **Coming soon** — agent definitions will be added in a future release.

## Hook Event Mapping

| Codex Event | epic-harness Subcommand | Purpose |
|-------------|------------------------|---------|
| SessionStart | `resume` | Restore session context, load evolved skills |
| PreToolUse (Bash) | `guard` | Block dangerous commands |
| PostToolUse (Bash) | `observe` | Record Bash tool results for evolution loop |
| PostToolUse (Edit) | `polish` | Auto-format + typecheck after edits |
| PostToolUse (Write) | `polish` | Auto-format + typecheck after file writes |
| Stop | `reflect` | Analyze session, evolve skills |

> Note: Codex has no PreCompact event — `snapshot` is not wired. Session state is preserved via `resume`/`reflect`.

## Verify Installation

Start a new Codex session. You should see output from the `resume` hook:

```
[harness] Session resumed — loaded N evolved skills, M memory entries
```

If you see `[harness] epic-harness not found`, ensure the binary is in PATH.

## Data Location

All per-project harness data lives in `$HARNESS_DIR/` at your project root:

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
