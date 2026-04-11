---
name: go
description: "Build it — auto-plan, delegate to sub-agents with TDD, and verify. The main execution engine."
---

# /go — Build It

You are starting the **Go** phase — the core execution engine of epic-harness.

## Process

### Step 0: Preflight
- Check if a spec exists (`$(epic-harness path)/specs/` or recent conversation). If not, run a quick inline spec conversation first.
- Check if `$(epic-harness path)/team/` exists — if yes, use project-specific agents.

### Step 1: Plan
Break the work into ordered tasks:
```
Task 1: [description] — depends on: none
Task 2: [description] — depends on: Task 1
Task 3: [description] — depends on: none (parallel with 1)
```
Show the plan. Get user confirmation (or auto-proceed if user said "just do it").

### Step 2: Execute

For each task, use a **Cursor sub-agent** (available in Cursor 1.7+):
- Open a new Composer session as a sub-agent with the task description
- Instruct it to follow TDD: write test first → implement → green
- Instruct it to apply the `debug` skill if tests fail
- Instruct it to apply the `verify` skill before reporting done

**If Cursor sub-agents are not available**, fall back to sequential execution:
- Execute each task in dependency order within this session
- Clearly mark task boundaries in your output (e.g., `--- Task 1 complete ---`)
- Apply TDD, debug, and verify skills inline

**Parallel execution**: For independent tasks with no dependencies, launch sub-agents concurrently when Cursor supports it. Otherwise, execute in the order that unblocks dependent tasks earliest.

### Step 3: Integrate
After all tasks complete:
- Run the full test suite
- Check for integration issues between tasks
- If anything fails, dispatch a sub-agent (or inline fix) to resolve it

### Step 4: Report
Summarize what was built, what tests pass, and any remaining issues.

## Skills Auto-Triggered
- **tdd**: Every task follows red-green-refactor
- **debug**: On any test failure or error
- **verify**: Before marking any task complete
- **simplify**: If any file exceeds 200 lines

## Red Flags
- Implementing without a plan
- Skipping tests "to save time"
- Not verifying the full suite after integration
- Implementing everything in a single file
