# epic-harness Cursor Integration — Install Guide

**CRITICAL**: Run `HARNESS_DIR=$(epic path)` first. NEVER use `.harness/` in the project directory.

## Requirements

- **Cursor 1.7 or later** — hooks (`preToolUse`, `postToolUse`, `sessionEnd`) and skills require Cursor 1.7+
- **epic binary** in `PATH`

---

## 1. Install the epic-harness Binary

**Homebrew (macOS/Linux):**
```bash
brew install epicsagas/tap/epic-harness
```

**Cargo (from source):**
```bash
cargo install epic
```

Verify the install:
```bash
epic --version
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
- `preToolUse` on `Shell` → `epic guard` (blocks dangerous commands)
- `postToolUse` on `Edit` / `Write` → `epic polish` (auto-format + type-check)
- `postToolUse` on `*` → `epic observe` (async observation recording)
- `sessionEnd` → `epic reflect` (evolution loop)

---

## 3. Install Rules

Rules provide always-on context that loads harness state at session start and applies quality skills automatically.

```bash
mkdir -p .cursor/rules
cp integrations/cursor/rules/harness-context.mdc .cursor/rules/
```

These rules replace the `session-start` hook by injecting harness context into every session automatically.

---

## 4. Install Skills

Skills provide both auto-triggered quality gates and user-invoked pipeline workflows.

```bash
mkdir -p .cursor/rules .cursor/skills
epic install cursor --local
```

This generates:
- `.cursor/skills/{name}/SKILL.md` — 21 individual skills (orbit, evolve, team, tdd, secure, etc.)
- `.cursor/rules/harness-skills.mdc` — Consolidated quality skills for auto-trigger

After installation, the following skills are available:
- **Pipeline skills** (user-invoked): orbit, evolve, team, discover, spec, go, audit, ship
- **Quality skills** (auto-triggered): tdd, secure, verify, simplify, perf, document, reflect, etc.

---

## 5. Install Agents (Future)

> **Note:** Agent knowledge is now embedded within skills (builder, reviewer, auditor, planner modes). No standalone agent files are needed. Use the built-in agent capabilities in Cursor's Composer.

---

## Install command behavior

`epic install cursor` (and `--local`) **writes or updates** every embedded integration file (`hooks.json`, `rules/`, `skills/`) so it matches the binary. Files that already match are left unchanged. Legacy commands (`commands/*.md`) are automatically cleaned up.

## 6. Verify Installation

```bash
ls .cursor/hooks.json .cursor/rules/ .cursor/skills/
```

Start a new Cursor session. The Composer should load harness context from `$HARNESS_DIR/memory/` and report any evolved skills from `$HARNESS_DIR/evolved/`.

---

## File Layout After Install

```
.cursor/
├── hooks.json          # Hook event → epic-harness subcommand mapping
├── rules/
│   ├── harness-context.mdc   # Session start context + auto-behaviors
│   └── harness-skills.mdc    # Consolidated quality skills (auto-trigger)
├── skills/
│   ├── orbit/SKILL.md
│   ├── evolve/SKILL.md
│   ├── team/SKILL.md
│   ├── tdd/SKILL.md
│   └── ... (21 skills total)
```

---

## Troubleshooting

**"epic not found" in hook output**
The hooks degrade gracefully — they print a warning and continue. Install the binary and ensure it is in your shell's `PATH`. Restart Cursor after installing.

**Hooks not firing**
Confirm Cursor version is 1.7 or later. Check `Cursor > Settings > Hooks` to verify hooks are enabled for the project. Open `hooks.json` and confirm it has `"version": 1` and camelCase keys (`preToolUse`, `postToolUse`, `sessionEnd`). Re-run `epic install cursor` after upgrading epic-harness so the file matches the embedded copy.

**Agents not listed**
Agents are not yet available for the Cursor integration. They will be added in a future release. Use the built-in agent capabilities in the meantime.

**Rules not loading**
Confirm `.mdc` files are in `.cursor/rules/` (not a subdirectory). Restart Cursor to pick up new rule files.

**Commands not appearing**
Commands were migrated to skills in v0.4.4. Run `epic install cursor --local` to clean up legacy command files and install skills instead.

---

## Memory Integration

epic-harness includes a unified memory store shared across all agents and tools.

**Session start — inject relevant context:**
```bash
epic mem context --project <slug>
```
This is surfaced automatically at session start via the `harness-context.mdc` rule.

**Manual add — record a decision or pattern:**
```bash
epic mem add --title "Chose Postgres over SQLite" --type decision --body "SQLite lacks concurrent writes needed for our workload."
```

**Supported `--type` values:** `decision`, `pattern`, `note`, `architecture`

**Web UI — browse and search all memory:**
```bash
epic mem serve
# → http://localhost:7700
```

**Auto-record via hook:** The `postToolUse` hook runs `epic mem-observe` after every Edit/Write tool call. If the tool output or assistant message contains decision keywords (`decision`, `architecture`, `pattern`, `chose`, `decided`, `approach`), the entry is recorded automatically.

**Shorthand via `harness` symlink** (if `hooks/bin/harness → epic-harness` exists):
```bash
harness mem add --title "..." --type decision --body "..."
harness mem context --project <slug>
harness mem serve
```
The symlink is created automatically by `epic install`. Run `epic install --check` to verify.
