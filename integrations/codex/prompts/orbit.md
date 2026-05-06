---
description: "Complete orbit — autonomous spec through ship. Choose interactive or council mode, then hands-off until PR."
---

# /orbit — Complete Orbit

**CRITICAL**: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

You are entering **Orbit** mode — the full autonomous pipeline from spec to PR in one shot.

## Step 0: Preflight

Initialize pipeline state at `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json`:
```json
{"id":"{timestamp}","mode":null,"phase":"mode_select","status":"running","spec_file":null,"goal_slug":null,"branch":null,"check_fail_count":0,"max_retries":3,"check_report":null,"started_at":"{ISO}","updated_at":"{ISO}","phase_history":[]}
```

## Step 1: Mode Selection

Ask the user:
> **1. Interactive discover** — You run `/discover` and `/spec` yourself, then say "orbit go".
> **2. Council auto-spec** — 4-voice council analyzes your request, generates a spec, you approve.

Wait for choice. Record in pipeline state.

## Step 2A: Interactive Mode

Tell user to run `/discover` → `/spec`, then say "orbit go". STOP and wait.
On resume: load latest `SPEC-*.md` with `status: approved`. Proceed to Step 3.

## Step 2B: Council Auto-Spec

1. Gather the user's request (ask if not stated)
2. Launch 4 parallel Codex sub-agents (Architect, Skeptic, Pragmatist, Critic) — each receives ONLY the request + codebase context, NOT the full conversation (anti-anchoring)
3. Synthesize: list agreement/disagreement, produce recommended approach
4. Generate spec at `$HARNESS_DIR/specs/SPEC-{timestamp}.md` with `status: pending`
5. Present to user: **Approve / Modify / Reject**
6. On approve: set `status: approved`, record via `mem_add` (type=decision, importance=0.9). Proceed.

## Step 3: Build (Go)

1. Load spec, extract `goal_slug`, create branch `feature/{goal_slug}`
2. Plan tasks from Requirements (R1, R2...)
3. Execute with Codex sub-agents — TDD, debug on failure, verify before done
4. Handle states: DONE / DONE_WITH_CONCERNS / NEEDS_CONTEXT / BLOCKED
5. Integrate: full test suite, verify ACs

## Step 4: Check

1. Gather scope via `git diff --stat`
2. Classify changed files (API, Frontend, DB, Backend, Tests, Infra)
3. Launch parallel Codex sub-agents: Reviewer, Auditor, Test runner (+ scope-specific)
4. Synthesize Check Report: Quality/Security/Performance PASS/WARN/FAIL + Spec Coverage
5. **PRESERVE check report** in pipeline state `check_report` field

## Step 5: Verdict

- **All PASS + all AC verified** → proceed to Ship
- **WARN** → log, auto-proceed
- **FAIL** → increment `check_fail_count`:
  - `< 3`: plan fixes from action items, execute, return to Step 4
  - `≥ 3`: **PAUSE** — ask user "continue or abort?"

## Step 6: Ship

1. **Gate**: verify PASS check report exists
2. **Isolated test**: launch Codex sub-agent with worktree isolation — build + test + lint
3. **Git hygiene**: conventional commits, rebase, squash fixups
4. **Create PR** via `gh pr create` with spec + check report in body
5. **CI watch** via `gh pr checks --watch`, auto-fix failures

## Step 7: Report

```
## Orbit Complete
- Pipeline: PIPELINE-{id}
- Mode: {interactive|council}
- Spec: SPEC-{timestamp} ({goal_slug})
- PR: {URL}
- Check retries: {count}

### Phase Summary
| Phase | Status | Retries |
|-------|--------|---------|
| Spec | approved | 0 |
| Go | complete | 0 |
| Check | PASS | {count} |
| Ship | complete | 0 |
```

"One orbit complete. Run `/evolve` to analyze observations."

## Red Flags
- Starting without user mode selection
- Proceeding without spec approval
- Continuing after 3 check failures without user consent
- Skipping isolated integration test
- Shipping with FAIL in security checks
- Losing check report between phases
