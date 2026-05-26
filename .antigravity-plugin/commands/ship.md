---
description: "Ship phase. Run after /check PASS. Runs isolated integration test in a fresh worktree, creates a PR with full spec + check report in the body, watches CI, and auto-fixes failures. Suggests /evolve on completion."
---

# /ship — Ship It

You are starting the **Ship** phase — from working code to merged PR.

## Process

### Step 0: Prerequisites

Run `HARNESS_DIR=$(epic-harness path)`.

Load the spec for PR content:
```bash
ls -t $HARNESS_DIR/specs/SPEC-*.md | head -1
```
Read the Goal, Requirements, and Acceptance Criteria — use them in the PR body.

**Gate: check must have passed.**
If the user has not run `/check` (or check report is absent from the conversation), run `/check` now before continuing. Do not proceed to PR creation without a PASS check report.

### Step 1: Pre-ship verification

**1a. Isolated Integration Test**
Launch an agent with `isolation: "worktree"` to verify in a clean environment:
- Run full build from scratch (`cargo build --release` / `npm run build` / etc.)
- Run complete test suite
- Run linter and formatter checks
- Verify no uncommitted artifacts generated during build

This simulates CI conditions locally and catches issues before PR creation.

**1b. Code Review (if check not already run)**
If isolated test passes and no check report exists, run `/check` on main working tree.

**Gate:** If either 1a or 1b fails → STOP. Tell the user what failed and instruct: "Fix with `/go`, then re-run `/check` before shipping."

### Step 2: Git hygiene
- Ensure all changes are committed with meaningful messages (Conventional Commits)
- Rebase on latest base branch if needed
- Squash fixup commits if appropriate

### Step 3: Create PR

Use the spec and check report to fill the PR body:

```bash
gh pr create --title "<goal from spec>" --body "$(cat <<'EOF'
## Summary
<Goal from spec — what and why, not how>

## Spec
- Spec ID: SPEC-{timestamp}
- Requirements: R1, R2, ...

## Changes
<bullet list of key changes>

## Acceptance Criteria Verified
- AC1: ✅
- AC2: ✅

## Check Report
<paste full Check Report here — Code Quality / Security / Performance / Tests / Spec Coverage>

## Test Plan
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual verification done
EOF
)"
```

### Step 4: CI verification
```bash
gh pr checks <PR_NUMBER> --watch
```
If CI fails, diagnose and fix. Do not ask the user to fix CI — handle it.

### Step 5: Report
```
## Ship Report
- Spec: SPEC-{timestamp} ({goal_slug})
- PR: <URL>
- CI: [PASS/FAIL]
- Ready to merge: [YES/NO]
- Action needed: <if any>
```

**If running inside `/orbit` pipeline** (a `PIPELINE-*.json` with `"status": "running"` exists):
- Do NOT stop here. Return control to orbit — it will run Step 7 (Evolve) automatically.

**If running standalone** (no active orbit pipeline):
- Suggest: **"One loop complete. Run `/evolve` to analyze this session's observations and improve skills for the next cycle."**

## Red Flags
- Shipping without a PASS check report
- PR description that says "various fixes" or "updates"
- Force-pushing to main
- Merging with failing CI
- PR body missing the Check Report section
