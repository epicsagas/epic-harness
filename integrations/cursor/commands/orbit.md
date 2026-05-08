---
name: orbit
description: "Complete orbit — autonomous spec through ship. Choose interactive or council mode, then hands-off until PR."
---

# /orbit — Complete Orbit

**CRITICAL**: Run `HARNESS_DIR=$(epic path)` first. NEVER use `.harness/` in the project directory.

Autonomous pipeline: spec → go → check → ship in one shot.

## Phase Recovery Protocol

At the start of **every response** during an active orbit:

1. Run `ls $HARNESS_DIR/orbit/PIPELINE-*.json 2>/dev/null`
2. Find the file with `"status": "running"`
3. Read it. Verify `phase` matches where you left off
4. **If `phase` is ahead of where you think you are, trust the file** — you may have compacted
5. **Conflict resolution (crash-mid-update)**: If `phase_history` contains an entry for the current `phase` with a completed timestamp, treat that phase as done and advance to the next phase — `phase_history` wins over the `phase` field when they disagree.
6. Resume from the resolved phase. Do NOT re-ask mode selection, re-run spec, or re-discover
7. **Worktree recovery**: If `worktree_name` is set in pipeline state:
   - Check if worktree still exists: `git worktree list | grep "{worktree_name}"`
   - If exists: `cd` into the worktree path to continue work
   - If not found: worktree was cleaned up externally — abort orbit with warning, set `"status": "aborted"`

If no file with `"status": "running"` exists, orbit was not started or has completed. Do not invent one.

**Crash recovery**: If `updated_at` is older than 45 minutes and the pipeline is in `status: running`, assume a crash occurred. Read the state, determine the last completed phase from `phase_history` (rule 5 above applies), and resume from there. Report the recovery to the user.

> **Worktree crash safety**: Pipeline state (`PIPELINE-*.json`), spec files, and check reports live in `$HARNESS_DIR` (shared across worktrees — same git remote → same project slug). They survive worktree loss. If the worktree was cleaned up externally during a crash, abort the orbit and warn the user.

## Step 0: Preflight

Initialize `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json`:
```json
{
  "id": "{timestamp}",
  "mode": null,
  "phase": "mode_select",
  "status": "running",
  "spec_file": null,
  "goal_slug": null,
  "branch": null,
  "worktree_name": null,
  "original_cwd": null,
  "check_fail_count": 0,
  "max_retries": 3,
  "check_report": null,
  "started_at": "{ISO}",
  "updated_at": "{ISO}",
  "phase_history": []
}
```

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

1. Load spec, extract `goal_slug`
2. **Git preflight**: verify clean working tree and not on detached HEAD:
   ```bash
   [ -z "$(git status --porcelain)" ] || (echo "ERROR: Dirty working tree or untracked files. Commit or stash first." && exit 1)
   git symbolic-ref -q HEAD || (echo "ERROR: Detached HEAD. Checkout a branch first." && exit 1)
   ```
3. **Worktree isolation**: Create an isolated git worktree so other sessions can freely switch branches:
   ```bash
   git worktree add .claude/worktrees/orbit-{goal_slug} -b orbit-{goal_slug} origin/{default-branch}
   cd .claude/worktrees/orbit-{goal_slug}
   ```
   - Record `worktree_name` as `orbit-{goal_slug}` and `original_cwd` in pipeline state
   - All subsequent phases (go/check/ship) execute inside the worktree
   - State files remain accessible: `$HARNESS_DIR` resolves to the same project directory (same git remote → same project slug)

   > **Why worktree?** Orbit pipelines can run for 30+ minutes. Without isolation, switching branches in another session corrupts the in-progress work — uncommitted changes, branch state, and file edits collide. Worktree isolation guarantees the orbit operates on its own copy of the repo.
4. Plan tasks from Requirements
5. Execute with Cursor sub-agents (or sequential fallback): TDD + debug + verify
6. Handle DONE/CONCERNS/NEEDS_CONTEXT/BLOCKED states
7. Integrate: full test suite, verify ACs

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
2. **Integration verification** — run directly in worktree (already isolated from main tree):
   - Clean build, full test suite, linter + formatter
   - Fail → STOP. Do NOT create PR.
3. Git hygiene → `gh pr create` with spec + check report → CI watch
4. **Exit worktree**: Return to original directory and keep the worktree (needed for PR head):
   ```bash
   cd {original_cwd}
   # Worktree preserved — branch is needed for PR
   ```
   Record worktree status in pipeline state.

## Step 7: Report

Consolidated phase summary: spec, branch, PR, check retries.

"One orbit complete. Run `/evolve` to analyze observations."

## Red Flags
- No user mode selection
- No spec approval
- Skipping isolated test
- Shipping with security FAIL
- Lost check report between phases
- Creating branch with dirty working tree
- Losing worktree reference between phases (`worktree_name` missing from pipeline state)
- Forgetting to exit worktree before orbit complete (leaves session in wrong directory)
- Entering worktree without saving `original_cwd` (can't find state files after)
