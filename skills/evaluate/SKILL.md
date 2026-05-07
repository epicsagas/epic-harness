---
name: evaluate
description: "Evaluate how effectively the user leverages AI as a thinking amplifier — scores 5 dimensions with evidence from obs/evolution/memory data."
trigger: "/evaluate"
---

# AI Usage Evaluation — Thinking Amplifier Assessment

## Process

### 1. Collect Data
Run the data collection script to gather evaluation context:
```bash
bash "$(dirname "$0")/evaluate.sh"
```

Then retrieve historical context from harness-mem:
```
mem_recall(hint: "evaluation patterns decisions errors project context")
```

### 2. Score 5 Dimensions

Evaluate each dimension on a 1-10 scale. Every score MUST cite specific evidence from the collected data.

#### D1: Thought Amplification (사고증폭)
**Question**: Is the user using AI to explore ideas they couldn't reach alone, or just delegating mechanical work?

Evidence to examine:
- Variety of file extensions in obs (more diverse = broader exploration)
- Ratio of Edit/Write (doing) vs Read/Grep (thinking) tool calls
- Memory nodes of type "decision" and "concept" (vs only "error" and "session")
- Whether prompts are getting more specific over time (check session score_history progression)

Score guide:
- 1-3: AI is a code typist. User asks "fix X", AI types.
- 4-6: Some exploration. User asks "what should I do about X?"
- 7-8: Genuine dialogue. User and AI challenge each other's assumptions.
- 9-10: AI extends user's thinking into territories they didn't anticipate.

#### D2: Self-Improvement (자기개선)
**Question**: Is the Evolve loop producing genuine improvement or just generating presets?

Evidence to examine:
- `evolved_skills_summary.by_type` — ratio of preset vs evolved vs auto-evolved
- `skill_attribution` deltas — are evolved skills measurably better than baseline?
- `patterns_detected` vs `total_entries` — what % of evolution runs found real patterns?
- `stagnation_count` — has learning plateaued?
- Score trend direction (improving/stable/declining)

Score guide:
- 1-3: All skills are presets. Evolution generates 0 patterns.
- 4-6: Some evolved skills exist but delta ≈ 0 or negative.
- 7-8: Evolved skills show positive delta. Patterns detected regularly.
- 9-10: Clear score improvement trend. Skills actively prevent known failure modes.

#### D3: Metacognitive Expansion (메타인지확장)
**Question**: Is the user reflecting on their own thinking patterns and learning from AI feedback?

Evidence to examine:
- Memory nodes: count of "decision" and "pattern" types (indicates reflection)
- Whether the user has corrected AI's approach (feedback memories)
- Long_debug_loop detections (lower = better self-awareness to avoid loops)
- Whether evolved skills include user-driven corrections (not just auto-generated)

Score guide:
- 1-3: No feedback memories. No decisions recorded. AI runs in a vacuum.
- 4-6: Some decisions recorded. User occasionally corrects direction.
- 7-8: Regular feedback. User adjusts approach based on AI observations.
- 9-10: User and AI co-evolve. Patterns from one session improve the next.

#### D4: Prompt Quality (프롬프트개선)
**Question**: Are prompts becoming more precise, specific, and effective over time?

Evidence to examine:
- `output_quality` trend across sessions (improving = prompts getting better)
- `tool_success` rate (high = clear instructions, low = ambiguous prompts)
- Ratio of targeted-read patterns (Grep→Read) vs whole-file reads
- Number of NEEDS_CONTEXT or BLOCKED states in recent sessions

Score guide:
- 1-3: Vague prompts ("fix this", "make it work"). Low tool_success.
- 4-6: Some specificity. User gives file paths and error messages.
- 7-8: Precise context. User specifies what they want and why.
- 9-10: Surgical prompts. User provides exact location, expected behavior, constraints.

#### D5: Execution Efficiency (실행효율)
**Question**: Is the AI-user collaboration becoming faster and more accurate over time?

