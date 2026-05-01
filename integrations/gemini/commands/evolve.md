---
description: "Trigger skill evolution manually - analyze observations, evolve skills, show status, or rollback"
---

CRITICAL: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

Run evolution now:
1. Read observation logs from `$HARNESS_DIR/obs/`
2. Analyze failure patterns across all sessions
3. Identify weak areas (error types, recurring failures)
4. Generate or improve evolved skills in `$HARNESS_DIR/evolved/`
5. Gate: validate new skills (format, dedup, cap of 10)
6. Report what changed

Show evolution dashboard:
Read `$HARNESS_DIR/metrics.json` and `$HARNESS_DIR/evolution.jsonl`, then display the dashboard with Overview, Score History, Evolved Skills, and Last Session Analysis.

Long-term analysis:
Read `$HARNESS_DIR/evolution.jsonl`, display Trend Over Time, Cumulative Pattern Frequency, Skill Effectiveness, and Dispatch Analysis.

Cross-project patterns:
Read `~/.harness-global/patterns.jsonl`, display Weak Tools Across Projects and Common Error Patterns.
Opt-in: create `$HARNESS_DIR/.cross-project-enabled` file.

Undo last evolution:
1. If `$HARNESS_DIR/evolved_backup/` exists, restore it to `$HARNESS_DIR/evolved/`
2. Otherwise, read `$HARNESS_DIR/evolution.jsonl` for last entry, remove skills seeded in that entry
3. Append a rollback record to evolution.jsonl
4. Report what was rolled back

Clear all evolution data:
1. Remove `$HARNESS_DIR/evolved/`, `$HARNESS_DIR/evolved_backup/`
2. Clear `metrics.json` and `evolution.jsonl`
3. Confirm with user first
