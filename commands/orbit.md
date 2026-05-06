---
description: "Complete orbit — autonomous spec through ship. Choose mode, then hands-off until PR."
---

# /orbit — Complete Orbit

Full autonomous pipeline: spec → go → check → ship in one shot.

## Step 0: Preflight

```bash
HARNESS_DIR=$(epic-harness path)
mkdir -p $HARNESS_DIR/orbit
```

Create `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json`:
```json
{
  "id": "{timestamp}",
  "mode": null,
  "phase": "mode_select",
  "status": "running",
  "spec_file": null,
  "goal_slug": null,
  "branch": null,
  "check_fail_count": 0,
  "max_retries": 3,
  "check_report": null,
  "started_at": "{ISO-8601}",
  "updated_at": "{ISO-8601}",
  "phase_history": []
}
```

Update after every phase transition — this is the checkpoint if context is compacted.

---

## Step 1: Mode Selection

> **Orbit Mode — how clear and complex is the requirement?**
>
> **1. Interactive** — Ambiguous or vague. You run `/discover` → `/spec`, then say "orbit go".
> **2. Council** — Clear but complex (architecture, trade-offs, multiple concerns). 4-voice council auto-generates spec and proceeds.
> **3. Direct** — Clear and simple. Spec written immediately, build starts now.

**Exception — skip mode selection if syntagma results are in context** (`analyze_code` / `suggest_refactorings` output present): enter Direct automatically.

Do NOT proceed until the user picks a mode. Record: `"mode": "interactive" | "council" | "direct"`.

---

## Step 2A: Interactive Mode

Tell the user:
> "Run `/discover` to frame the problem, then `/spec` to define what to build. Say **orbit go** when the spec is saved with `status: approved`."

**STOP and wait.** On resume:
1. `ls -t $HARNESS_DIR/specs/SPEC-*.md | head -1` — find latest spec
2. Verify `status: approved` in frontmatter. If not, tell user to finish `/spec` first.
3. Extract `goal_slug` → proceed to **Step 3**.

---

## Step 2B: Council Mode

**2B-1** — If the user hasn't described what to build, ask now.

**2B-2** — Launch 4 subagents in parallel (`run_in_background: true`). Each receives ONLY the user's request + relevant codebase context. No cross-contamination.

| Voice | Focus |
|-------|-------|
| **Architect** | Maintainability, extensibility, architectural fit |
| **Skeptic** | Simpler alternatives, hidden costs, YAGNI |
| **Pragmatist** | Timeline, user impact, MVP scope |
| **Critic** | Edge cases, failure modes, migration risks, security |

**2B-3** — After all 4 report back, synthesize: list agreement (→ spec) and disagreement (→ trade-offs). Present to user.

**2B-4** — Write spec at `$HARNESS_DIR/specs/SPEC-{timestamp}.md` with `status: approved`:

```yaml
---
status: approved
created: {ISO-8601}
goal_slug: {kebab-case}
---
## Goal
{one sentence}

## Scope
### In
- {what we're building}
### Out
- {what we're NOT building}

## Requirements
- R1: ...
- R2: ...

## Acceptance Criteria
- AC1: {verifiable, maps to R1}
- AC2: {verifiable, maps to R2}

## Technical Notes
{key decisions from council synthesis}
```

Record via `mem_add` (type=decision, importance=0.9). Show spec as FYI → **proceed immediately to Step 3**.

---

## Step 2C: Direct Mode

Write spec immediately — no council, no discovery.

**If syntagma results are present:**
- Map each detected smell → Requirement (e.g., God Class → R1: Extract responsibilities)
- Map each suggested refactoring → AC (e.g., Extract Method → AC1: each method ≤ 20 lines, single responsibility)
- Use syntagma output as authoritative source — do NOT re-derive from raw request

**Otherwise:** derive `goal_slug`, Requirements, and AC directly from the request.

Write `$HARNESS_DIR/specs/SPEC-{timestamp}.md` with `status: approved`. Show as FYI → **proceed immediately to Step 3**.

---

## Step 3: Go Phase

Update state: `"phase": "go"`.

1. Load spec: `ls -t $HARNESS_DIR/specs/SPEC-*.md | head -1` — extract `goal_slug`, R1…, AC1…
2. Create branch: `git checkout -b feature/{goal_slug}`
3. Plan tasks — map each Requirement:
   ```
   Task 1: [desc] — satisfies: R1 — depends on: none    — modifies: [files]
   Task 2: [desc] — satisfies: R2 — depends on: Task 1  — modifies: [files]
   ```
4. Execute via subagents (Agent tool):
   - TDD: write test first → implement → green
   - `debug` skill on test failure; `verify` skill before done
   - `run_in_background: true` for independent tasks
   - `isolation: "worktree"` only if parallel tasks modify overlapping files

**Subagent states:**

| State | Action |
|-------|--------|
| DONE | Proceed |
| DONE_WITH_CONCERNS | Review — auto-escalate security/data/breaking issues |
| NEEDS_CONTEXT | Ask user, then resume |
| BLOCKED | Try one alternative. If still blocked, skip and report |

5. Integrate: run full test suite, verify each AC, fix failures.

**Go Report:**
```
## Go Report
- Spec: SPEC-{timestamp} ({goal_slug})
- Branch: feature/{goal_slug}
- Requirements: R1 ✅, R2 ✅, ...
- AC verified: AC1 ✅, AC2 ✅, ...
- Tests: X/Y passing
- Subagents: X DONE, Y CONCERNS, Z BLOCKED
```

