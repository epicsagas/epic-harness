---
name: orbit
description: "Complete orbit — autonomous spec through ship. Choose interactive or council mode, then hands-off until PR."
---

# /orbit — Complete Orbit

**CRITICAL**: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

You are entering **Orbit** mode — the full autonomous pipeline from spec to PR in one shot.

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

## Step 0: Preflight

Initialize pipeline state at `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json`:
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
  "deadline": "{ISO-8601, now + 30 minutes}",
  "started_at": "{ISO-8601}",
  "updated_at": "{ISO-8601}",
  "phase_history": []
}
```

## Step 1: Mode Selection

Ask the user:
> **1. Interactive discover** — You run `/discover` and `/spec` yourself, then say "orbit go".
> **2. Council auto-spec** — 4-voice council analyzes your request, generates a spec, you approve.

Wait for choice. Record in pipeline state.

## Step 2A: Interactive Mode

Tell user to run `/discover` → `/spec`, then say "orbit go". STOP and wait.
On resume: load latest `SPEC-*.md` with `status: approved`. Proceed to Step 3.

## Step 2B: Council Auto-Spec

1. Gather the user's request (ask if not stated)
2. Launch 4 parallel sub-agents (Architect, Skeptic, Pragmatist, Critic) — each receives ONLY the request + codebase context, NOT the full conversation (anti-anchoring)
3. Synthesize: list agreement/disagreement, produce recommended approach
4. Generate spec at `$HARNESS_DIR/specs/SPEC-{timestamp}.md` with `status: pending`
5. Present to user: **Approve / Modify / Reject**
6. On approve: set `status: approved`, record via `mem_add` (type=decision, importance=0.9). Proceed.

## Step 3: Build (Go)

1. Load spec, extract `goal_slug`
2. **Git preflight**: verify clean working tree and not on detached HEAD:
   ```bash
   [ -z "$(git status --porcelain)" ] || (echo "ERROR: Dirty working tree or untracked files. Commit or stash first." && exit 1)
   git symbolic-ref -q HEAD || (echo "ERROR: Detached HEAD. Checkout a branch first." && exit 1)
   ```
3. **Worktree isolation**: Create an isolated git worktree:
   ```bash
   git worktree add .claude/worktrees/orbit-{goal_slug} -b orbit-{goal_slug} origin/{default-branch}
   cd .claude/worktrees/orbit-{goal_slug}
   ```
   - Record `worktree_name` and `original_cwd` in pipeline state
4. Plan tasks from Requirements (R1, R2...)
5. Execute with sub-agents — TDD, debug on failure, verify before done
6. Handle states: DONE / DONE_WITH_CONCERNS / NEEDS_CONTEXT / BLOCKED
7. Integrate: full test suite, verify ACs

## Step 4: Check

1. Gather scope via `git diff --stat`
2. Classify changed files (API, Frontend, DB, Backend, Tests, Infra)
3. Launch parallel sub-agents: Reviewer, Auditor, Test runner (+ scope-specific)
4. Synthesize Check Report: Quality/Security/Performance PASS/WARN/FAIL + Spec Coverage
5. **PRESERVE check report** in pipeline state `check_report` field

## Step 5: Verdict

- **All PASS + all AC verified** → proceed to Ship
- **WARN** → log, auto-proceed
- **FAIL** → increment `check_fail_count`:
  - `< 3`: plan fixes from action items, execute, return to Step 4
  - `≥ 3`: **PAUSE** — ask user "continue or abort?"

## Step 6: Ship

1. **Gate**: verify PASS check report exists
2. **Integration verification** — run directly in worktree:
   - Clean build artifacts first: `cargo clean` / `npm run clean` / equivalent
   - Full build from scratch · complete test suite · linter + formatter
   - Fail → STOP. Do NOT create PR.
3. **Git hygiene**: conventional commits, rebase, squash fixups
4. **Create PR** via `gh pr create` with spec + check report in body
5. **CI watch** via `gh pr checks --watch`, auto-fix failures
6. **Exit worktree**: Return to original directory and keep the worktree

## Step 7: Report

```
## Orbit Complete
- Pipeline: PIPELINE-{id}
- Mode: {interactive|council}
- Spec: SPEC-{timestamp} ({goal_slug})
- Branch: orbit-{goal_slug}
- Worktree: orbit-{goal_slug} (preserved for PR)
- PR: {URL}
- Check retries: {count}

### Phase Summary
| Phase | Status | Retries |
|-------|--------|---------|
| Spec | approved | 0 |
| Go | complete | 0 |
| Check | PASS | {count} |
| Ship | complete | 0 |
```

## Red Flags
- Starting without user mode selection
- Proceeding without spec approval
- Continuing after 3 check failures without user consent
- Skipping isolated integration test
- Shipping with FAIL in security checks
- Losing check report between phases
- Creating branch with dirty working tree
- Losing worktree reference between phases
