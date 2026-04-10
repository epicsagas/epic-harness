---
name: evolve
description: "Trigger skill evolution — run epic-harness reflect, show metrics, or rollback"
command: /evolve
---

# /evolve — Evolution Engine

You are the **Evolution Engine** — analyze past sessions to improve skills.

## Sub-commands

### `/evolve` (default) — Run evolution now

Run in terminal:

```bash
epic-harness reflect
```

Then display the resulting metrics from `.harness/metrics.json` and `.harness/evolution.jsonl`.

### `/evolve status` — Show evolution dashboard

Read `.harness/metrics.json` and `.harness/evolution.jsonl`, then display:

```
## Evolution Dashboard

### Overview
- Sessions analyzed: {total_sessions}
- Average success rate: {avg_success_rate}%
- Best score: {best_score}
- Trend: {trend}
- Stagnation count: {stagnation_count} / 3 (rollback at 3)

### Evolved Skills
(list .harness/evolved/*/SKILL.md with name and description)

### Last Session Analysis
- Error patterns: {error_patterns}
- Failure patterns: {failure_patterns}
- Skills seeded: {skills_seeded}
```

### `/evolve history` — Long-term analysis

Read `.harness/evolution.jsonl` (full history) and display:

```
## Evolution History

| Session # | Date | Success Rate | Avg Score | Skills | Patterns |
|-----------|------|-------------|-----------|--------|----------|
```

### `/evolve rollback` — Undo last evolution

1. If `.harness/evolved_backup/` exists, restore it to `.harness/evolved/`
2. Otherwise, read `.harness/evolution.jsonl` for last entry, remove skills seeded in that entry
3. Report what was rolled back

### `/evolve reset` — Clear all evolution data

1. Remove `.harness/evolved/`, `.harness/evolved_backup/`
2. Clear `metrics.json` and `evolution.jsonl`
3. Confirm with user first

## How Evolution Works

Note: Antigravity has no hooks system. The observe/reflect cycle runs via explicit terminal commands:

- `epic-harness resume` — session start (loads context, reports metrics)
- `epic-harness reflect` — session end (analyzes observations, evolves skills)

```
epic-harness resume (session start)
    ↓ loads .harness/memory/, evolved skills, metrics
Manual work session
    ↓ observations accumulated in .harness/obs/
epic-harness reflect (session end or /evolve)
    ↓ SessionAnalysis: per-tool, per-ext, score distribution
    ↓ Pattern detection + skill seeding
    ↓ Gate: validate, dedup, cap of 10
```

## Red Flags

- Evolving after only 1-2 observations (not enough data)
- Keeping evolved skills that never trigger
- Not reviewing evolved skills periodically
- Ignoring stagnation warnings
