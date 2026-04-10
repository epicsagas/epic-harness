## epic-harness

This project uses epic-harness for AI quality automation.

### Commands
- /spec — define requirements
- /go — build with TDD (sequential execution)
- /check — sequential review + audit + tests
- /ship — PR creation + CI
- /evolve — skill evolution status

### Auto-behaviors
- Before coding: read `.harness/memory/` for project context
- After each task: verify build+tests pass
- On security-related code: run secure skill checklist
- On large files (>200 lines): run simplify skill

### Hook Events
- **BeforeAgent** → `epic-harness resume` (restore session + load skills)
- **AfterAgent** → `epic-harness reflect` (evolve skills + save metrics)
- **AfterModel** → `epic-harness observe` (record tool scores, async)
- **BeforeModel** → `epic-harness guard` (scan for dangerous shell patterns)

### Notes
- Gemini CLI does not support parallel agent spawning — all tasks run sequentially
- Guard runs at BeforeModel level (no PreToolUse equivalent in Gemini CLI)
- Evolved skills stored in `.harness/evolved/`, backed up in `.harness/evolved_backup/`
