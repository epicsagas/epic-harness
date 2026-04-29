---
description: "Ship it — create PR, verify CI, merge. End-to-end delivery."
---

# /ship — Ship It

You are starting the **Ship** phase — from working code to merged PR.

## Process

### Step 1: Pre-ship verification

**1a. Isolated Integration Test**
Use git worktree to verify in a clean environment:
- Run full build from scratch (`cargo build --release` / `npm run build` / etc.)
- Run complete test suite
- Run linter and formatter checks
- Verify no uncommitted artifacts generated during build

This simulates CI conditions locally and catches issues before PR creation.

**1b. Code Review**
If isolated test passes, run `/check` on main working tree for final quality review.

**Gate:** If either 1a or 1b fails → STOP. Do not proceed to PR creation.

### Step 2: Git hygiene
- Ensure all changes are committed with meaningful messages
- Rebase on latest base branch if needed
- Squash fixup commits if appropriate

### Step 3: Create PR
```bash
gh pr create --title "<concise title>" --body "$(cat <<'EOF'
## Summary
<what and why, not how>

## Changes
<bullet list of key changes>

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
- PR: <URL>
- CI: [PASS/FAIL]
- Ready to merge: [YES/NO]
```

## Red Flags
- Shipping without running tests
- PR description that says "various fixes" or "updates"
- Force-pushing to main
- Merging with failing CI
