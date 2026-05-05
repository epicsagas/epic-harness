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

### Step 2: Adaptive expert dispatch

First, classify changed files by scope:
```bash
git diff --name-only $(git merge-base HEAD main)
```

**Scope detection rules:**
| Pattern | Scope | Extra checks |
|---------|-------|-------------|
| `*.api.*`, `*route*`, `*controller*`, `*handler*` | API | + Contract testing, request validation |
| `*.tsx`, `*.jsx`, `*.vue`, `*.svelte`, `*.css` | Frontend | + Accessibility, visual regression hints |
| `*.sql`, `*migration*`, `*schema*` | Database | + Migration safety, rollback plan |
| `*.rs`, `Cargo.toml`, `*.go`, `go.mod` | Backend | + Build verification, type safety |
| `*.test.*`, `*.spec.*`, `__tests__/` | Tests | + Coverage delta, flaky test detection |
| `Dockerfile*`, `*.yml`, `*.yaml`, `Makefile` | Infra | + Config validation, secret detection |
| `*.md`, `*.txt` | Docs | + Link checking, freshness |

**Always run (core 3 agents):**

1. **Reviewer** (use `agents/reviewer.md`):
   - Code quality, logic correctness, style consistency
   - Look for bugs, race conditions, edge cases
   - Check test coverage for changed code
   - Verify each spec Requirement is addressed in the diff

2. **Auditor** (use `agents/auditor.md`):
   - Security: injection, auth bypass, secret exposure, OWASP Top 10
   - Performance: N+1 queries, memory leaks, unnecessary computation
   - Refer to `references/security.md` and `references/performance.md`

3. **Test runner**:
   - Run the full test suite
   - Verify each Acceptance Criterion (AC1, AC2, ...) is demonstrably met
   - Report coverage delta
   - Flag any flaky tests

**Conditionally add (scope-based):**
- **API scope detected**: Add contract check — verify request/response schemas, test edge cases (empty, malformed, oversized inputs)
- **Frontend scope detected**: Add accessibility check — semantic HTML, ARIA labels, keyboard navigation
- **Database scope detected**: Add migration check — verify `up` has matching `down`, check for destructive operations (DROP, data loss), suggest transaction wrapping
- **Infra scope detected**: Add config check — validate YAML/TOML syntax, check for hardcoded secrets, verify environment variable references

Launch all agents with `run_in_background: true`.

### Step 3: Synthesize

Combine findings into a single report:

```
## Check Report
- Spec: SPEC-{timestamp} ({goal_slug})
- Branch: {current branch}

### Change Scope
- Scopes detected: [API, Frontend, Backend, Database, Infra, Docs, Tests]
- Scope-specific checks: [list what ran]

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
