# ADR 0002: Reflection consumes observations by id cursor; cross-project consent stays global

- Status: Proposed
- Date: 2026-11-16
- Area: B (issues #218, #272; PRs #323, #371)
- Spec: `docs/specs/0.9.0-pipeline.md`, section B
- Literature: `docs/references/research-0.9.0-pipeline.md`, Area B

## Context

All references are to main at 74424a2.

Watermark:

- Reflect reads today's observations for the current project from SQLite (`src/hooks/reflect.rs:911-932`), and falls back to today's JSONL files only when the SQLite result is empty or the read failed (`src/hooks/reflect.rs:933-953`). Before the read, today's JSONL fallback files are backfilled into SQLite and deleted when fully inserted (`src/hooks/reflect.rs:811-843`).
- It then drops everything at or below a stored watermark with a strict lexical `>` on ISO timestamps (`src/hooks/reflect.rs:845-857`, `:961-965`). The watermark lives in a per-project file (`src/shared/paths.rs:212-221`; `harness_dir()` is per slug, `src/shared/paths.rs:105-109`).
- The new watermark is the maximum timestamp of the batch and is written before analysis, deliberately: a crash loses one batch instead of re-counting cumulative state (`src/hooks/reflect.rs:969-985`). Fewer than three new observations return early without advancing (`src/hooks/reflect.rs:966-968`).
- `now_iso` has one-second precision (`src/shared/helpers.rs:87-91`). An observation written in the same second as the watermark, after the round read the table, is dropped forever (#218).
- Rows carry a strictly increasing id: `observations.id INTEGER PRIMARY KEY AUTOINCREMENT` (`src/store/schema.rs:260-261`). The read query does not select it (`src/store/observations.rs:130-151`) and `ObsRecord` has no id field (`src/shared/obs.rs:5-23`).
- Project merge relabels rows with `UPDATE ... SET project` and keeps their ids (`src/store/merge_project.rs:148-159`, `:195-200`). It copies only the `obs`, `sessions`, `orbit` and `evolved` directories (`src/store/merge_project.rs:92-100`), so it never touches the watermark file.
- Cumulative state written by a round is not transactional: skill seeding, the seesaw registry, the variant pool, memory ingest, the evolution record, and metrics are written at different points (`src/hooks/reflect.rs:1090-1172`, `:1189-1195`, `:1273-1281`).
- The harness DB is SQLite in practice: only the sqlx `sqlite` and `any` features are enabled (`Cargo.toml:41`) and the DDL uses `AUTOINCREMENT`. `pool.rs` parses `postgres:` URLs (`src/store/pool.rs:41-58`, `:213-229`) but no Postgres driver is compiled in.
- The audit (`.lead/audit/all.json`, PR 323) rated an id cursor as the right idea and the PR as overreach: it removed the JSONL fallback, dropped the "today" scope, and reset the cursor to 0 on merges and migrations, which replays history and re-inflates `total_sessions`. It also deleted the `obs_at` test helper (`src/hooks/reflect.rs:1605-1620`) that #369 (0.8.15) uses.

Cross-project consent (#272):

- Code reads the opt-in marker at `~/.harness/global/.cross-project-enabled` and exports to `~/.harness/global/patterns.jsonl`, with a comment that the marker is intentionally global (`src/shared/paths.rs:268-284`; read in `src/hooks/resume.rs:146-168` and `src/evolve/skills.rs:794`).
- `README.md:579-586`, `README.md:650`, `AGENTS.md:134-137` and `AGENTS.md:323` document a per-project marker and `~/.harness/global_patterns.jsonl`. `docs/data-map.md:14-16` already matches the code. Following the README shares nothing, so the mismatch fails closed.

Attribution: holdout assignment is `hash(skill, date) % modulus == 0` (`src/evolve/metrics.rs:283-288`, modulus at `src/config.rs:216-220`) and reflect credits the pre-seed listing (`src/hooks/reflect.rs:1152-1163`). Verdicts are reported, never enforced (AGENTS.md, Skill Attribution).

## Decision

1. Replace the timestamp watermark with a cursor `{ v: 2, obs_id, ts_floor }` stored in a new per-project file `reflect_cursor.json`, written atomically (tmp file plus rename).
   - SQLite rows are kept when `id > obs_id` and, if `ts_floor` is set, `timestamp > ts_floor`.
   - JSONL fallback rows (no id) are kept when `timestamp > ts_floor`.
   - After a SQLite round, `obs_id` becomes the maximum id among all rows the query returned; `ts_floor` is unchanged. After a JSONL round, `ts_floor` becomes the maximum timestamp of the kept batch; `obs_id` is unchanged.
   - `ts_floor` never decreases and is never cleared. It exists so that JSONL records consumed while SQLite was down are not counted again after the next round backfills them into SQLite with fresh ids.
2. Keep the "advance before analysis" ordering and its crash trade-off. Do not tie the cursor to the evolution record transaction, because the other cumulative writes are not in that transaction.
3. Keep the JSONL fallback and the "today" scope exactly as on main.
4. Migration: when `reflect_cursor.json` is absent and the legacy `reflect_watermark.txt` exists, start from `{ obs_id: None, ts_floor: legacy value }`. Keep writing the legacy file (maximum consumed timestamp, as today) during 0.9.x so a downgrade resumes correctly.
5. A cursor file that exists but does not parse fails closed: the round consumes the batch without analysis, rewrites a valid cursor that covers it, and prints one stderr warning.
6. Merge and migration code never write or reset the cursor. This is the current behavior, now pinned by a test.
7. Attribution stays as is. Option B3 (always-valid sequential tests, CUPED-style variance reduction) is recorded for the day verdicts start to gate behavior.
8. #272: documentation follows the code (memo option C1 for consent). Consent stays global. README and AGENTS.md are corrected; no code change.

## Alternatives considered

- PR 323 as proposed. Rejected for the three audit findings: no SQLite fallback, no "today" scope, cursor reset on merge.
- Commit the cursor in the same transaction as the evolution record (memo B1 wording). Rejected: the record is written late in the round (`src/hooks/reflect.rs:1189-1195`) after seeding and registry writes, so a crash between them would re-count those effects on the next round. That is the exact corruption the current ordering prevents (`src/hooks/reflect.rs:969-976`).
- Store the cursor in SQLite (`metrics_state` row). Rejected for 0.9.0: the cursor must still work when SQLite is unreachable (JSONL path), and a file keeps rollback trivial.
- Reuse `reflect_watermark.txt` with JSON content. Rejected: an older binary would compare `"{"` lexically as a timestamp, which sorts after every ISO timestamp and would make a downgraded binary drop all observations.
- B2 (`>=` plus a persisted seen-id set for the boundary second). Rejected: needs bounded dedup state and is weaker under concurrent sessions; the id column already exists.
- A per-file line-index tie-breaker for the JSONL path. Rejected: the JSONL path only runs while SQLite is unreachable, and its records are re-inserted with new ids by the backfill, so a line index would not identify them afterwards. The residual same-second loss is limited to rounds that crossed a JSONL fallback.
- #272 options C2 (accept either marker) and C3 (per-project marker canonical with migration). C2 rejected: it silently widens consent. C3 is a legitimate product choice, offered in Decision needed B-1.

## Consequences

- Late observations written in the watermark second are no longer lost (#218). Rounds that cross a JSONL fallback keep the old one-second boundary for that boundary only.
- A new file `~/.harness/projects/{slug}/reflect_cursor.json` appears after the first 0.9.0 round. The legacy `reflect_watermark.txt` is still maintained.
- Invariant introduced: ids become visible in increasing order. This holds on SQLite because writers are serialized. It does not hold on Postgres sequences, where a higher id can commit before a lower one. If a Postgres backend is ever enabled, this ADR must be revisited (flagged in the spec's review list).
- Unchanged residuals: concurrent reflect rounds in the same project can still both read the same cursor (read-modify-write race, as today); rows merged in from another slug and dated today can be analyzed again under the target (as today with the watermark).
- #272: README and AGENTS.md describe the global marker and `~/.harness/global/patterns.jsonl`. Users who created the documented per-project marker were never opted in; the changelog says so.

## Migration and compatibility

- Stored files: new `reflect_cursor.json` (`{"v":2,"obs_id":<int|null>,"ts_floor":<string|null>}`). Legacy `reflect_watermark.txt` is read once for migration and dual-written through 0.9.x. Proposed removal of the legacy write: 0.10.0 (Decision needed B-2).
- DB schema: none. The read query additionally selects `id`.
- Config keys: none.

## Rollback

Revert the commit. The older binary reads `reflect_watermark.txt`, which 0.9.0 kept current, so it resumes at the same point; `reflect_cursor.json` is ignored and can be deleted.