Evidence to examine:
- `execution_cost` average (should be high = efficient tool use)
- `tool_success` rate trend
- Observations per session (too many = thrashing, too few = underutilizing)
- Score trend first 5 vs last 5 sessions

Score guide:
- 1-3: Declining scores. More tool calls per task. Repeated failures.
- 4-6: Stable scores. Reasonable tool usage. Some waste.
- 7-8: Improving scores. Efficient tool selection. Minimal rework.
- 9-10: Consistently high scores. Few retries. Tasks completed in fewer rounds.

### 3. Brutally Honest Summary (냉정한 총평)

Write 3-5 sentences that specifically counter the natural tendency to rate everything positively:
- Compare scores to what "genuine thought amplification" looks like, not to "better than nothing"
- Call out specific numbers that contradict high scores
- Identify the biggest gap between current state and ideal
- Name the ONE thing that would make the biggest difference if improved

### 4. Action Items (minimum 3)

Each action item must be:
- **Specific**: Not "use AI better" but "when starting a task, spend 2 minutes framing the problem before asking for implementation"
- **Measurable**: Include a metric to track improvement
- **Achievable**: Within the user's control

### 5. Output Format

```markdown
# AI Usage Evaluation Report
Date: {timestamp}

## Scores

| Dimension | Score | Trend |
|-----------|-------|-------|
| D1: Thought Amplification | {1-10} | {↑/→/↓} |
| D2: Self-Improvement | {1-10} | {↑/→/↓} |
| D3: Metacognitive Expansion | {1-10} | {↑/→/↓} |
| D4: Prompt Quality | {1-10} | {↑/→/↓} |
| D5: Execution Efficiency | {1-10} | {↑/→/↓} |
| **Composite** | **{avg}** | |

## Evidence
{cite specific numbers for each dimension}

## Brutally Honest Summary
{3-5 sentences, no hedging}

## Action Items
1. [specific, measurable action]
2. [specific, measurable action]
3. [specific, measurable action]
```

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|-------------------|
| "AI is helping a lot" | Helping with WHAT? Typing code is not thinking amplification. | Check if AI has ever changed your mind about an approach. |
| "Scores are improving" | Scores measure tool success, not thinking quality. A perfect tool_success score with zero decisions recorded means the AI is a typist, not a partner. | Look at decision/concept memory nodes, not just composite scores. |
| "I use AI every day" | Frequency ≠ depth. 93 sessions of "fix this bug" is less valuable than 5 sessions of genuine architectural exploration. | Measure variety of task types, not session count. |
| "Evolve is working" | Most evolved skills are cold-start presets, not genuine learning. Check the ratio of preset vs evolved skills. | Count how many skills have positive delta AND evolved (not preset) type. |
| "My prompts are fine" | Compare your first session's output_quality with your latest. If they're similar, you haven't improved your prompts. | Track output_quality trend across sessions. |
| "This score seems harsh" | Good. Comfortable scores mean the evaluation isn't doing its job. | Re-read the score guide for each dimension and match honestly. |

## Evidence Required

- [ ] evaluate.sh output cited (total observations, success rates, by-tool breakdown)
- [ ] evolution.jsonl patterns referenced (patterns detected, skills generated)
- [ ] At least 1 harness-mem memory node cited via mem_recall
- [ ] Score trend over time shown (first 5 vs last 5 sessions)
- [ ] Evolved skills inventory referenced (preset vs evolved ratio)
- [ ] Each dimension score justified with a specific number
- [ ] Brutally honest summary does NOT use the word "good" or "great"

## Red Flags

- Giving all dimensions 7+ without citing contradictory evidence
- Ignoring downward trends in score_history
- Not citing specific observation counts or tool success rates
- Vague action items ("communicate better", "use AI more effectively")
- Skipping the brutally honest summary
- Writing a summary that could apply to any user (generic praise)
- Omitting harness-mem recall results
