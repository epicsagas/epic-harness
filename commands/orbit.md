---
description: "Autonomous pipeline: spec → go → check → ship → evolve in one shot. Selects mode automatically — Interactive (vague requirement, user runs /discover+/spec first), Council (complex, 4-voice parallel spec), or Direct (clear requirement, immediate build). Auto-retries check up to 3 times before pausing for user input. Runs /evolve automatically on PR+CI success."
---

# /orbit — Complete Orbit

Full autonomous pipeline: spec → go → check → ship → evolve in one shot.

## Phase Recovery Protocol

At the start of **every response** during an active orbit:

1. Run `ls $HARNESS_DIR/orbit/PIPELINE-*.json 2>/dev/null`
2. Find the file with `"status": "running"`
3. Read it. Verify `phase` matches where you left off
4. **If `phase` is ahead of where you think you are, trust the file** — you may have compacted
5. **Conflict resolution (crash-mid-update)**: If `phase_history` contains an entry for the current `phase` with a completed timestamp, treat that phase as done and advance to the next phase — `phase_history` wins over the `phase` field when they disagree. This covers the case where the state file was partially written before a crash.
6. Resume from the resolved phase. Do NOT re-ask mode selection, re-run spec, or re-discover

If no file with `"status": "running"` exists, orbit was not started or has completed. Do not invent one.

**Crash recovery**: If `updated_at` is older than 45 minutes and the pipeline is in `status: running`, assume a crash occurred. Read the state, determine the last completed phase from `phase_history` (rule 5 above applies), and resume from there. Report the recovery to the user.

> **Note:** The crash staleness threshold (45 min) is intentionally larger than the pipeline deadline (30 min) to avoid misclassifying a timeout'd-but-active pipeline as crashed. A pipeline that hit its deadline will show `status: timeout`, not `status: running`.

---

## Step 0: Preflight

```bash
HARNESS_DIR=$(epic-harness path)
mkdir -p $HARNESS_DIR/orbit
```

### Concurrent Orbit Guard

Before creating `PIPELINE-{timestamp}.json`:

1. Check with a JSON-aware read (not regex grep, which can false-positive on field values):
   ```bash
   for f in $HARNESS_DIR/orbit/PIPELINE-*.json; do
     [ -f "$f" ] && [ "$(jq -r .status "$f" 2>/dev/null)" = "running" ] && echo "$f" && break
   done
   ```
2. If a match is found: **STOP**. Tell the user:
   > "An orbit pipeline is already active (PIPELINE-{id}, phase={phase}). Say **orbit abort** to cancel it, or wait for it to complete."
3. Do NOT create a new pipeline file.

### Create Pipeline State

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
  "deadline": "{ISO-8601, now + 30 minutes}",
  "started_at": "{ISO-8601}",
  "updated_at": "{ISO-8601}",
  "phase_history": []
}
```

Update `updated_at` after every phase transition — this is the checkpoint if context is compacted.

### Deadline Enforcement

At the start of every step (Step 3 through Step 7):
1. Read `deadline` from pipeline state
2. If `now > deadline`: set `"status": "timeout"`, report to user, **STOP**
3. Default deadline is 30 minutes from `started_at`. User may override by editing the field.

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

### Git Preflight

Before creating a branch, verify git state:

```bash
# Clean working tree? (--porcelain catches untracked files that git diff --quiet HEAD misses)
[ -z "$(git status --porcelain)" ] || (echo "ERROR: Dirty working tree or untracked files. Commit or stash first." && exit 1)
# Not on detached HEAD?
git symbolic-ref -q HEAD || (echo "ERROR: Detached HEAD. Checkout a branch first." && exit 1)
# Branch doesn't already exist?
git show-ref --quiet refs/heads/feature/{goal_slug} && echo "WARN: Branch already exists, reusing" || true
```

Sanitize `goal_slug`: only `a-z`, `0-9`, `-`. Replace invalid characters with `-`.

2. Create branch: `git checkout -b feature/{goal_slug}` (reuse if exists)
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

**Write full check report to `$HARNESS_DIR/orbit/CHECK-{pipeline_id}.md`** — this is a separate file, not embedded in JSON. Set `"check_report": "$HARNESS_DIR/orbit/CHECK-{pipeline_id}.md"` in pipeline state. This ensures the report survives context compaction.

> **Security note:** `pipeline_id` used in the filename must contain only `a-z`, `0-9`, `-`, `_`. Replace any other characters with `-` before constructing the path. This prevents path traversal via a malformed pipeline ID.

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

**Gate:** Read check report from `$HARNESS_DIR/orbit/CHECK-{pipeline_id}.md`. If file does not exist → STOP. Report must show PASS/WARN.

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
<content of $HARNESS_DIR/orbit/CHECK-{pipeline_id}.md>

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
- Starting a second orbit while one is already running
- Ignoring the Phase Recovery Protocol after context compaction
- Proceeding past deadline without user consent
- Creating a branch with unsanitized goal_slug or dirty working tree
