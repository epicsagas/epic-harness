---
name: orbit
description: "Complete orbit — autonomous spec through ship. Choose interactive or council mode, then hands-off until PR."
---

# /orbit — Complete Orbit

**CRITICAL**: Run `HARNESS_DIR=$(epic path)` first. NEVER use `.harness/` in the project directory.

Autonomous pipeline: spec → go → check → ship in one shot.

## Step 0: Preflight

Initialize `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json` with phase tracking.

## Step 1: Mode Selection

Ask the user:
> **1. Interactive discover** — You run `/discover` → `/spec`, then say "orbit go"
> **2. Council auto-spec** — 4-voice council generates spec, you approve

## Step 2A: Interactive Mode

Wait for user to finish `/discover` → `/spec`. On "orbit go": load approved spec, proceed.

## Step 2B: Council Auto-Spec

1. Launch 4 Cursor sub-agents (Composer sessions, available in Cursor 1.7+):
   - **Architect**: maintainability, extensibility, architecture fit
   - **Skeptic**: simpler alternatives, hidden costs, YAGNI
   - **Pragmatist**: timeline, user impact, MVP scope
   - **Critic**: edge cases, failure modes, security concerns
   - Each gets ONLY the request + codebase context (anti-anchoring)
2. If Cursor sub-agents unavailable, run voices sequentially in this session
3. Synthesize → generate spec → user approves

## Step 3: Build (Go)

1. Load spec, create `feature/{goal_slug}` branch
2. Plan tasks from Requirements
3. Execute with Cursor sub-agents (or sequential fallback): TDD + debug + verify
4. Handle DONE/CONCERNS/NEEDS_CONTEXT/BLOCKED states
5. Integrate: full test suite, verify ACs

## Step 4: Check

1. Scope: `git diff --stat` + classify changed files
2. Launch 3 Cursor sub-agents (or sequential):
   - **Reviewer**: code quality, logic, style, spec coverage
   - **Auditor**: security (OWASP) + performance (N+1, leaks)
   - **Test runner**: full suite, AC verification, coverage
3. + scope-specific checks (API contract, Frontend a11y, DB migration, Infra config)
4. Synthesize Check Report, **preserve in pipeline state**

## Step 5: Verdict

- **PASS** → Ship
- **WARN** → auto-proceed with notes
- **FAIL** → fix cycle (max 3), then pause for user

## Step 6: Ship

1. Gate: verify PASS check report
2. Isolated test (worktree): build + test + lint
3. Git hygiene → `gh pr create` with spec + check report → CI watch

## Step 7: Report

Consolidated phase summary: spec, branch, PR, check retries.

"One orbit complete. Run `/evolve` to analyze observations."

## Red Flags
- No user mode selection
- No spec approval
- Skipping isolated test
- Shipping with security FAIL
- Lost check report between phases
