---
description: "Autonomous pipeline that orchestrates skills: spec → go → audit → ship → evolve in one shot. Selects mode automatically — Interactive (vague requirement, user runs discover+spec first), Council (complex, 4-voice parallel spec), or Direct (clear requirement, immediate build). Auto-retries audit up to 3 times before pausing for user input. Runs evolve automatically on PR+CI success."
---

# /orbit — Complete Orbit

Autonomous pipeline that orchestrates skills: spec → go → audit → ship → evolve in one shot.

## Phase Recovery Protocol

At the start of **every response** during an active orbit:

1. Run `ls $HARNESS_DIR/orbit/PIPELINE-*.json 2>/dev/null`
2. Find the file with `"status": "running"`
3. Read it. Verify `phase` matches where you left off
4. **If `phase` is ahead of where you think you are, trust the file** — you may have compacted
5. **Conflict resolution (crash-mid-update)**: If `phase_history` contains an entry for the current `phase` with a completed timestamp, treat that phase as done and advance to the next phase — `phase_history` wins over the `phase` field when they disagree. This covers the case where the state file was partially written before a crash.
6. Resume from the resolved phase. Do NOT re-ask mode selection, re-run spec, or re-discover
7. **Worktree recovery**: If `worktree_name` is set in pipeline state:
   - Check if worktree still exists: `git worktree list | grep "{worktree_name}"`
   - If exists: re-enter via `EnterWorktree` with `path` parameter pointing to the worktree path
   - If not found: worktree was cleaned up externally — abort orbit with warning, set `"status": "aborted"`

If no file with `"status": "running"` exists, orbit was not started or has completed. Do not invent one.

**Crash recovery**: If `updated_at` is older than 45 minutes and the pipeline is in `status: running`, assume a crash occurred. Read the state, determine the last completed phase from `phase_history` (rule 5 above applies), and resume from there. Report the recovery to the user.

> **Worktree crash safety**: Pipeline state (`PIPELINE-*.json`), spec files, and audit reports live in `$HARNESS_DIR` (shared across worktrees — same git remote → same project slug). They survive worktree loss. If the worktree was cleaned up externally during a crash, abort the orbit and warn the user.

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
  "worktree_name": null,
  "original_cwd": null,
  "audit_fail_count": 0,
  "max_retries": 3,
  "audit_report": null,
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

**Orchestration setup (if EPIC_ORCHESTRATION enabled):**
If `EPIC_ORCHESTRATION=enabled`:
1. Generate an `orchestration_id` (same as pipeline id)
2. Add `orchestration_id` field to the pipeline JSON
3. The Rust `orchestrate` hook will use this ID to link pipeline state to orchestrator state

---

## Step 1: Mode Selection

> **Orbit Mode — how clear and complex is the requirement?**
>
> **1. Interactive** — Ambiguous or vague. You run **discover** → **spec** skills, then say "orbit go".
> **2. Council** — Clear but complex (architecture, trade-offs, multiple concerns). 4-voice council auto-generates spec and proceeds.
> **3. Direct** — Clear and simple. Spec written immediately, build starts now.

**Exception — skip mode selection if episteme results are in context** (`analyze_code` / `suggest_refactorings` output present): enter Direct automatically.

Do NOT proceed until the user picks a mode. Record: `"mode": "interactive" | "council" | "direct"`.

---

## Step 2A: Interactive Mode

Tell the user:
> "Run **discover** to frame the problem, then **spec** to define what to build. Say **orbit go** when the spec is saved with `status: approved`."

**STOP and wait.** On resume:
1. `ls -t $HARNESS_DIR/specs/SPEC-*.md | head -1` — find latest spec
2. Verify `status: approved` in frontmatter. If not, tell user to finish **spec** first.
3. Extract `goal_slug` → proceed to **Step 3**.

---

## Step 2B: Council Mode

**2B-1** — If the user hasn't described what to build, ask now.

**2B-2** — Launch 4 parallel voices via skill execution (`run_in_background: true`). Each receives ONLY the user's request + relevant codebase context. No cross-contamination.

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

**If episteme results are present:**
- Map each detected smell → Requirement (e.g., God Class → R1: Extract responsibilities)
- Map each suggested refactoring → AC (e.g., Extract Method → AC1: each method ≤ 20 lines, single responsibility)
- Use episteme output as authoritative source — do NOT re-derive from raw request

**Otherwise:** derive `goal_slug`, Requirements, and AC directly from the request.

Write `$HARNESS_DIR/specs/SPEC-{timestamp}.md` with `status: approved`. Show as FYI.

