---
name: ship
description: "Ship phase. Runs isolated integration test in a fresh worktree, creates a PR with full spec + audit report in the body, watches CI, and auto-fixes failures."
---

# Ship — Ship It

**CRITICAL**: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

## Process

### Step 0: Prerequisites

Load the spec for PR content:
```bash
ls -t $HARNESS_DIR/specs/SPEC-*.md | head -1
```

**Gate: audit must have passed.** If no audit report exists, invoke the **audit** skill before continuing.

### Step 1: Pre-ship Verification

**1a. Isolated Integration Test**
Launch an agent with `isolation: "worktree"` to verify in a clean environment:
- Run full build from scratch (`cargo build --release` / `npm run build` / etc.)
- Run complete test suite
- Run linter and formatter checks
- Verify no uncommitted artifacts

**Gate:** If isolated test fails → STOP. **"Fix with `/go`, then re-run `/audit` before shipping."**

### Step 2: Git Hygiene

- Ensure all changes are committed (Conventional Commits)
- Rebase on latest base branch if needed
- Squash fixup commits if appropriate

### Step 3: Create PR

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

## Audit Report
<paste full Audit Report>

## Test Plan
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual verification done
EOF
)"
```

Register the PR in the merge-outcome ledger (orbit pipelines are picked up
automatically; standalone ships need this one-liner):

```bash
epic-harness evolve record-pr <PR_URL> [--goal <goal_slug>] [--branch <branch>]
```

### Step 4: CI Verification

Inspect the exact PR head and the base repository from its URL before watching:

```bash
gh pr view <PR_NUMBER> --repo <BASE_OWNER/BASE_REPO> --json headRefOid,statusCheckRollup,url
gh api "repos/<BASE_OWNER>/<BASE_REPO>/actions/runs?head_sha=<HEAD_SHA>&per_page=100"
gh api "repos/<BASE_OWNER>/<BASE_REPO>/commits/<HEAD_SHA>/check-suites?per_page=100"
```

Follow pagination as needed and match evidence to that head SHA. An empty check
rollup does not prove that CI is absent or passing.

- Runs/suites with `action_required`: report **PENDING — maintainer approval
  required**, with head SHA and run links. Skip the impossible check watch;
  do not mark ready to merge or treat approval as a source failure.
- Same-head runs/suites without check contexts: inspect actual runs with
  `gh run view <RUN_ID> --repo <BASE_OWNER/BASE_REPO> --json headSha,status,conclusion,url,jobs`.
  Poll queued/in-progress runs within a declared deadline and report **PENDING**
  if unfinished. Preserve completed failures and infrastructure errors; do not
  watch an empty check set. Accept a completed success only after verifying its
  current head, required workflow and successful required jobs.
- Executable current-head check contexts: run
  `gh pr checks <PR_NUMBER> --repo <BASE_OWNER/BASE_REPO> --watch`.
  Diagnose and fix actual source failures, retrying up to 2 times. Distinguish
  infrastructure startup errors from executed compiler/test failures.
- Successful queries with no contexts, runs or suites: report **no checks
  observed**, skip the empty watch, and allow at most one bounded re-probe for
  delayed registration. Claim **N/A — no applicable CI** only after verifying
  that no applicable provider is configured.
- API/authentication/permission/network errors: report **UNKNOWN** and the error;
  do not turn failed queries into empty results.

Re-read the PR head before accepting CI results and discard stale evidence.
Record the classification, head SHA and links in the report. Only verified
passing current-head CI is **PASS**; pending/unknown CI is not ready to merge.
When called from Orbit, return this outcome without blocking indefinitely so
Orbit can run its unconditional reflection; green-only additions remain gated.

### Step 5: Report

```
## Ship Report
- Spec: SPEC-{timestamp} ({goal_slug})
- PR: <URL>
- CI: [PASS/FAIL/PENDING/UNKNOWN/N/A]
- Ready to merge: [YES/NO]
- Action needed: <if any>
```

**If inside `/orbit`**: Return control to orbit — it will run evolve automatically.

**If standalone**: Suggest **"Run `/evolve` to analyze this session."**

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|-------------------|
| "CI will catch it" | CI doesn't catch everything | Run isolated test locally first |
| "The PR description doesn't matter" | It's the permanent record of why | Include spec + audit report |
| "I'll merge without CI" | CI is a safety net | Wait for CI, fix failures |

## Evidence Required

- [ ] Isolated test passed in clean worktree
- [ ] PR created with spec + audit report in body
- [ ] Current-head CI evidence checked and reported as PASS, FAIL, PENDING, UNKNOWN or verified N/A
- [ ] All Conventional Commits applied

## Red Flags

- Shipping without a PASS audit report
- PR description that says "various fixes" or "updates"
- Force-pushing to main
- Merging with failing CI
- PR body missing the Audit Report section
