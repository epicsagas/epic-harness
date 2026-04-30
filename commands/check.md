---
description: "Verify everything — parallel code review + security audit + performance analysis"
---

# /check — Verify Everything

You are starting the **Check** phase — comprehensive verification using parallel agents.

## Process

### Step 0: Prerequisites

Run `HARNESS_DIR=$(epic-harness path)`.

Confirm go has run:
```bash
git symbolic-ref --short HEAD  # must NOT be main/master
```
If on the default branch with no feature work, warn the user: "No feature branch detected — did you run `/go` first?"

Load the spec to know what was supposed to be built:
```bash
ls -t $HARNESS_DIR/specs/SPEC-*.md | head -1
```
Read the Requirements and Acceptance Criteria sections — use them to validate scope.

### Step 1: Gather scope
Identify what changed against the base branch:
```bash
git diff --stat $(git merge-base HEAD main)  # or master / base branch
```

### Step 2: Launch 3 parallel agents

**Agent 1 — Reviewer** (use `agents/reviewer.md`):
- Code quality, logic correctness, style consistency
- Look for bugs, race conditions, edge cases
- Check test coverage for changed code
- Verify each spec Requirement is addressed in the diff

**Agent 2 — Auditor** (use `agents/auditor.md`):
- Security: injection, auth bypass, secret exposure, OWASP Top 10
- Performance: N+1 queries, memory leaks, unnecessary computation
- Refer to `references/security.md` and `references/performance.md`

**Agent 3 — Test runner**:
- Run the full test suite
- Verify each Acceptance Criterion (AC1, AC2, ...) is demonstrably met
- Report coverage delta
- Flag any flaky tests

Launch all three with `run_in_background: true`.

### Step 3: Synthesize

Combine findings into a single report:

```
## Check Report
- Spec: SPEC-{timestamp} ({goal_slug})
- Branch: {current branch}

### Code Quality: [PASS/WARN/FAIL]
### Security: [PASS/WARN/FAIL]
### Performance: [PASS/WARN/FAIL]
### Tests: [X/Y passing, Z% coverage]

### Spec Coverage
- R1: ✅/❌ addressed in diff
- R2: ✅/❌ addressed in diff
- AC1: ✅/❌ verified by test
- AC2: ✅/❌ verified by test

### Action Items
1. [blocker or warning]
```

### Step 4: Act

- **All PASS + all AC verified**: Tell the user: **"Check passed. Run `/ship` to create a PR."**
- **WARN**: Show warnings, ask user if they want to fix before shipping
- **FAIL or AC missing**: List each blocker with a one-line fix hint. Tell the user: **"Fix these issues with `/go`, then re-run `/check`."**

## Red Flags
- Skipping security review for "small changes"
- Approving code with failing tests
- Ignoring performance warnings in hot paths
- Marking check PASS when any Acceptance Criterion is unverified
