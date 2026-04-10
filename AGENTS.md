---
description: "epic-harness agent configuration for Google Antigravity"
---

# epic-harness Agent Configuration

## Identity

You are an expert software engineer using epic-harness for quality automation.
This project uses epic-harness workflows (/spec, /go, /check, /ship, /evolve, /team).

## Session Startup

At the beginning of each session:
1. **You (the agent) run via Bash tool**: `epic-harness resume`
   This prints session context, previous metrics, and evolved skills to stderr.
2. Read `.harness/memory/` for project-specific rules
3. Check `.harness/evolved/` for evolved skills and apply them

## Auto-behaviors (Ring 2 — Skills)

Trigger these behaviors automatically based on context:

### When writing or modifying code
- Follow TDD: write failing test → implement → verify passes
- One test per behavior. Show Red → Green → Refactor output.

### When touching auth, database, API, or secrets code
- Apply Security checklist (OWASP Top 10, injection, auth bypass)
- No exceptions for "small" changes — one missed injection is a breach

### Before marking any task complete
- Run: build + test + lint (all must pass)
- Show actual command output — "I ran it" without output is not verified

### When a file exceeds 200 lines
- Apply simplify skill: extract functions, reduce nesting
- Verify tests still pass after refactor

### On test failures
- Diagnose root cause before retrying
- Check error message carefully, don't guess
- Show the failure message before proposing a fix

## Workflows (Slash Commands)

Use these commands for structured execution:

| Command | Purpose |
|---------|---------|
| `/spec` | Define what to build — clarify requirements, produce spec |
| `/go`   | Build it — plan, execute with TDD, verify |
| `/check`| Verify everything — quality + security + tests |
| `/ship` | Create PR, verify CI, merge |
| `/evolve` | Run `epic-harness reflect` + show metrics |
| `/team` | Design project-specific agent team |

## Multi-Agent Execution (Manager View)

Antigravity's Manager view enables native parallel agent execution.
Use it for independent tasks in /go and /check workflows.

## Session End

Before ending the session, run: `epic-harness reflect` in terminal.
This records observations and evolves skills for the next session.

## Forbidden Commands (Guard Rules)

Never execute the following without explicit user confirmation:
- `git push --force` or `git push --force-with-lease` — can overwrite shared history
- `DROP TABLE`, `DROP DATABASE`, `TRUNCATE`, or `DELETE FROM` without a WHERE clause
- `rm -rf /`, `rm -rf ~`, `rm -rf $HOME`, `rm -rf .` — recursive deletion of broad paths
- `kubectl delete` — destroys running workloads
- Any command targeting a production environment, prod database, or live infrastructure

Note: These are soft guidelines enforced by the LLM. For hard enforcement, use the
`guard` subcommand (`epic-harness guard`) which blocks patterns at the binary level.

## Quality Standards

- No deprecated APIs — use latest stable versions
- All mutations have tests — no untested code paths
- Security-sensitive code reviewed before /ship
- PR descriptions explain what and why, not just what
