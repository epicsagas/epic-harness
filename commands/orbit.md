---
description: "Complete orbit — autonomous spec through ship. Choose interactive or council mode, then hands-off until PR."
---

# /orbit — Complete Orbit

You are entering **Orbit** mode — the full autonomous pipeline from spec to PR in one shot.

## Step 0: Preflight

Run `HARNESS_DIR=$(epic-harness path)` to resolve the data directory.

**Initialize pipeline state:**
```bash
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

Update this file after every phase transition. It is your checkpoint if context is compacted.

## Step 1: Mode Selection

Assess the user's request and present three options:

> **Orbit Mode — how clear and complex is the requirement?**
>
> **1. Interactive discover** — The requirement is ambiguous or vague. You run `/discover` and `/spec` to define it, then say "orbit go" and I'll take over from there.
>
> **2. Council auto-spec** — The requirement is clear but complex (architectural decisions, trade-offs, or multiple concerns). A 4-voice council (Architect, Skeptic, Pragmatist, Critic) will analyze it, auto-generate a spec, and proceed.
>
> **3. Direct build** — The requirement is clear and simple. I'll write the spec immediately and start building without council or discovery.

Do NOT proceed until the user picks a mode. Record the choice in pipeline state (`"mode": "interactive"`, `"mode": "council"`, or `"mode": "direct"`).

---

## Step 2C: Direct Build Mode

Write a spec immediately — no council, no discovery.

**If syntagma analysis results are present in context** (from `analyze_code` / `suggest_refactorings`):
1. Map each detected smell → Requirement (e.g., "God Class detected in `UserService`" → R1: Extract responsibilities into focused classes)
2. Map each suggested refactoring → Acceptance Criterion (e.g., "Extract Method" → AC1: Each extracted method ≤ 20 lines, single responsibility)
3. Use the syntagma output as the authoritative source — do NOT re-derive from the raw request
4. Skip mode selection prompt entirely — enter Direct Build automatically

**Otherwise** (clear + simple request, no syntagma output):
1. Derive `goal_slug`, Requirements, and Acceptance Criteria directly from the request

**Both paths converge here:**
- Write `$HARNESS_DIR/specs/SPEC-{timestamp}.md` with `status: approved`
- Show the spec as an FYI, then **proceed immediately to Step 3**

---

## Step 2A: Interactive Mode

Tell the user:

> "Run `/discover` to frame the problem, then `/spec` to define what to build. When the spec is saved with `status: approved`, say **orbit go** to start the autonomous pipeline."

Then **STOP and wait**. Do nothing until the user says "orbit go" or indicates the spec is ready.

On resume:
1. Find the latest approved spec: `ls -t $HARNESS_DIR/specs/SPEC-*.md | head -1`
2. Verify frontmatter has `status: approved`. If not, tell the user to finish `/spec` first.
3. Extract `goal_slug` and proceed to **Step 3**.

## Step 2B: Council Auto-Spec Mode

### 2B-1: Gather the request

If the user hasn't stated what to build, ask: "What do you want to build? Describe the feature, fix, or change in your own words."

### 2B-2: Summon 4 Voices (Parallel)

Launch 4 independent subagents via the Agent tool with `run_in_background: true`. Each receives ONLY:
- The user's request (verbatim)
- Relevant codebase context (file paths, current architecture, key dependencies)
- Their role and focus area

**They do NOT receive:** the full conversation, other voices' opinions, or any leaning.

| Voice | Role | Focus |
|-------|------|-------|
| **Architect** | Long-term correctness | Maintainability, extensibility, architectural alignment, how this fits the existing codebase |
| **Skeptic** | Challenge assumptions | Simpler alternatives, hidden costs, "what if we don't build this?", YAGNI |
| **Pragmatist** | Ship it now | Timeline, user impact, operational complexity, what's the MVP scope |
| **Critic** | Find the cracks | Edge cases, failure modes, migration risks, rollback difficulty, security concerns |

### 2B-3: Synthesize

After all 4 voices report back:
1. List areas of **agreement** (strong signal — these should be in the spec)
2. List areas of **disagreement** (where trade-offs live)
3. Present the synthesis to the user

### 2B-4: Generate Spec and Proceed

From the synthesis, write a spec file at `$HARNESS_DIR/specs/SPEC-{timestamp}.md` with `status: approved`:

```yaml
---
status: approved
created: {ISO-8601 timestamp}
goal_slug: {kebab-case-goal-summary}
---