**Checkpoint — write before proceeding:**
```bash
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
jq --arg now "$NOW" --arg spec "$HARNESS_DIR/specs/SPEC-{timestamp}.md" \
  '.phase = "go" | .spec_file = $spec | .updated_at = $now |
   .phase_history += [{"phase": "spec", "status": "complete", "completed_at": $now}]' \
  "$PIPELINE_FILE" > /tmp/_orbit_tmp.json && mv /tmp/_orbit_tmp.json "$PIPELINE_FILE"
```

Proceed immediately to Step 3.

---

## Step 3: Go Phase

Update state: `"phase": "go"` — already written by the spec checkpoint above.

1. Load spec: `ls -t $HARNESS_DIR/specs/SPEC-*.md | head -1` — extract `goal_slug`, R1…, AC1…

### Git Preflight

Before entering worktree isolation, verify git state:

```bash
# Clean working tree? (--porcelain catches untracked files that git diff --quiet HEAD misses)
[ -z "$(git status --porcelain)" ] || (echo "ERROR: Dirty working tree or untracked files. Commit or stash first." && exit 1)
# Not on detached HEAD?
git symbolic-ref -q HEAD || (echo "ERROR: Detached HEAD. Checkout a branch first." && exit 1)
```

Sanitize `goal_slug`: only `a-z`, `0-9`, `-`. Replace invalid characters with `-`.

### Worktree Isolation

Enter an isolated git worktree so other sessions can freely switch branches without conflicting with the orbit pipeline:

1. Record current directory as `original_cwd` in pipeline state
2. Call `EnterWorktree` with name `orbit-{goal_slug}`
   - Creates `.claude/worktrees/orbit-{goal_slug}/` with a fresh branch from `origin/{default-branch}`
   - The worktree branch is `orbit-{goal_slug}`
3. Record `worktree_name` and `branch` in pipeline state
4. All subsequent phases (go/audit/ship) execute inside the worktree
5. State files remain accessible: `$HARNESS_DIR` resolves to the same `~/.harness/projects/{slug}/` (same git remote → same project slug)

> **Why worktree?** Orbit pipelines can run for 30+ minutes. Without isolation, switching branches in another session corrupts the in-progress worktree — uncommitted changes, branch state, and file edits collide. Worktree isolation guarantees the orbit operates on its own copy of the repo.
3. Plan tasks — map each Requirement:
   ```
   Task 1: [desc] — satisfies: R1 — depends on: none    — modifies: [files]
   Task 2: [desc] — satisfies: R2 — depends on: Task 1  — modifies: [files]
   ```
4. Execute via skill execution modes — **all inside the worktree**:
   - **go:plan** — task planning and dependency mapping
   - **go:build** — TDD: write test first → implement → green
   - `debug` skill on test failure; `verify` skill before done
   - `run_in_background: true` for independent tasks
   - `isolation: "worktree"` only if parallel tasks modify overlapping files

**Skill execution states:**

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
- Branch: orbit-{goal_slug} (worktree: orbit-{goal_slug})
- Requirements: R1 ✅, R2 ✅, ...
- AC verified: AC1 ✅, AC2 ✅, ...
- Tests: X/Y passing
- Skill executions: X DONE, Y CONCERNS, Z BLOCKED
```

**Orchestration-aware Go Phase:**
When `orchestration_id` is present in pipeline state:
1. Delegate execution management to the orchestrator — the **go** skill's orchestration extensions handle this
2. The Go Phase reads `$HARNESS_DIR/orchestrator/run.json` for real-time execution status instead of waiting for background notifications
3. After Go Phase completes, include orchestration metrics in the Go Report

**Checkpoint — write before proceeding to Step 4:**
```bash
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
jq --arg now "$NOW" \
  '.phase = "audit" | .updated_at = $now |
   .phase_history += [{"phase": "go", "status": "complete", "completed_at": $now}]' \
  "$PIPELINE_FILE" > /tmp/_orbit_tmp.json && mv /tmp/_orbit_tmp.json "$PIPELINE_FILE"
```

---

## Step 4: Audit Phase

State is already `"phase": "audit"` from the go checkpoint. Set `audit_report` path now:
```bash
AUDIT_REPORT="$HARNESS_DIR/orbit/AUDIT-${PIPELINE_ID}.md"
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
jq --arg now "$NOW" --arg report "$AUDIT_REPORT" \
  '.audit_report = $report | .updated_at = $now' \
  "$PIPELINE_FILE" > /tmp/_orbit_tmp.json && mv /tmp/_orbit_tmp.json "$PIPELINE_FILE"
