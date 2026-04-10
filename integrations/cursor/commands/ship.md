---
name: ship
description: "Ship it — create PR, verify CI, merge. End-to-end delivery."
---

# /ship — Ship It

You are starting the **Ship** phase — from working code to merged PR.

## Process

### Step 1: Pre-ship verification
Run `/check` internally if not already done. All tests must pass.

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

## Check Report
<paste from /check if available>
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
- Action needed: <if any>
```

## Red Flags
- Shipping without running tests
- PR description that says "various fixes" or "updates"
- Force-pushing to main
- Merging with failing CI
