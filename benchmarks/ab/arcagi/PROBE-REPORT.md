# ARC-AGI-3 floor probe — REPORT

**Date:** 2026-09-07 · **Model:** GLM-5.3-flash[1m] via claudy `zai` (bare arm) · **Game:** ls20-9607627b · **Verdict: NO-GO** for the ARC-AGI-3 bare-vs-epic A/B track.

## Question

Pre-registered kill test before committing to an ARC-AGI-3 A/B comparison
(follow-up to the SWE-bench Verified main run): does the base model make ANY
level progress on one public game when driven headless by the same bare-arm
driver as the A/B harness? Zero levels completed means the track has no
dynamic range (floor effect) and is dead.

## Result

| metric | value |
|---|---|
| levels_completed | **0** (all runs) |
| score | 0.0 |
| game actions taken | 6 (bridge-side steps_used) |
| wall time | 1800 s (RUN_TIMEOUT hit, agent_rc 124) |
| pace | ~5 min per game action |
| cost | not captured (timeout killed claudy before the result line) |

The agent was alive and interacting (6 successful bridge steps, multiple
`/new` game runs, one level reset) but made no measurable progress within the
wall budget. A full 60-action bare run at this pace would need ~5 hours of
wall time, so the track is economically dead **before** capability is even the
binding constraint.

## Interpretation

- Consistent with the official ARC-AGI-3 leaderboard: frontier models score
  < 1% (arXiv 2603.24621, March 2026: Opus 4.6 Max 0.50%, GPT 5.4 High 0.20%).
  A mid-tier model scoring exactly 0 on the bare arm is expected, not an
  anomaly. A harness delta cannot show on a benchmark where the base model
  scores zero — the same "epic can only lose" ceiling logic as the smoke
  test, now at the floor.
- Confound: the run died on RUN_TIMEOUT, not on a proven capability ceiling.
  6 actions is a small sample. The verdict rests on pace-economics +
  external leaderboard evidence, not on a clean capability proof.
- Per-action cost structure is the structural killer: each observation is a
  64x64 grid rendered as ~4-5 KB of text through TUI turns, so even a
  competent agent burns minutes per action. SWE-bench turns are cheap by
  comparison.

## Decision

- ARC-AGI-3 A/B track: **shelved**. Revisit trigger: any base model scoring
  > 0 (one level) on a public game under this exact driver within a
  30-minute wall budget. Until then SWE-bench Verified remains the
  benchmark of record for the bare-vs-epic comparison.

## Infrastructure notes (reusable if the track is revisited)

`benchmarks/ab/arcagi/`:

- `bridge.py` — official `arc_agi` toolkit (0.9.9) ONLINE mode in-process,
  one scorecard, frames rendered as text rows, server-side guards
  (ACTION_CAP / MAX_NEW), result JSON auto-written on `/close`.
- `task-arcagi-probe/task.md` — bare-arm prompt (mechanics only, no strategy
  coaching).
- `selftest.sh` — pre-flight: key, game fetch, frame render, step, scorecard,
  result-file write. Run before any agent tokens are spent.
- `run_arcagi_probe.sh` — run_smoke.sh conventions (claudy profile, bare/epic
  toggle, cost/turns capture, cost cap).

Quirks found while wiring:

1. Toolkit REST routes are uppercase: `/api/cmd/RESET`, `/api/cmd/ACTION1..7`
   (the lowercase forms 404).
2. Games whose only lever is ACTION6 (ft09, vc33) return HTTP 500 from
   three.arcprize.org for coordinate-less ACTION6; with `"x":..,"y":..` in
   the payload they return 200. Plain-action games (ls20) work bare. Default
   game is therefore ls20.
3. Local/OFFLINE play needs game files under `ENVIRONMENTS_DIR` which the pip
   package does not ship (repo gitignores `environment_files/`); ONLINE mode
   is the practical path.
4. `arc.make()` auto-resets and each `make()` registers a scorecard run
   entry, so a probe card accumulates several runs; read `max` over runs, not
   `runs[0]`.

Raw result: `results/probe-20260907T160611-ls20-bare.json`