```

```bash
git diff --stat $(git merge-base HEAD main)
git diff --name-only $(git merge-base HEAD main)
```

Classify changed files by scope: API · Frontend · Backend · Database · Infra · Docs · Tests.

**Launch 3 core skill execution modes in parallel** (`run_in_background: true`):
1. **audit:code** — code quality, logic, style, test coverage, spec Requirements coverage
2. **audit:security** — security (OWASP Top 10) + performance (N+1, leaks)
3. **audit:test** — full test suite, AC verification, coverage delta

**Conditional scope checks:**
- API: contract testing, request validation
- Frontend: accessibility, semantic HTML
- Database: migration safety, rollback plan
- Infra: config validation, secret detection

**Orchestration-aware Audit:**
When orchestration is active:
1. The **audit:code** execution reads `$HARNESS_DIR/orchestrator/` for activity history
2. The **audit:security** execution checks for concurrent write conflicts logged in execution streams
3. Include orchestration-specific metrics in the Audit Report

**Audit Report:**
```
## Audit Report
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

**Collect execution results before writing the report.** After all 3 background skill executions complete, synthesize their outputs into the Audit Report. Do NOT write the file until all executions have reported back — a partial report is worse than no report for crash recovery.

Write full audit report to `$HARNESS_DIR/orbit/AUDIT-{pipeline_id}.md` — this is a separate file, not embedded in JSON. The `audit_report` path was already set in pipeline state at the start of this step.

> **Security note:** `pipeline_id` used in the filename must contain only `a-z`, `0-9`, `-`, `_`. Replace any other characters with `-` before constructing the path. This prevents path traversal via a malformed pipeline ID.

**Checkpoint — confirm report written:**
```bash
[ -f "$AUDIT_REPORT" ] || (echo "ERROR: Audit report not written. Do not proceed." && exit 1)
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
jq --arg now "$NOW" '.updated_at = $now' \
  "$PIPELINE_FILE" > /tmp/_orbit_tmp.json && mv /tmp/_orbit_tmp.json "$PIPELINE_FILE"
```

→ **Step 5**.

---

## Step 5: Verdict

| Result | Action |
|--------|--------|
| All PASS + all AC ✅ | Write checkpoint (audit: pass) → **Step 6** |
| WARN | Write checkpoint (audit: warn), auto-proceed → **Step 6** |
| FAIL or AC missing | Increment `audit_fail_count`, write checkpoint |

**Checkpoint on PASS or WARN:**
```bash
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
jq --arg now "$NOW" --arg verdict "pass" \
  '.phase = "ship" | .updated_at = $now |
   .phase_history += [{"phase": "audit", "status": $verdict, "completed_at": $now}]' \
  "$PIPELINE_FILE" > /tmp/_orbit_tmp.json && mv /tmp/_orbit_tmp.json "$PIPELINE_FILE"
```

**Checkpoint on FAIL (increment counter):**
```bash
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
jq --arg now "$NOW" \
  '.audit_fail_count += 1 | .updated_at = $now |
   .phase_history += [{"phase": "audit", "status": "fail", "completed_at": $now}]' \
  "$PIPELINE_FILE" > /tmp/_orbit_tmp.json && mv /tmp/_orbit_tmp.json "$PIPELINE_FILE"
```

**On FAIL — if `audit_fail_count < 3`:**
1. Read Action Items from audit report
2. Plan targeted fix tasks per blocker
3. Execute via skill execution modes (TDD + debug + verify)
4. Return to **Step 4**

**On FAIL — if `audit_fail_count >= 3`:** set `"status": "paused"`.

> **Orbit paused** — 3 audit cycles failed. Decide:
> - **"continue"** — another fix cycle (increments `max_retries`)
> - **"abort"** — stop, set `"status": "aborted"`

**Orchestration cleanup:**
On PASS or WARN:
1. Update `$HARNESS_DIR/orchestrator/run.json` status to "complete"
2. Preserve orchestrator state directory for /status and /evolve analysis
On FAIL:
1. Keep orchestrator state for debugging
2. Include execution stream data in Action Items for targeted fixes

---

## Step 6: Ship

Update state: `"phase": "ship"`.

**Gate:** Read audit report from `$HARNESS_DIR/orbit/AUDIT-{pipeline_id}.md`. If file does not exist → STOP. Report must show PASS/WARN.

**6a. Integration verification** — run directly in worktree (already isolated from main tree):
- Clean build artifacts first: `cargo clean` / `npm run clean` / equivalent
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

## Audit Report
<content of $HARNESS_DIR/orbit/AUDIT-{pipeline_id}.md>

**Orchestration data in PR:**
Include in PR body:
- Orchestration ID
- Skill execution count and final states
- Total orchestration elapsed time

## Test Plan
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual verification done
EOF
)"
```

**6d. CI:** Check CI status — do not block orbit on CI:

```bash
# Check if gh is available and CI is configured
if command -v gh &>/dev/null && gh pr checks <PR_NUMBER> 2>/dev/null | grep -q .; then
  CI_RETRY=0
  CI_MAX_RETRIES=2
  while [ $CI_RETRY -le $CI_MAX_RETRIES ]; do
    gh pr checks <PR_NUMBER> --watch
    if [ $? -eq 0 ]; then
      CI_STATUS="PASS"
      break
    fi
    CI_RETRY=$((CI_RETRY + 1))
    if [ $CI_RETRY -le $CI_MAX_RETRIES ]; then
      # Diagnose and fix automatically, then re-check
      echo "CI failed (attempt $CI_RETRY/$CI_MAX_RETRIES) — diagnosing and fixing..."
    else
      CI_STATUS="FAIL"
    fi
  done
