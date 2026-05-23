# epic-harness

**CRITICAL**: Run `HARNESS_DIR=$(epic-harness path)` first. Never use `.harness/` in the project directory.

## Commands

| Command | Purpose |
|---------|---------|
| `/evolve` | Inspect or trigger skill evolution |
| `/team` | Generate project-specific agent team |
| `/orbit` | Autonomous spec→ship pipeline |

## Auto Skills

These skills activate automatically based on context signals:

| Skill | Purpose |
|-------|---------|
| `spec` | Define requirements before coding |
| `go` | Build with auto-plan + TDD |
| `check` | Review + security audit + tests |
| `ship` | Create PR, verify CI, merge |
| `tdd` | New feature or bug fix — Red → Green → Refactor |
| `debug` | Test failure, runtime error, unexpected behavior |
| `secure` | Auth, DB, API, or secrets code touched |
| `verify` | Before marking done or shipping |
| `document` | Public API/function/module added or changed |
| `perf` | Loops, DB queries, rendering, or batch ops |
| `simplify` | File >200 lines, high complexity, or duplication |
| `council` | Architecture decisions with significant trade-offs |
| `commit` | Conventional Commits generation |
| `context` | Session restoration from snapshots |
| `discover` | Problem discovery — 5 Whys, JTBD, Socratic |
| `orchestrate` | Multi-agent orchestration status and control |
| `agent-introspection` | Failure recovery on 3+ consecutive errors |
| `reflect` | Human-triggered: "Am I using AI as a thought amplifier?" 5-dimension evidence-based self-assessment consuming hook-produced data |

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
| Session end | `epic-harness reflect` | Analyze failures, seed evolved skills, update metrics, ingest to memory |

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

Chains `spec → go → check → ship` skills in one session.

**Two modes:**
- **Interactive**: user describes the problem, then spec/go/check/ship skills fire automatically
- **Council auto-spec**: 4-voice council generates spec; user approves or rejects

After spec approved, runs autonomously. On FAIL: auto-fix and re-check, max 3 cycles. Pauses for human input if all 3 fail.
State tracked in `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json`.
