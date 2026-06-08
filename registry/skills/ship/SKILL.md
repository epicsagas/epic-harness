---
name: ship
description: "Ship phase. Tiered verification (T0-T3) in fresh worktree, PR creation, CI monitoring, auto-fix on failure."
---

# Ship — Ship It

**CRITICAL**: Run `HARNESS_DIR=$(epic path)` first. NEVER use `.harness/` in the project directory.

## Process

### Step 0: Prerequisites

Load the spec for PR content:
```bash
ls -t $HARNESS_DIR/specs/SPEC-*.md | head -1
```

**Gate: audit must have passed.** If no audit report exists, invoke the **audit** skill before continuing.

### Step 1: Tiered Verification Ladder

Launch an agent with `isolation: "worktree"` to verify in a clean environment.
Each tier must pass before advancing. T1/T2 failures auto-retry up to 3 times.

#### T0 — Build (mandatory)

```bash
cargo build --release   # or: npm run build, go build ./...
```

**Gate:** Build must succeed. No retry — build failures require code fixes.
If T0 fails → STOP. **"Fix with `/go`, then re-run `/audit` before shipping."**

#### T1 — Tests (mandatory, auto-retry ≤3)

```bash
cargo test              # or: npm test, go test ./...
cargo clippy -- -D warnings   # or: eslint, golangci-lint
cargo fmt --check       # or: prettier --check
```

**Gate:** All tests pass, linter clean, formatter clean.
If T1 fails → diagnose, apply fix, retry from T0. After 3 failures → STOP and report.
**"T1 failed 3 times. Fix with `/go`, then re-run `/ship`."**

#### T2 — Audit AC Verification (mandatory, auto-retry ≤3)

Load the spec and verify every Acceptance Criterion is demonstrably met:
```bash
ls -t $HARNESS_DIR/specs/SPEC-*.md | head -1
```

For each AC, confirm one of:
- A test explicitly covers it (cite test name)
- The diff directly implements it (cite file + line)
- Manual verification was performed (describe what was checked)

**Gate:** All ACs verified. Any unverified AC is a failure.
If T2 fails → fix with `/go`, then re-run `/audit`. After 3 failures → STOP and report.

#### T3 — Security Assessment (optional)

Only runs when:
- `.harness/engagement.md` exists in the project, OR
- The diff touches auth, crypto, DB, or secrets code

Uses the **secure** skill checklist. Reports CRITICAL/HIGH findings only.
- CRITICAL findings → block ship
- HIGH findings → warn, ask user

**Gate:** No CRITICAL findings. HIGH findings require user acknowledgment.

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

## Verification Ladder
- T0 (Build): ✅
- T1 (Tests): ✅ (N retries)
- T2 (AC Verified): ✅
- T3 (Security): ✅ / N/A

## Audit Report
<paste full Audit Report>

## Test Plan
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual verification done
EOF
)"
```

### Step 4: CI Verification

```bash
gh pr checks <PR_NUMBER> --watch
```

If CI fails, diagnose and fix automatically. Retry up to 2 times.

### Step 5: Report

```
## Ship Report
- Spec: SPEC-{timestamp} ({goal_slug})
- PR: <URL>
- Verification: T0 ✅ T1 ✅ (N retries) T2 ✅ T3 ✅/N/A
- CI: [PASS/FAIL/N/A]
- Ready to merge: [YES/NO]
- Action needed: <if any>
```

**If inside `/orbit`**: Return control to orbit — it will run evolve automatically.

**If standalone**: Suggest **"Run `/evolve` to analyze this session."**

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|-------------------|
| "CI will catch it" | CI doesn't catch everything | Run tiered verification locally first |
| "The PR description doesn't matter" | It's the permanent record of why | Include spec + audit report + ladder results |
| "I'll merge without CI" | CI is a safety net | Wait for CI, fix failures |
| "T3 is overkill" | One CRITICAL vuln is a breach | Run T3 when security-scope is detected |
| "3 retries is too many" | Flaky tests exist; retry distinguishes flaky from broken | Track retry count — 3 consecutive = real failure |

## Evidence Required

- [ ] T0 passed: clean build in isolated worktree
- [ ] T1 passed: full test suite + linter + formatter
- [ ] T2 passed: every AC verified with evidence
- [ ] T3 passed or N/A (with justification)
- [ ] PR created with spec + audit report + ladder results in body
- [ ] CI status checked (PASS, FAIL with fix, or N/A)
- [ ] All Conventional Commits applied

## Red Flags

- Shipping without a PASS audit report
- Skipping T1 because "tests are slow"
- Marking T2 PASS without citing specific evidence per AC
- PR description that says "various fixes" or "updates"
- Force-pushing to main
- Merging with failing CI
- PR body missing the Verification Ladder section
