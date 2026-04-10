---
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
```

### `/evolve rollback` — Undo last evolution
1. If `.harness/evolved_backup/` exists, restore it to `.harness/evolved/`
2. Append a rollback record to evolution.jsonl
3. Report what was rolled back

## Red Flags
- Evolving after only 1-2 observations (not enough data)
- Keeping evolved skills that never trigger
- Ignoring stagnation warnings
