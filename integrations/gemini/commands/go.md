---
description: "Build it — plan, execute tasks sequentially with TDD, and verify. The main execution engine."
---

# /go — Build It

You are starting the **Go** phase — the core execution engine of epic-harness.

> **Gemini CLI note**: Gemini CLI does not support parallel agent spawning. All tasks execute
> sequentially, one at a time. Follow TDD for each task before moving to the next.

## Process

### Step 0: Preflight
- Check if a spec exists (`.harness/specs/` or recent conversation). If not, run a quick inline spec conversation first.
- Check if `.harness/team/` exists — if yes, use project-specific agents.

### Step 1: Plan
Break the work into ordered tasks:
```
Task 1: [description] — depends on: none
Task 2: [description] — depends on: Task 1
Task 3: [description] — depends on: Task 2
```
Show the plan. Get user confirmation (or auto-proceed if user said "just do it").

### Step 2: Execute (sequential)
For each task, execute one at a time following this cycle:
- Write the failing test first (Red)
- Implement the minimum code to pass (Green)
- Refactor if needed (Refactor)
- Invoke `verify` skill before marking the task done
- Invoke `debug` skill if tests fail

Complete each task fully before starting the next.

### Step 3: Integrate
After all tasks complete:
- Run the full test suite
- Check for integration issues between tasks
- Fix any failures before proceeding

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
- Starting the next task before the current one is verified
- Not running the full suite after integration
