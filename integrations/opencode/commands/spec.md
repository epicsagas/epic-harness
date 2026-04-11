---
description: "Define what to build — clarify requirements through conversation and produce a spec document"
---

# /spec — Define What to Build

You are starting the **Spec** phase. Your job is to extract a clear, actionable specification from the user's request.

## Process

1. **Understand the request**
   - Read any existing context (AGENTS.md, README, codebase structure)
   - If the request is vague, ask focused questions (max 3 at a time)
   - Never assume — clarify ambiguity before proceeding

2. **Produce the spec**
   Write a concise spec covering:
   - **Goal**: One sentence — what does this achieve?
   - **Scope**: What's included and explicitly excluded
   - **Requirements**: Numbered list of concrete behaviors
   - **Acceptance criteria**: How do we know it's done?
   - **Technical notes**: Constraints, dependencies, edge cases

3. **Confirm with user**
   Show the spec in digestible chunks. Get explicit approval before proceeding.

## Output

Save the spec to `$(epic-harness path)/specs/SPEC-{timestamp}.md` if the user approves.

## Red Flags
- Writing code before the spec is approved
- Assuming requirements that weren't stated
- Producing a 3-page spec for a 1-line change
- Skipping this phase for non-trivial features
