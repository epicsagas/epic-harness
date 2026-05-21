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

### 2. Skills

Skills are seeded to `~/.codex/skills/` on `epic-harness install codex`.
Each skill is a directory with `SKILL.md`:

```
~/.codex/skills/
├── check/SKILL.md              # /check — verify everything
├── go/SKILL.md                 # /go — build it
├── ship/SKILL.md               # /ship — ship it
├── evolve/SKILL.md             # /evolve — evolve skills
├── spec/SKILL.md               # /spec — write spec
├── team/SKILL.md               # /team — manage team
├── discover/SKILL.md           # /discover — discover context
├── orbit/SKILL.md              # /orbit — autonomous pipeline
├── tdd/SKILL.md                # Auto-skill: TDD
├── secure/SKILL.md             # Auto-skill: security
├── verify/SKILL.md             # Auto-skill: verification
├── simplify/SKILL.md           # Auto-skill: simplification
├── perf/SKILL.md               # Auto-skill: performance
├── commit/SKILL.md             # Auto-skill: commit
├── document/SKILL.md           # Auto-skill: documentation
├── debug/SKILL.md              # Auto-skill: debugging
├── context/SKILL.md            # Auto-skill: context management
├── council/SKILL.md            # Auto-skill: multi-voice review
├── agent-introspection/SKILL.md # Auto-skill: failure recovery
├── reflect/SKILL.md            # Auto-skill: session reflection
├── discover/SKILL.md           # Auto-skill: codebase discovery
├── orchestrate/SKILL.md        # Auto-skill: task orchestration
(No standalone agent files — all agent knowledge absorbed into skills)
```

> **Note**: `prompts/` is deprecated in Codex. All commands and agents are seeded as skills.

### 3. Agents

Agents are seeded inside skills with `agents/openai.yaml` metadata (see above).
Codex also supports standalone TOML agents at `~/.codex/agents/`, but epic-harness
uses the skill-based approach for unified management.

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