Push `{"phase": "go", "status": "complete"}` to `phase_history` → **Step 4**.

---

## Step 4: Check Phase

Update state: `"phase": "check"`.

```bash
git diff --stat $(git merge-base HEAD main)
git diff --name-only $(git merge-base HEAD main)
```

Classify changed files by scope: API · Frontend · Backend · Database · Infra · Docs · Tests.

**Launch 3 core agents in parallel** (`run_in_background: true`):
1. **Reviewer** — code quality, logic, style, test coverage, spec Requirements coverage
2. **Auditor** — security (OWASP Top 10) + performance (N+1, leaks)
3. **Test runner** — full test suite, AC verification, coverage delta

**Conditional scope checks:**
- API: contract testing, request validation
- Frontend: accessibility, semantic HTML
- Database: migration safety, rollback plan
- Infra: config validation, secret detection

**Check Report:**
```
## Check Report
- Spec: SPEC-{timestamp} ({goal_slug})
- Branch: {branch}
- Scopes: [API, Backend, Tests, ...]
- Code Quality: PASS/WARN/FAIL
- Security:     PASS/WARN/FAIL
- Performance:  PASS/WARN/FAIL
- Tests:        X/Y passing, Z% coverage
- R1: ✅/❌  R2: ✅/❌
- AC1: ✅/❌  AC2: ✅/❌
### Action Items
1. [blocker or warning]
```

**Store full check report text in `check_report` field of pipeline state** — must survive context compaction for Ship phase.

→ **Step 5**.

---

## Step 5: Verdict

| Result | Action |
|--------|--------|
| All PASS + all AC ✅ | Push `{"phase": "check", "status": "pass"}` → **Step 6** |
| WARN | Log warnings, auto-proceed → **Step 6** |
| FAIL or AC missing | Increment `check_fail_count` |

**On FAIL — if `check_fail_count < 3`:**
1. Read Action Items from check report
2. Plan targeted fix tasks per blocker
3. Execute via subagents (TDD + debug + verify)
4. Return to **Step 4**

**On FAIL — if `check_fail_count >= 3`:** set `"status": "paused"`.

> **Orbit paused** — 3 check cycles failed. Decide:
> - **"continue"** — another fix cycle (increments `max_retries`)
> - **"abort"** — stop, set `"status": "aborted"`

---

## Step 6: Ship

Update state: `"phase": "ship"`.

**Gate:** check report must exist and show PASS/WARN. Missing → STOP.

**6a. Isolated integration test** — launch agent with `isolation: "worktree"`:
- Full build from scratch · complete test suite · linter + formatter
- Fail → STOP. Do NOT create PR.

**6b. Git hygiene:**
- All changes committed (Conventional Commits)
- Rebase on latest base branch if needed
- Squash fixup commits if appropriate

**6c. Create PR:**
```bash
gh pr create --title "<goal from spec>" --body "$(cat <<'EOF'
## Summary
<Goal>

## Spec
- ID: SPEC-{timestamp}
- Pipeline: PIPELINE-{id}
- Requirements: R1, R2, ...

## Changes
<bullet list>

## Acceptance Criteria Verified
- AC1: ✅  AC2: ✅

## Check Report
<full check report>

## Test Plan
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual verification done
EOF
)"
```

**6d. CI:** `gh pr checks <PR_NUMBER> --watch` — diagnose and fix automatically on failure.

**Ship Report:**
```
## Ship Report
- Spec: SPEC-{timestamp} ({goal_slug})
- PR: <URL>
- CI: PASS/FAIL
- Ready to merge: YES/NO
```

Push `{"phase": "ship", "status": "complete"}` to `phase_history`.

---

## Step 7: Orbit Complete

Update state: `"phase": "complete"`, `"status": "complete"`.

**Consolidated Report:**
```
## Orbit Complete
- Pipeline: PIPELINE-{id}
- Mode: {interactive|council|direct}
- Spec: SPEC-{timestamp} ({goal_slug})
- Branch: feature/{goal_slug}
- PR: {URL}
- Duration: {started_at → now}

| Phase | Status   | Retries          |
|-------|----------|------------------|
| Spec  | approved | 0                |
| Go    | complete | 0                |
| Check | PASS     | {check_fail_count}|
| Ship  | complete | 0                |

### Key Decisions
{from council/direct synthesis}

### Go Report     {embedded}
### Check Report  {embedded}
### Ship Report   {embedded}
```

### Step 7a: Evolve (Auto)

PR created + CI green → run `/evolve` automatically:
1. Read `$HARNESS_DIR/obs/` session logs
2. Read `$HARNESS_DIR/metrics.json` + `evolution.jsonl`
3. Detect failure patterns, weak tools, weak file types
4. Seed evolved skills if thresholds met; gate + cap at `MAX_EVOLVED_SKILLS` (10)

```
## Evolve Report
- Patterns detected: {list or "none"}
- Skills evolved:    {list or "none"}
- Trend: {improving | stable | declining}
```

Push `{"phase": "evolve", "status": "complete"}` to `phase_history`.

**Orbit + Evolve complete. The next session starts smarter.**

---

## Red Flags
- Skipping mode selection without syntagma results present
- Proceeding past spec without `status: approved`
- Continuing after 3 check failures without user consent
- Skipping isolated integration test before PR
- Shipping with FAIL on any security check item
- Losing `check_report` between phases
- Creating PR without full check report in body
