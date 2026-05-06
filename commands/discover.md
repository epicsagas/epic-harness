---
description: "Explore and define the problem before specifying a solution"
---

# /discover — Problem Discovery

You are starting the **Discover** phase. Your job is to help the user articulate what problem they are actually trying to solve, before jumping to solutions or specs.

**CRITICAL**: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

## Process

### Step 0: Prerequisites

- Resolve harness directory: `HARNESS_DIR=$(epic-harness path)`
- Read any existing context (CLAUDE.md, README, codebase structure)
- Check for existing problem statements in `$HARNESS_DIR/specs/PROBLEM-*.md`

### Step 1: Listen

Read the user's request carefully. Repeat it back in your own words and ask:
- "Is that the core of it, or is there more?"

Categorize the request:
- **Solution without problem**: User names a technology/approach ("Add Redis", "Rewrite in Go")
- **Feature without context**: User describes output ("Build a dashboard")
- **Systemic complaint**: Broad negative ("Everything is slow", "It's all broken")
- **Vague ambition**: Goal without boundaries ("Make it better", "Modernize")
- **Clear problem**: Observable gap stated (skip to Step 3)

### Step 2: Probe

Select the right technique based on the category. Ask **max 3 questions per round**, run **max 3 rounds**. If the user can't answer, proceed to Frame with available information.

| User signal | Technique | Core question |
|---|---|---|
| Names a solution ("Add Redis") | **5 Whys** | "What's happening that makes you need this?" → repeat |
| Describes a feature without why | **JTBD** | "What situation makes you need this? What would 'done' look like?" |
| "Everything is broken" | **Fishbone** | "Which area: People / Process / Technology / Data / Environment?" |
| Vague or contradictory | **Socratic** | "What specifically do you mean by 'X'?" |
| Has a vision but no path | **Done looks like** | "When this works perfectly, what do you see?" |
| Uncertain assumptions | **Assumption map** | "What must be true for this to work?" |

Each round should narrow the space. If after 2 rounds you have enough to frame, don't force a third.

### Step 3: Frame

Synthesize into a structured problem statement:

> **[Who]** experiences **[observable problem]** when **[trigger condition]**, resulting in **[quantified impact]**. The desired state is **[measurable outcome]**.

Capture supporting context:
- **Root cause** or **Job story** (from probing)
- **Constraints**: timeline, technology, scale, compatibility
- **Assumptions**: what must be true, flagged as certain or uncertain
- **Out of scope**: explicitly excluded items

Show the frame to the user. Ask: "Does this capture the problem accurately?"

### Step 4: Save

Once confirmed, save the problem statement:

```bash
mkdir -p "$HARNESS_DIR/specs"
```

Write to `$HARNESS_DIR/specs/PROBLEM-{timestamp}.md`:

```markdown
---
status: framed
created: {ISO-8601 timestamp}
context: {one-line summary}
---

# Problem: {title}

## Problem Statement
{Who} experiences {observable problem} when {trigger condition}, resulting in {quantified impact}. The desired state is {measurable outcome}.

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

### Step 5: Transition

Tell the user: **"Problem defined. Run `/spec` to turn this into a buildable specification."**

If the user realizes there are multiple problems during probing, address them one at a time. Each problem gets its own file.

## Output

A saved `PROBLEM-{timestamp}.md` in `$HARNESS_DIR/specs/` with status `framed`.

## Red Flags

- Jumping to solutions or code during the discovery conversation
- Asking more than 3 questions per round
- More than 3 rounds without attempting a frame
- Producing a problem statement the user didn't explicitly confirm
- Framing so broadly it could mean anything ("improve the system")
- Skipping discovery for non-trivial, ambiguous work
- Confusing symptoms with root causes