## Goal
{one clear sentence}

## Scope
### In
- {what we're building}

### Out
- {what we're explicitly NOT building}

## Requirements
- R1: {requirement}
- R2: {requirement}

## Acceptance Criteria
- AC1: {verifiable criterion for R1}
- AC2: {verifiable criterion for R2}

## Technical Notes
{key architectural decisions from council synthesis}
```

Record the decision via `mem_add` (type=decision, importance=0.9). Show the generated spec to the user as an FYI, then **proceed immediately to Step 3 without waiting for approval**.

---

## Step 3: Build (Go Phase)

Update pipeline state: `"phase": "go"`.

**Load the approved spec:**
1. `ls -t $HARNESS_DIR/specs/SPEC-*.md | head -1`
2. Extract `goal_slug`, Requirements (R1...), Acceptance Criteria (AC1...)

**Create feature branch:**
```bash
git checkout -b feature/{goal_slug}
```

**Plan tasks** — map each Requirement to tasks:
```
Task 1: [description] — satisfies: R1 — depends on: none — modifies: [file list]
Task 2: [description] — satisfies: R2 — depends on: Task 1 — modifies: [file list]
```

**Execute tasks** — launch subagents (Agent tool) with:
- Task description + which Requirement(s) it satisfies
- TDD instruction: write test first → implement → green
- Invoke `debug` skill on test failure
- Invoke `verify` skill before reporting done
- `run_in_background: true` for independent tasks
- `isolation: "worktree"` only if parallel tasks modify overlapping files

**Handle subagent states:**

| State | Action |
|-------|--------|
| DONE | Proceed |
| DONE_WITH_CONCERNS | Review — auto-escalate security/data/breaking concerns |
| NEEDS_CONTEXT | Ask user specific questions, then resume |
| BLOCKED | Try one alternative. If still blocked, skip and report |

**Integrate** after all tasks complete:
- Run full test suite
- Verify each Acceptance Criterion is met
- Fix any failures

**Go Report:**
```
## Go Report
- Spec: SPEC-{timestamp} ({goal_slug})
- Branch: feature/{goal_slug}
- Requirements satisfied: R1 ✅, R2 ✅, ...
- Acceptance criteria verified: AC1 ✅, AC2 ✅, ...
- Tests: X/Y passing
- Subagent states: X DONE, Y CONCERNS, Z BLOCKED
```

Update pipeline state: push `{"phase": "go", "status": "complete"}` to `phase_history`. Proceed to **Step 4**.

---

## Step 4: Verify (Check Phase)

Update pipeline state: `"phase": "check"`.

**Gather scope:**
```bash
git diff --stat $(git merge-base HEAD main)
git diff --name-only $(git merge-base HEAD main)
```

**Classify changed files** by scope (API, Frontend, Database, Backend, Tests, Infra, Docs).

**Launch 3 core agents in parallel** (`run_in_background: true`):

1. **Reviewer** (agents/reviewer.md) — code quality, logic, style, test coverage, spec Requirements coverage
2. **Auditor** (agents/auditor.md) — security (OWASP Top 10) + performance (N+1, leaks), references `references/security.md` and `references/performance.md`
3. **Test runner** — full test suite, AC verification, coverage delta

**Conditionally add** scope-specific checks:
- API: contract testing, request validation
- Frontend: accessibility, semantic HTML
- Database: migration safety, rollback plan
- Infra: config validation, secret detection

**Synthesize into Check Report:**
```
## Check Report
- Spec: SPEC-{timestamp} ({goal_slug})
- Branch: {current branch}

### Change Scope
- Scopes detected: [API, Frontend, Backend, Database, Infra, Docs, Tests]

### Code Quality: [PASS/WARN/FAIL]
### Security: [PASS/WARN/FAIL]
### Performance: [PASS/WARN/FAIL]
### Tests: [X/Y passing, Z% coverage]

### Spec Coverage
- R1: ✅/❌
- R2: ✅/❌
- AC1: ✅/❌
- AC2: ✅/❌

### Action Items
1. [blocker or warning]
```

**PRESERVE the full check report text** — store it in pipeline state `check_report` field. This is critical because it must survive context compaction for the ship phase.

Proceed to **Step 5**.

---

## Step 5: Verdict

**Decision gate based on check results:**

### All PASS + all AC verified
Update pipeline state: push `{"phase": "check", "status": "pass"}` to `phase_history`. Proceed directly to **Step 6 (Ship)**.

### WARN
Log warnings in pipeline state. Auto-proceed to **Step 6 (Ship)** with warnings noted.

### FAIL or AC missing
Increment `check_fail_count` in pipeline state.

**If `check_fail_count < max_retries` (3):**
1. Read the check report's Action Items
2. Plan targeted fix tasks addressing each blocker
3. Execute fixes via subagents (same TDD + debug + verify rules)
4. Return to **Step 4 (Check)** — re-run full verification

**If `check_fail_count >= max_retries` (3):**
Update pipeline state: `"status": "paused"`.

> **Orbit paused** — 3 check cycles failed. Review the failures above and decide:
> - **"continue"** — I'll attempt another fix cycle
> - **"abort"** — Stop the orbit and keep what's done

Wait for user input. On "continue": increment `max_retries` by 1, resume fix cycle. On "abort": update to `"status": "aborted"`, report and stop.

---

## Step 6: Ship

Update pipeline state: `"phase": "ship"`.

**Gate:** Verify check report exists and shows PASS. If missing, STOP and report the error.

### 6a. Isolated Integration Test
Launch an agent with `isolation: "worktree"`:
- Full build from scratch
- Complete test suite
- Linter and formatter checks

If isolated test fails → STOP. Report failure. Do NOT create the PR.

### 6b. Git Hygiene
- Ensure all changes committed with Conventional Commits
- Rebase on latest base branch if needed
- Squash fixup commits if appropriate

### 6c. Create PR
```bash
gh pr create --title "<goal from spec>" --body "$(cat <<'EOF'
## Summary
<Goal from spec>

## Spec
- Spec ID: SPEC-{timestamp}
- Orbit Pipeline: PIPELINE-{id}
- Requirements: R1, R2, ...

## Changes
<bullet list of key changes>

## Acceptance Criteria Verified
- AC1: ✅
- AC2: ✅

## Check Report
<paste full Check Report>

## Test Plan
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual verification done
EOF
)"
```

### 6d. CI Verification
```bash
gh pr checks <PR_NUMBER> --watch
```
If CI fails, diagnose and fix automatically.

### Ship Report:
```
## Ship Report
- Spec: SPEC-{timestamp} ({goal_slug})
- PR: <URL>
- CI: [PASS/FAIL]
- Ready to merge: [YES/NO]
```

Update pipeline state: push `{"phase": "ship", "status": "complete"}` to `phase_history`.

---

## Step 7: Orbit Complete

Update pipeline state: `"phase": "complete"`, `"status": "complete"`.

### Consolidated Report
```
## Orbit Complete
- Pipeline: PIPELINE-{id}
- Mode: {interactive|council}
- Spec: SPEC-{timestamp} ({goal_slug})
- Branch: feature/{goal_slug}
- PR: {URL}
- Duration: {started_at → now}

### Phase Summary
| Phase | Status | Retries |
|-------|--------|---------|
| Spec | approved | 0 |
| Go | complete | 0 |
| Check | PASS | {check_fail_count} |
| Ship | complete | 0 |

### Key Decisions
{from council synthesis or inline decisions}

### Go Report
{embedded}

### Check Report
{embedded}

### Ship Report
{embedded}
```

### Step 7a: Evolve (Auto)

PR created and CI green — run `/evolve` automatically now:

1. Read `$HARNESS_DIR/obs/` observation logs from this session
2. Read `$HARNESS_DIR/metrics.json` and `$HARNESS_DIR/evolution.jsonl`
3. Analyze failure patterns, weak tools, weak file types from this orbit session
4. Seed evolved skills if thresholds are met
5. Gate: validate evolved skills, cap at `MAX_EVOLVED_SKILLS` (10)
6. Report evolved skill summary

```
## Evolve Report
- Sessions analyzed: 1 (this orbit)
- Patterns detected: {list or "none"}
- Skills evolved: {list or "none"}
- Trend: {improving | stable | declining}
```

Update pipeline state: push `{"phase": "evolve", "status": "complete"}` to `phase_history`.

**Orbit + Evolve complete. The next session starts smarter.**

---

## Red Flags
- Starting without user mode selection
- Proceeding past spec without explicit approval
- Continuing after 3 check failures without user consent
- Skipping the isolated integration test before PR
- Shipping with FAIL in security-related check items
- Losing the check report between phases (always store in pipeline state)
- Creating a PR without the check report in the body
