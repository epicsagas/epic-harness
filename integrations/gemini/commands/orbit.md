---
description: "Complete orbit — autonomous spec through ship in one shot"
---

# /orbit — Complete Orbit

CRITICAL: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

Autonomous pipeline: spec → go → check → ship. All phases run sequentially (Gemini does not support parallel agents).

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

**Mode selection** — ask the user:
1. **Interactive**: User runs `/discover` → `/spec` manually, then says "orbit go"
2. **Council auto-spec**: 4-voice analysis generates spec, user approves

**Council mode** (if chosen):
- Ask each voice sequentially: Architect → Skeptic → Pragmatist → Critic
- Each gets ONLY the request + codebase context (anti-anchoring)
- Synthesize → generate spec → user approves/rejects

**After spec approved:**

## Step 0: Preflight

Create pipeline state at `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json`:
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

## Step 3: Go Phase — Build

1. Load spec, extract `goal_slug`
2. **Git preflight**: verify clean working tree and not on detached HEAD
3. **Worktree isolation**: Create an isolated git worktree so other sessions can freely switch branches:
   ```bash
   git worktree add .claude/worktrees/orbit-{goal_slug} -b orbit-{goal_slug} origin/{default-branch}
   cd .claude/worktrees/orbit-{goal_slug}
   ```
   - Record `worktree_name` as `orbit-{goal_slug}` and `original_cwd` in pipeline state
   - All subsequent phases (go/check/ship) execute inside the worktree
   - State files remain accessible: `$HARNESS_DIR` resolves to the same project directory (same git remote → same project slug)
4. Plan tasks from Requirements, execute one at a time (TDD: red → green → refactor)
5. Run full test suite after all tasks
6. Check: review code quality, security, performance, test coverage, spec coverage
7. On FAIL: fix and re-check, max 3 retries. After 3, pause for user decision
8. On PASS: proceed to Ship

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
   Record `worktree_name` status in pipeline state.

**Pipeline state**: `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json` — update after each phase.

**Report**: Consolidated phase summary with spec, branch, PR URL, check retries.

"One orbit complete. Run `/evolve` to analyze observations."

## Red Flags
- Starting without user mode selection
- Proceeding without spec approval
- Continuing after 3 check failures without user consent
- Skipping isolated integration test before PR
- Shipping with FAIL on any security check item
- Losing `check_report` between phases
- Creating branch with dirty working tree
- Losing worktree reference between phases (`worktree_name` missing from pipeline state)
- Forgetting to exit worktree before orbit complete (leaves session in wrong directory)
- Entering worktree without saving `original_cwd` (can't find state files after)
