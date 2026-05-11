# epic-harness

**CRITICAL**: Run `HARNESS_DIR=$(epic-harness path)` first. Never use `.harness/` in the project directory.

## Commands

| Command | Purpose |
|---------|---------|
| `/discover` | Explore and define the problem before specifying a solution |
| `/spec` | Define requirements before coding |
| `/go` | Build with auto-plan + TDD |
| `/check` | Review + security audit + tests |
| `/ship` | Create PR, verify CI, merge |
| `/evolve` | Inspect or trigger skill evolution |
| `/team` | Generate project-specific agent team |
| `/orbit` | Autonomous spec→ship pipeline |

## Session Start

At the beginning of every session:
- Read `$HARNESS_DIR/memory/` for project-specific rules and patterns
- If `$HARNESS_DIR/sessions/*.json` exists, read the latest file for previous session context
- Report any evolved skills found in `$HARNESS_DIR/evolved/`
- Run `epic-harness resume` if binary is available

## Auto-Behaviors

| Signal | Action |
|--------|--------|
| Auth, session, token, password, permissions code | Run `secure` skill checklist |
| Database, ORM, SQL, migration code | Run `secure` + `perf` checklist |
| File exceeds 200 lines | Run `simplify` skill |
| Test failure encountered | Run `debug` skill — diagnose root cause before retrying |
| Before marking any task complete | Run `verify` skill (build + test + lint) |
| New feature or bug fix | Apply `tdd` skill — write test first |
| Before coding: read project memory | `$HARNESS_DIR/memory/` |

## Hook Events

| Hook | Command | Effect |
|------|---------|--------|
| Session start | `epic-harness resume` | Restore session + load evolved skills |
| Pre tool use | `epic-harness guard` | Block dangerous shell patterns |
| Post tool use | `epic-harness observe` | Record tool scores (async) |
| Post edit | `epic-harness polish` | Auto-format + typecheck |
| Pre compact | `epic-harness snapshot` | Save session state |
| Session end | `epic-harness reflect` | Evolve skills + save metrics |

## Harness Memory

Always consult `$HARNESS_DIR/memory/` before starting work. These files contain:
- Project-specific coding patterns and conventions
- Known pitfalls and anti-patterns for this codebase
- Cross-session learnings from the evolution engine

## Evolved Skills

If `$HARNESS_DIR/evolved/` contains skill directories, treat them as supplementary guidance.
Static skills (tdd, secure, verify, simplify, perf, debug) always take priority over evolved skills.

## Commit — Conventional Commits

Format: `type(scope): description` — lowercase type, optional scope, imperative mood, ≤72 chars.
Breaking changes: append `!` before `:`. Valid types: `feat` `fix` `refactor` `docs` `test` `build` `chore` `ci` `style` `perf`.

Process: `git status` + `git diff HEAD` + `git log --oneline -5` → determine type → stage specific files → commit automatically.
Anti-patterns: vague messages, wrong type, staging unrelated files, using `--no-verify`.

## Orbit — Autonomous Pipeline

Chains `/spec → /go → /check → /ship` in one session.

**Two modes:**
- **Interactive**: user runs `/discover` → `/spec`, then triggers orbit
- **Council auto-spec**: 4-voice council generates spec; user approves or rejects

After spec approved, runs autonomously. On FAIL: auto-fix and re-check, max 3 cycles. Pauses for human input if all 3 fail.
State tracked in `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json`.
