---
name: check
description: "Verify everything — parallel code review + security audit + performance analysis"
---

# /check — Verify Everything

You are starting the **Check** phase — comprehensive verification using parallel agents.

## Process

### Step 1: Gather scope
Identify what changed:
```bash
git diff --stat HEAD~1  # or against base branch
```

### Step 2: Launch 3 parallel agents

Use **Cursor sub-agents** (available in Cursor 1.7+) to run all three in parallel. Open a separate Composer session for each:

**Agent 1 — Reviewer** (use `agents/reviewer.md`):
- Code quality, logic correctness, style consistency
- Look for bugs, race conditions, edge cases
- Check test coverage for changed code

**Agent 2 — Auditor** (use `agents/auditor.md`):
- Security: injection, auth bypass, secret exposure, OWASP Top 10
- Performance: N+1 queries, memory leaks, unnecessary computation
- Refer to `references/security.md` and `references/performance.md`

**Agent 3 — Test runner**:
- Run the full test suite
- Report coverage delta
- Flag any flaky tests

**If Cursor sub-agents are not available**, run sequentially in this order: Reviewer → Auditor → Test runner. Collect all findings before synthesizing.

### Step 3: Synthesize
Combine findings into a single report:
```
## Check Report
### Code Quality: [PASS/WARN/FAIL]
### Security: [PASS/WARN/FAIL]
### Performance: [PASS/WARN/FAIL]
### Tests: [X/Y passing, Z% coverage]
### Action Items: [numbered list]
```

### Step 4: Act
- PASS on all: Ready for `/ship`
- WARN: Show warnings, ask user if they want to fix
- FAIL: List blockers, offer to fix with `/go`

## Red Flags
- Skipping security review for "small changes"
- Approving code with failing tests
- Ignoring performance warnings in hot paths
