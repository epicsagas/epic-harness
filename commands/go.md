---
description: "Build it — auto-plan, delegate to subagents with TDD, and verify. The main execution engine."
---

# /go — Build It

You are starting the **Go** phase — the core execution engine of epic-harness.

## Process

### Step 0: Preflight
- Run `HARNESS_DIR=$(epic-harness path)` to resolve the data directory.
- Check if a spec exists (`$HARNESS_DIR/specs/` or recent conversation). If not, run a quick inline spec conversation first.
- Check if `$HARNESS_DIR/team/` exists — if yes, use project-specific agents.

### Step 1: Plan
Break the work into ordered tasks:
```
Task 1: [description] — depends on: none — modifies: [file list]
Task 2: [description] — depends on: Task 1 — modifies: [file list]
Task 3: [description] — depends on: none (parallel with 1) — modifies: [file list]
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
- The task description
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

### Step 3: Integrate
After all tasks complete:
- Run the full test suite
- Check for integration issues between tasks
- If anything fails, dispatch a subagent to fix it

### Step 4: Report
Summarize what was built, what tests pass, and any remaining issues.

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
