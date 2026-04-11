# epic-harness Cursor Integration — Install Guide

## Requirements

- **Cursor 1.7 or later** — hooks (`preToolUse`, `postToolUse`, `sessionEnd`) and custom slash commands require Cursor 1.7+
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

The `hooks.json` file follows [Cursor’s hooks schema](https://cursor.com/docs/hooks): **`version` must be `1`**, hook names are **camelCase** (`preToolUse`, not `PreToolUse`). The shell tool is matched with **`Shell`** (not `Bash`). If any of these are wrong, Cursor may ignore the file.

The hooks wire up:
- `preToolUse` on `Shell` → `epic-harness guard` (blocks dangerous commands)
- `postToolUse` on `Edit` / `Write` → `epic-harness polish` (auto-format + type-check)
- `postToolUse` on `*` → `epic-harness observe` (async observation recording)
- `sessionEnd` → `epic-harness reflect` (evolution loop)

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

Each command file uses YAML frontmatter with **`name`** (kebab-case identifier, same as the file stem) and **`description`** (one-line summary). [Cursor’s command format](https://cursor.com/docs/reference/plugins) expects both; if `name` is missing, the slash menu may show only `(user)` instead of your description.

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

Sub-agent definitions for use with Cursor's sub-agent system or manual invocation. Each file includes `model: inherit` so the subagent follows the parent composer model ([Cursor subagents](https://cursor.com/docs/subagents)).

```bash
mkdir -p .cursor/agents
cp integrations/cursor/agents/*.md .cursor/agents/
```

These agents are referenced by `/go` and `/check` commands when launching sub-tasks.

---

## Install command behavior

`epic-harness install cursor` (and `--local`) **writes or updates** every embedded integration file (`hooks.json`, `rules/`, `commands/`, `agents/`) so it matches the binary. Files that already match are left unchanged. This fixes the case where an older or empty `hooks.json` existed and would previously have been skipped entirely.

## 6. Verify Installation

```bash
ls .cursor/hooks.json .cursor/rules/ .cursor/commands/ .cursor/agents/
```

Start a new Cursor session. The Composer should load harness context from `$(epic-harness path)/memory/` and report any evolved skills from `$(epic-harness path)/evolved/`.

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
Confirm Cursor version is 1.7 or later. Check `Cursor > Settings > Hooks` to verify hooks are enabled for the project. Open `hooks.json` and confirm it has `"version": 1` and camelCase keys (`preToolUse`, `postToolUse`, `sessionEnd`). Re-run `epic-harness install cursor` after upgrading epic-harness so the file matches the embedded copy.

**Agents not listed**
Custom agents live under `.cursor/agents/` (project) or `~/.cursor/agents/` (global). They appear when delegating to subagents in supported flows; there is not always a separate “installed agents” list in the UI.

**Rules not loading**
Confirm `.mdc` files are in `.cursor/rules/` (not a subdirectory). Restart Cursor to pick up new rule files.

**Commands not appearing**
Confirm `.md` files are in `.cursor/commands/`. In Composer, type `/` to see available custom commands.
