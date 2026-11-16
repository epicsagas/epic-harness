# ADR 0005: Orbit lock always checks running pipelines; agent staleness uses the newest liveness signal; state readers accept a BOM

- Status: Proposed
- Date: 2026-11-16
- Area: E (issues #247, #244, #282; PRs #348, #345, #393)
- Spec: `docs/specs/0.9.0-pipeline.md`, section E
- Literature: `docs/references/research-0.9.0-pipeline.md`, Area E

## Context

All references are to main at 74424a2.

Orbit lock:

- `epic orbit lock` calls `try_acquire_orbit_lock` and exits 1 with "another orbit is running" on refusal; `epic orbit unlock` warns on invalid completion and removes the lock (`src/main.rs:227-249`).
- The lock is a file created with `create_new`, because the owner is a short-lived CLI and the lock must outlive it (`src/shared/orbit.rs:158-166`, `:185-195`). The running-pipeline scan runs only when the lock file already exists (`src/shared/orbit.rs:177-184`). A fresh `running` PIPELINE file with no lock file is therefore admitted (#247, confirmed part).
- A gap between acquiring the lock and writing the first PIPELINE file is a documented ceiling (`src/shared/orbit.rs:168-171`). Unlock is unconditional by design because it runs on abort and after crashes (`src/shared/orbit.rs:198-201`, `:216-218`).
- A `running` file older than 45 minutes is treated as crashed (`src/shared/orbit.rs:8-13`, `:73-85`).
- The reclaim path (`remove_file` then `create_new`) is a check-then-act sequence: two processes that both see a stale lock can both remove and recreate it. The `create_new` race only protects the "no lock file" path.

Orchestrator cleanup:

- `auto_cleanup_stale_runs` marks a Running agent Done when `started_at` is more than 30 minutes old (`src/orchestrate/state.rs:482-496`), although its doc comment says heartbeat (`:464`). The heartbeat in `agents/{id}/status.json` is consulted only later, to decide directory removal (`src/orchestrate/state.rs:527-549`). Heartbeats are refreshed by the pre-invocation hook only (`src/orchestrate/mod.rs:126-140`). A long-running agent with a fresh heartbeat is marked Done (#244).

BOM:

- PIPELINE files are parsed with `serde_json::from_str` (`src/shared/orbit.rs:72`), which rejects a leading U+FEFF. Windows PowerShell 5.1 writes a BOM with `-Encoding UTF8` (memo Area E item 6). A BOM-prefixed running pipeline is treated as absent and the lock is reclaimed (#282).
- The same pattern exists in every reader of host-authored state: `src/shared/orbit.rs:272`, `src/hooks/reflect.rs:1296`, `:1502`, `src/orchestrate/state.rs:166`, `:535`, `src/hooks/guard.rs:253`, `:343`, `src/serve.rs:784`, `src/store/orbit_store.rs:68`, `:97` (reflect stores raw file content in the DB, `src/hooks/reflect.rs:1302-1311`), `src/evolve/outcomes.rs:327`, and the generic `read_json` (`src/shared/helpers.rs:178-183`).

Audit (`.lead/audit/all.json`): PR 348 adds an admission lock file, a 2-minute reservation, an `EPIC_ORBIT_OWNER` env var, and makes unlock fail while running, reversing the abort and crash design; minimal fix is to always scan before acquiring. PR 345 rewrites cleanup semantics (Blocked, Paused, no orphan removal, no 2-hour completion); minimal fix is `max(started_at, last_heartbeat)`. PR 393's `parse_persisted_json` helper is correct and independent.

## Decision

Adopt memo option E1:

1. `try_acquire_orbit_lock` runs `scan_running_pipeline_in` first, always. A fresh running pipeline refuses the lock whether or not a lock file exists. Then, as today: an existing lock file with no running pipeline is reclaimed; otherwise the lock file is created with `create_new`. Unlock stays unconditional.
2. Cleanup computes an agent's last liveness as the newer of `started_at` and `status.json`'s `last_heartbeat`, and marks it Done only when that is more than 30 minutes old. Thresholds and the rest of cleanup (2-hour run completion, directory removal rule) are unchanged.
3. Add `parse_persisted_json<T>(&str)` to `src/shared/helpers.rs`. It strips exactly one leading U+FEFF and parses with serde. Use it at every reader listed above. Writers never emit a BOM.
4. Keep the documented startup window and the reclaim check-then-act race as ceilings (comments updated). Fencing tokens and kernel locks are deferred.
5. UTF-16 files (PowerShell 5.1 `Out-File` and `>` redirection) are out of scope: `read_to_string` fails, and the file is treated as unreadable, as today (Decision needed E-1).

## Alternatives considered

- E3, PR 348 as proposed. Rejected: unlock failing while a pipeline runs strands the lock after aborts and crashes (the reason unlock is unconditional), and the env-gated release path adds a hidden contract.
- E2, OS advisory lock (`File::try_lock`, stable since Rust 1.89, memo Area E item 3) plus heartbeat and fencing token. Rejected for 0.9.0: orbit runs as short-lived CLI invocations, so no process lives long enough to hold a kernel lock; fencing only helps if every PIPELINE writer checks the token (memo Area E item 1), and writers include the model following the skill.
- Close the startup window with a reservation on the lock file age. Not chosen: it reintroduces the reservation semantics the audit questioned in PR 348, and the window has caused no reported incident. Offered as Decision needed E-2.
- PR 345's cleanup rewrite. Rejected: changes directory-removal criteria for `fs::remove_dir_all` and run completion, far beyond the bug.
- Phi accrual failure detection (memo Area E item 2). Rejected: heartbeats come from one pre-invocation hook, so inter-arrival statistics would mostly measure the model's tool-call cadence.

## Consequences

- A second `epic orbit lock` while a running pipeline exists, but no lock file, now exits 1 (was admitted).
- BOM-prefixed PIPELINE, control, run and status files are honored everywhere.
- Long-running agents with fresh heartbeats are no longer marked Done after 30 minutes.
- Documented ceilings remain: startup window, reclaim race, UTF-16 files, and cleanup racing a heartbeat write during `remove_dir_all`.

## Migration and compatibility

No config keys, DB schema, file formats, or CLI exit codes change. Files with a BOM that were previously ignored are now read, which is the fix.

## Rollback

Revert the commit. No state written by 0.9.0 differs from 0.8.x.
