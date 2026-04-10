---
name: evolve
description: "Trigger skill evolution manually — analyze observations, evolve skills, show status, or rollback"
---

# /evolve — Manual Evolution Trigger

You are the **Evolution Engine** — analyze past sessions to improve skills.

## Sub-commands

### `/evolve` (default) — Run evolution now
1. Read observation logs from `.harness/obs/`
2. Analyze failure patterns across all sessions
3. Identify weak areas (error types, recurring failures)
4. Generate or improve evolved skills in `.harness/evolved/`
5. Gate: validate new skills (format, dedup, cap of 10)
6. Report what changed

Alternatively, run directly:
```bash
epic-harness reflect
```

### `/evolve status` — Show evolution dashboard

Read `.harness/metrics.json` and `.harness/evolution.jsonl`, then display:

```
## Evolution Dashboard

### Overview
- Sessions analyzed: {total_sessions}
- Average success rate: {avg_success_rate}%
- Best score: {best_score} (session: {best_session})
- Trend: {trend} ({score_history.length} data points)
- Stagnation count: {stagnation_count} / 3 (rollback at 3)

### Score History (last 5 sessions)
| Session | Success Rate | Avg Score | Observations | Tool Success | Output Quality |
|---------|-------------|-----------|--------------|-------------|---------------|
(read score_history array, show dimension_averages for each)

### Evolved Skills
(list .harness/evolved/*/SKILL.md with name and description from frontmatter)

### Last Session Analysis
(read last entry from evolution.jsonl)
- Error patterns: {error_patterns}
- Failure patterns: {failure_patterns[].pattern_type}
- Skills seeded: {skills_seeded}
- Skills rolled back: {skills_rolled_back}
- Analysis: {analysis_summary}
```

### `/evolve history` — Long-term analysis

Read `.harness/evolution.jsonl` (full history), then display:

```
## Evolution History

### Trend Over Time
| Session # | Date | Success Rate | Avg Score | Skills | Patterns |
|-----------|------|-------------|-----------|--------|----------|
(each row = one evolution.jsonl entry)

### Cumulative Pattern Frequency
| Pattern | Total Count | First Seen | Last Seen |
|---------|-------------|------------|-----------|

### Skill Effectiveness
Read `.harness/metrics.json` → `skill_attribution`, then display:
| Skill | Sessions Active | Avg Score With | Avg Score Without | Delta |
|-------|----------------|----------------|-------------------|-------|
(positive delta = effective, negative = consider removing)

### Dispatch Analysis
Read `.harness/dispatch/dispatch_*.jsonl`, then display:
| Skill | Times Invoked | Top Trigger Signals |
|-------|--------------|---------------------|
```

### `/evolve cross-project` — Cross-project patterns

Read `~/.harness-global/patterns.jsonl`, then display:
```
## Cross-Project Patterns

### Weak Tools Across Projects
| Tool | Projects Affected | Frequency |
|------|-------------------|-----------|

### Common Error Patterns
| Error Type | Projects | Total Occurrences |
|------------|----------|-------------------|
```

To opt-in: create `.harness/.cross-project-enabled` file.
To opt-out: remove it.

### `/evolve rollback` — Undo last evolution
1. If `.harness/evolved_backup/` exists, restore it to `.harness/evolved/`
2. Otherwise, read `.harness/evolution.jsonl` for last entry, remove skills seeded in that entry
3. Append a rollback record to evolution.jsonl
4. Report what was rolled back

### `/evolve reset` — Clear all evolution data
1. Remove `.harness/evolved/`, `.harness/evolved_backup/`
2. Clear `metrics.json` and `evolution.jsonl`
3. Confirm with user first

## How Evolution Works

The evolution loop runs automatically at session end (via the `SessionEnd` hook calling `epic-harness reflect`).
This command lets you trigger it manually or inspect the state.

```
Observe (PostToolUse — multi-dimensional scoring)
    ↓ .harness/obs/session_YYYYMMDD_{pid}_{rand}.jsonl
Analyze (SessionEnd or /evolve)
    ↓ SessionAnalysis: per-tool, per-ext, score distribution
    ↓ Pattern detection: repeated_same_error, fix_then_break, long_debug_loop, thrashing
Seed (auto-generate targeted skills)
    ↓ 4 seeding paths: pattern / weak tool / weak file type / high-freq error
Gate (validate: format, dedup, cap of 10)
    ↓ Stagnation check: 3 sessions no improvement → rollback to best checkpoint
Reload (next session context loads evolved skills via harness-context.mdc rule)
```

## Scoring System
- **Composite**: `0.5 × tool_success + 0.3 × output_quality + 0.2 × execution_cost`
- **Failure classification**: 9 categories (type_error, syntax_error, test_fail, lint_fail, build_fail, permission_denied, timeout, not_found, runtime_error)
- **Pattern detection**: 4 types (repeated_same_error, fix_then_break, long_debug_loop, thrashing)

## Red Flags
- Evolving after only 1-2 observations (not enough data)
- Keeping evolved skills that never trigger
- Not reviewing evolved skills periodically
- Ignoring stagnation warnings
