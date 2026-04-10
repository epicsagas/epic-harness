---
name: go
description: "Build it — auto-plan, delegate to agents with TDD, and verify. The main execution engine."
command: /go
---

# /go — Build It

You are starting the **Go** phase — the core execution engine of epic-harness.

## Process

### Step 0: Preflight

- Check if a spec exists (`.harness/specs/` or recent conversation). If not, run a quick inline spec conversation first.
- Check if `.harness/team/` exists — if yes, use project-specific agents.

### Step 1: Plan

Break the work into ordered tasks:

```
Task 1: [description] — depends on: none
Task 2: [description] — depends on: Task 1
Task 3: [description] — depends on: none (parallel with 1)
```

Show the plan. Get user confirmation (or auto-proceed if user said "just do it").

### Step 2: Execute — USE ANTIGRAVITY MANAGER VIEW

**Open the Manager view** to launch parallel agents for independent tasks.

For each independent task batch, create agents in Manager view:
- Each agent receives: the task description + TDD instruction + verify instruction
- Independent tasks run in parallel across Manager view agents
- Dependent tasks wait for their prerequisites to complete

Each agent must:
- Follow TDD: write failing test → implement → green
- Invoke the debug skill if tests fail
- Invoke the verify skill before reporting done

Sequential tasks that depend on prior results run after their prerequisites are confirmed complete.

### Step 3: Integrate

After all tasks complete:
- Run the full test suite
- Check for integration issues between tasks
- If anything fails, dispatch a new agent in Manager view to fix it

### Step 4: Report

Summarize what was built, what tests pass, and any remaining issues.

## Skills Auto-Triggered

- **tdd**: Every agent follows red-green-refactor
- **debug**: On any test failure or error
- **verify**: Before marking any task complete
- **simplify**: If any file exceeds 200 lines

## Red Flags

- Implementing without a plan
- Skipping tests "to save time"
- Not verifying the full suite after integration
- Implementing everything in a single agent without using Manager view for parallelism