else
  # No CI configured (e.g., CodeCommit without gh Actions) — skip
  CI_STATUS="N/A"
  CI_NOTE="No CI configured — skipped"
fi
```

- CI fails → diagnose and fix automatically, retry up to 2 times.
- All retries exhausted → record `CI: FAIL`, proceed to Step 7 (evolve will analyze the failure pattern).
- CI absent / `gh` not available → set `CI: N/A`, proceed immediately to Step 7.
- Do NOT block orbit progression regardless of CI outcome.

**Ship Report:**
```
## Ship Report
- Spec: SPEC-{timestamp} ({goal_slug})
- PR: <URL>
- CI: PASS/FAIL/N/A
- Ready to merge: YES/NO
```

**Checkpoint — write before exiting worktree:**
```bash
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
jq --arg now "$NOW" \
  '.phase = "complete" | .updated_at = $now |
   .phase_history += [{"phase": "ship", "status": "complete", "completed_at": $now}]' \
  "$PIPELINE_FILE" > /tmp/_orbit_tmp.json && mv /tmp/_orbit_tmp.json "$PIPELINE_FILE"
```

**6e. Exit worktree:** Call `ExitWorktree` with `action: "keep"` (preserve worktree and branch until user merges the PR — branch is needed for PR head). The session returns to the original working directory. Pipeline state, specs, and audit reports are already in `$HARNESS_DIR` (shared).

> **Orbit context:** After pushing ship to `phase_history`, **immediately proceed to Step 7 (Orbit Complete + Evolve)**. Do NOT stop here.

---

## Step 7: Orbit Complete

State is already `"phase": "complete"` from the ship checkpoint. Write final status:
```bash
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
jq --arg now "$NOW" \
  '.status = "complete" | .updated_at = $now' \
  "$PIPELINE_FILE" > /tmp/_orbit_tmp.json && mv /tmp/_orbit_tmp.json "$PIPELINE_FILE"
```

**Consolidated Report:**
```
## Orbit Complete
- Pipeline: PIPELINE-{id}
- Mode: {interactive|council|direct}
- Spec: SPEC-{timestamp} ({goal_slug})
- Branch: orbit-{goal_slug}
- Worktree: {worktree_name} (preserved for PR)
- PR: {URL}
- Duration: {started_at → now}

| Phase | Status   | Retries          |
|-------|----------|------------------|
| Spec  | approved | 0                |
| Go    | complete | 0                |
| Audit | PASS     | {audit_fail_count}|
| Ship  | complete | 0                |

| Orchestration | {enabled/disabled} | {execution_count} skill executions |

### Key Decisions
{from council/direct synthesis}

### Go Report     {embedded}
### Audit Report  {embedded}
### Ship Report   {embedded}
```

### Step 7a: Evolve (Auto)

**Always run** — regardless of CI status (PASS / FAIL / N/A). Evolve on every completed orbit:
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

**Checkpoint — final write:**
```bash
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
jq --arg now "$NOW" \
  '.updated_at = $now |
   .phase_history += [{"phase": "evolve", "status": "complete", "completed_at": $now}]' \
  "$PIPELINE_FILE" > /tmp/_orbit_tmp.json && mv /tmp/_orbit_tmp.json "$PIPELINE_FILE"
```

**Orbit + Evolve complete. The next session starts smarter.**

---

## Red Flags
- Skipping mode selection without episteme results present
- Proceeding past spec without `status: approved`
- Continuing after 3 audit failures without user consent
- Skipping isolated integration test before PR
- Shipping with FAIL on any security audit item
- Losing `audit_report` between phases
- Creating PR without full audit report in body
- Starting a second orbit while one is already running
- Ignoring the Phase Recovery Protocol after context compaction
- Proceeding past deadline without user consent
- Creating a branch with unsanitized goal_slug or dirty working tree
- Losing worktree reference between phases (worktree_name missing from pipeline state)
- Forgetting to exit worktree before orbit complete (leaves session in wrong directory)
- Entering worktree without saving original_cwd (can't find state files after)
- Skipping a phase checkpoint jq write — if context compacts mid-orbit, the phase will re-run
- Launching background audit skill executions before setting `audit_report` path in pipeline state
- Writing audit report before all skill executions have completed — partial reports cause wrong verdicts
- Skipping `updated_at` refresh — stale timestamp triggers false crash recovery after 45 minutes
