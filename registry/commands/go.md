---
description: "Go phase. Run after /spec. Reads the approved SPEC file, maps Requirements to tasks, executes via TDD subagents (parallel where safe, worktree-isolated when files overlap), integrates results, and verifies all Acceptance Criteria before handing off to /check."
---

# /go — Build It

You are starting the **Go** phase — the core execution engine of epic-harness.

## Process

### Step 0: Preflight

Run `HARNESS_DIR=$(epic-harness path)` to resolve the data directory.

**Load the spec:**
1. Find the latest approved spec: `ls -t $HARNESS_DIR/specs/SPEC-*.md | head -1`
2. Read the file. Confirm its frontmatter has `status: approved`. If not, tell the user to run `/spec` first and stop.
3. Extract the `goal_slug` from frontmatter — use it as the git branch name: `feature/{goal_slug}`
4. Extract **Requirements** (R1, R2, ...) and **Acceptance Criteria** (AC1, AC2, ...) — these drive the Task list in Step 1.

If no spec file exists and no spec is visible in the conversation, run a quick inline spec conversation first.

Check if `$HARNESS_DIR/team/` exists — if yes, use project-specific agents.

**Create the feature branch:**
```bash
git checkout -b feature/{goal_slug}
```

**Initialize orchestration (if EPIC_ORCHESTRATION enabled):**
If `EPIC_ORCHESTRATION=enabled` is set in environment:
1. Run: `HARNESS_DIR=$(epic-harness path)`
2. Initialize orchestration run:
   ```bash
   mkdir -p "$HARNESS_DIR/orchestrator/agents"
   ```
3. Create `run.json` with the task plan (agents, dependencies)
4. Create `agents/{id}/task.json` for each planned subagent

### Step 1: Plan

Map each Requirement → one or more Tasks. Every task must reference its source requirement:

```
Task 1: [description] — satisfies: R1 — depends on: none — modifies: [file list]
Task 2: [description] — satisfies: R2 — depends on: Task 1 — modifies: [file list]
Task 3: [description] — satisfies: R1, R2 (integration test) — depends on: Task 1, 2 — modifies: [file list]
```

**Conflict Analysis:**
- Identify which tasks modify the same files
- If parallel tasks share files → either:
  - **Option A:** Make them dependent (serialize execution)
  - **Option B:** Use `isolation: "worktree"` for safe parallel execution
- If tasks modify different files → safe to run in parallel without isolation

Show the plan with conflict analysis. Get user confirmation (or auto-proceed if user said "just do it").

### Step 2: Execute

For each task, launch a subagent (Agent tool) with:
- The task description and which Requirement(s) it satisfies
- Instruction to follow TDD: write test first → implement → green
- Instruction to invoke `debug` skill if tests fail
- Instruction to invoke `verify` skill before reporting done
- `run_in_background: true` for independent tasks (parallel execution)
- `isolation: "worktree"` **if and only if:**
  - This task runs in parallel with another task, AND
  - Both tasks modify overlapping files

**Isolation Decision Matrix:**
| Scenario | Parallel? | Same Files? | Isolation? |
|----------|-----------|-------------|------------|
| Task A, B sequential | No | Any | ❌ No |
| Task A, B parallel | Yes | No overlap | ❌ No |
| Task A, B parallel | Yes | Overlap exists | ✅ Yes |

**Orchestration-aware execution:**
When `EPIC_ORCHESTRATION=enabled`:
- Before launching each subagent, write its task definition to `$HARNESS_DIR/orchestrator/agents/{id}/task.json`
- The `orchestrate` Rust hook will automatically track status updates via Pre/Post Agent tool invocations
- Subagent output format should include structured state for the hook to parse
- After all subagents complete, read `$HARNESS_DIR/orchestrator/run.json` to get final orchestration status

### Subagent Result States (Step 2.5)

Every subagent must report one of 4 states on completion:

| State | Meaning | Follow-up |
|-------|---------|-----------|
| **DONE** | Task completed, all tests pass, requirements met | Proceed to next task |
| **DONE_WITH_CONCERNS** | Task completed but has warnings/notes | Review concerns. If non-blocking, proceed. If architectural, escalate. |
| **NEEDS_CONTEXT** | Cannot proceed without user input or missing information | Prompt user with specific questions. Resume after answer. |
| **BLOCKED** | External dependency failed or unresolvable error | Log blocker. Attempt alternative approach. If impossible, report in Step 4. |

**Handling rules:**
- DONE: Merge immediately, no additional review needed.
- DONE_WITH_CONCERNS: Main agent reviews concerns within 30 seconds. Auto-escalate if concerns mention: security, data loss, breaking changes.
- NEEDS_CONTEXT: Formulate exactly what information is needed. Present as numbered options when possible. Do NOT guess or assume.
- BLOCKED: Try one alternative approach. If still blocked, skip task and report in Step 4 with blocker details.

**Subagent output format:**
```
## Status: [DONE|DONE_WITH_CONCERNS|NEEDS_CONTEXT|BLOCKED]
## Summary: [1-2 sentences]
## Evidence: [test output, file changes, or specific observations]
## Concerns: [only for DONE_WITH_CONCERNS — list specific issues]
## Questions: [only for NEEDS_CONTEXT — specific numbered questions]
## Blocker: [only for BLOCKED — what failed and what was tried]
```

### Step 3: Integrate

After all tasks complete:
- Categorize each task result by its state (DONE/DONE_WITH_CONCERNS/NEEDS_CONTEXT/BLOCKED)
- For NEEDS_CONTEXT tasks: resolve before integration
- For BLOCKED tasks: attempt alternatives, exclude from "satisfied" count in report
- Run the full test suite
- Verify each Acceptance Criterion (AC1, AC2, ...) is demonstrably met
- If anything fails, dispatch a subagent to fix it

**Orchestration integration check:**
If `EPIC_ORCHESTRATION=enabled`:
1. Read `$HARNESS_DIR/orchestrator/run.json` for final status
2. If any agents are in BLOCKED or FAILED state, include in the report
3. If run status is "aborted" (user cancelled), note this in the report

### Step 4: Report

```
## Go Report
- Spec: SPEC-{timestamp} ({goal_slug})
- Branch: feature/{goal_slug}
- Requirements satisfied: R1 ✅, R2 ✅, ...
- Acceptance criteria verified: AC1 ✅, AC2 ✅, ...
- Tests: X/Y passing
- Subagent states: X DONE, Y CONCERNS, Z BLOCKED
- Concerns resolved: [list] / [unresolved list]
- Remaining issues: none / [list]
- Orchestration: enabled/disabled
- Agent states: [from orchestrator state]
```

Tell the user: **"Build complete. Run `/check` to verify before shipping."**

## Output

- Git branch `feature/{goal_slug}` with all changes committed
- Conventional Commits: `feat:`, `fix:`, `test:` prefixes
- All Requirements satisfied and Acceptance Criteria verified

## Skills Auto-Triggered
- **tdd**: Every subagent follows red-green-refactor
- **debug**: On any test failure or error
- **verify**: Before marking any task complete
- **simplify**: If any file exceeds 200 lines

## Red Flags
- Implementing without a plan
- Skipping tests "to save time"
- Not verifying the full suite after integration
- Implementing everything in a single file
- Starting without a `status: approved` spec
