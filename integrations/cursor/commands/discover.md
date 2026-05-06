---
description: "Explore and define the problem before specifying a solution"
---

# /discover — Problem Discovery

**CRITICAL**: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

You are starting the **Discover** phase. Your job is to help the user articulate what problem they are actually trying to solve.

## Process

1. **Listen** — Repeat the user's request back. Categorize it:
   - Solution without problem → use **5 Whys** (drill to root cause)
   - Feature without context → use **JTBD** (extract job story)
   - Systemic complaint → use **Fishbone** (map causes across categories)
   - Vague or contradictory → use **Socratic questioning** (clarify)
   - Has a vision but no path → use **"What does done look like?"**

2. **Probe** — Ask max 3 questions per round, max 3 rounds. Each round should narrow the space. If the user can't answer, proceed with what you have.

3. **Frame** — Write a structured problem statement:
   > **[Who]** experiences **[observable problem]** when **[trigger condition]**, resulting in **[quantified impact]**. The desired state is **[measurable outcome]**.

   Capture: root cause or job story, constraints, assumptions, out-of-scope.
   Show the frame. Ask: "Does this capture the problem accurately?"

4. **Save** — Once confirmed, save to `$HARNESS_DIR/specs/PROBLEM-{timestamp}.md`:

```markdown
---
status: framed
created: {ISO-8601 timestamp}
context: {one-line summary}
---

# Problem: {title}

## Problem Statement
{Structured problem statement}

## Root Cause / Job Story
{5 Whys chain or JTBD job story}

## Constraints
- Timeline: {when needed}
- Technology: {stack constraints}
- Scale: {expected load}
- Compatibility: {backward-compat needs}

## Assumptions
- {assumption} — {certain / uncertain}

## Out of Scope
- {explicitly excluded}
```

5. **Transition** — "Problem defined. Run `/spec` to turn this into a buildable specification."

## Red Flags
- Jumping to solutions during discovery
- More than 3 questions per round
- More than 3 rounds without framing
- Problem statement not confirmed by user
- Framing so broadly it could mean anything
