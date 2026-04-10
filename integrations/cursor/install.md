# epic-harness Cursor Integration — Install Guide

## Requirements

- **Cursor 1.7 or later** — hooks (`PreToolUse`, `PostToolUse`, `SessionEnd`) and custom slash commands require Cursor 1.7+
- **epic-harness binary** in `PATH`

---

## 1. Install the epic-harness Binary

**Homebrew (macOS/Linux):**
```bash
brew install epicsagas/tap/epic-harness
```

**Cargo (from source):**
```bash
cargo install epic-harness
```

Verify the install:
```bash
epic-harness --version
```

---

## 2. Install Hooks

Hooks tell Cursor to run epic-harness automatically on tool events.

**Project-level** (affects only this project):
```bash
cp integrations/cursor/hooks.json .cursor/hooks.json
```

**Global** (affects all Cursor projects):
```bash
cp integrations/cursor/hooks.json ~/.cursor/hooks.json
```

The hooks wire up:
- `PreToolUse` on Bash → `epic-harness guard` (blocks dangerous commands)
- `PostToolUse` on Edit/Write → `epic-harness polish` (auto-format + type-check)
- `PostToolUse` on `*` → `epic-harness observe` (async observation recording)
- `SessionEnd` → `epic-harness reflect` (evolution loop)

---

## 3. Install Rules

Rules provide always-on context that loads harness state at session start and applies quality skills automatically.

```bash
mkdir -p .cursor/rules
cp integrations/cursor/rules/harness-context.mdc .cursor/rules/
cp integrations/cursor/rules/harness-skills.mdc .cursor/rules/
```

These rules replace the `session-start` hook (which Cursor does not expose) by injecting harness context into every session automatically.

---

## 4. Install Slash Commands

Custom slash commands expose the six epic-harness commands inside Cursor's Composer.

```bash
mkdir -p .cursor/commands
cp integrations/cursor/commands/*.md .cursor/commands/
```

After installation, the following commands are available in Cursor Composer:
- `/spec` — Define what to build
- `/go` — Build it (TDD + sub-agents)
- `/check` — Verify everything (review + audit + tests)
- `/ship` — Create PR and watch CI
- `/evolve` — Manage skill evolution
- `/team` — Design a project-specific agent team

---

## 5. Install Agents (Optional)

Sub-agent definitions for use with Cursor's sub-agent system or manual invocation.

```bash
mkdir -p .cursor/agents
cp integrations/cursor/agents/*.md .cursor/agents/
```

These agents are referenced by `/go` and `/check` commands when launching sub-tasks.

---

## 6. Verify Installation

```bash
ls .cursor/hooks.json .cursor/rules/ .cursor/commands/ .cursor/agents/
```

Start a new Cursor session. The Composer should load harness context from `.harness/memory/` and report any evolved skills from `.harness/evolved/`.

---

## File Layout After Install

```
.cursor/
├── hooks.json          # Hook event → epic-harness subcommand mapping
├── rules/
│   ├── harness-context.mdc   # Session start context + auto-behaviors
│   └── harness-skills.mdc    # Condensed TDD, secure, verify, simplify, perf rules
├── commands/
│   ├── spec.md
│   ├── go.md
│   ├── check.md
│   ├── ship.md
│   ├── evolve.md
│   └── team.md
└── agents/
    ├── builder.md
    ├── reviewer.md
    ├── auditor.md
    └── planner.md
```

---

## Troubleshooting

**"epic-harness not found" in hook output**
The hooks degrade gracefully — they print a warning and continue. Install the binary and ensure it is in your shell's `PATH`. Restart Cursor after installing.

**Hooks not firing**
Confirm Cursor version is 1.7 or later. Check `Cursor > Settings > Hooks` to verify hooks are enabled for the project.

**Rules not loading**
Confirm `.mdc` files are in `.cursor/rules/` (not a subdirectory). Restart Cursor to pick up new rule files.

**Commands not appearing**
Confirm `.md` files are in `.cursor/commands/`. In Composer, type `/` to see available custom commands.
