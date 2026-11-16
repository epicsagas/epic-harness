# ADR 0001: Eval regression gating gets a SKIPPED verdict and validated thresholds

- Status: Proposed
- Date: 2026-11-16
- Area: A (issues #179, #284, #275; PRs #289, #380, #374)
- Spec: `docs/specs/0.9.0-pipeline.md`, section A
- Literature: `docs/references/research-0.9.0-pipeline.md`, Area A

## Context

All references are to main at 74424a2 unless noted.

- `epic eval --dimension regression` runs no producer. Producers run only when the filter is absent or names them (`src/eval/mod.rs:118-135`); the regression dimension is then computed over whatever ran (`src/eval/mod.rs:146-153`), which is an empty slice for `--dimension regression`. `compute_regression` iterates only current results that have a baseline entry (`src/eval/runner.rs:184-205`), so no comparisons means no regression and verdict PASS (`src/eval/runner.rs:207-216`).
- With no baseline at all, the regression dimension returns PASS with score 1.0 and a "first run" note (`src/eval/runner.rs:170-179`). A run that measured nothing reports PASS.
- SKIPPED producers are compared as if they were measurements. A SKIPPED dimension carries score 0.0 (`src/eval/runner.rs:28-35`, `:86-93`, `:124-131`), and `compute_regression` does not look at the verdict, so a baseline score of 1.0 against a now-SKIPPED dimension yields delta -1.0 and FAIL (`src/eval/runner.rs:185-194`).
- SKIPPED dimensions are averaged into `overall_score` (`src/eval/report.rs:25-28`, `:44-48`), and the overall verdict has no SKIPPED value (`src/eval/report.rs:50-61`); the table printer has no icon for an overall SKIPPED (`src/eval/report.rs:101-106`).
- `threshold` is deserialized without validation (`src/eval/config.rs:70-71`, `:195-203`). `.nan` or `.inf` make `delta < -threshold` always false (`src/eval/runner.rs:191`), which silently disables the gate. The default is 0.05 (`src/eval/config.rs:114-116`, `:183-191`).
- `max_regression_pct` is dead config (`src/eval/config.rs:51-55`, `:111-113`, `:169-175`): it never reaches `ResolvedCommands` (`src/eval/config.rs:381-387`), and `run_performance` scores exit status only (`src/eval/runner.rs:80-114`). `epic eval --init` serializes `EvalConfig::default()` (`src/eval/config.rs:226-232`), so every scaffolded `eval.yaml` contains `max_regression_pct: 10.0` and suggests a latency gate that does not exist.
- Exit code is 1 only for an overall FAIL (`src/eval/mod.rs:197`). Orbit Step 5.5 runs `epic eval --json` and branches on PASS and FAIL only (`skills/orbit/SKILL.md:155-165`); its report table already allows `{PASS|SKIPPED}` (`skills/orbit/SKILL.md:242`).
- No code outside `src/eval/` reads eval reports or baselines (grep for `overall_verdict`, `overall_score`, `EVAL-`, `latest.json`, `baselines` across `src/` and `app/src/` finds only `src/eval/` and `src/main.rs` wiring). Changing report semantics affects the CLI, its JSON consumers, and the two skills, not reflect or attribution.
- Release context: 0.8.14 (in review, not on main) adds `min_pass_rate` validation inside `config::load` and fail-closed composite parsing in `runner.rs`. 0.8.15 is planned to carry #306 (argument allow-list, exit 2 for usage errors), #307 (corrupt baseline is an error), and #309 (`fail_on_regression` honored). This ADR builds on those and does not redo them.
- The audit (`.lead/audit/all.json`, PR 289) found the "run all producers, skip SKIPPED dims" idea correct but rejected a global "everything skipped means exit 1" rule because it fails orbit Step 5.5 for scaffolded projects without benchmarks. For PR 374 it found that gating on wall-clock time with a 10% default and a possible 0 ms baseline would create flaky failures after upgrade.

## Decision

Adopt option A1 from the memo, refined:

1. Validate `dimensions.regression.extra.threshold` at load: finite and within `0.0..=1.0`, otherwise `epic eval` exits 1 with a message naming the key. Same placement and style as the 0.8.14 `min_pass_rate` check.
2. `--dimension regression` runs every enabled producer (the same set as an unfiltered run), then computes regression.
3. Regression compares only dimensions that are measured on both sides: current verdict is not SKIPPED, the baseline entry exists, the baseline verdict is not SKIPPED, and both scores are finite. Everything else is listed in `details.skipped` with a reason.
4. Zero comparable dimensions, including "no baseline yet", gives the regression dimension the verdict SKIPPED instead of PASS.
5. The overall verdict gains SKIPPED, which applies only when every dimension is SKIPPED or there are no dimensions. Precedence: FAIL, then WARN, then PASS, then SKIPPED. `overall_score` averages non-SKIPPED dimensions only.
6. Exit code for an overall SKIPPED is 0 (non-blocking but visible), pending maintainer confirmation (spec, Decision needed A-1). Orbit Step 5.5 treats SKIPPED as "proceed to Ship and record SKIPPED".
7. `max_regression_pct` becomes `Option<f64>`, is no longer written by `--init`, and prints a one-line stderr warning when it is set while the performance dimension is enabled. It never gates.
8. `--baseline-update` refuses to save a baseline when the overall verdict is SKIPPED (stderr warning, exit 0), so an empty run cannot become the comparison point.

Option A2 (opt-in measured metrics with trials and a noise floor) is specified in the spec as a stretch item and ships only with maintainer approval (Decision needed A-2). Option A3 (change point detection) is deferred.

## Alternatives considered

- PR 289 as proposed (global "all SKIPPED or empty means exit 1"). Rejected: breaks orbit Step 5.5 for every scaffolded project without benchmarks (audit), and turns a legitimate "nothing configured" state into a failure. The memo's OPA `--fail-on-empty` analogy (Area D, item 6) applies to a test runner that executed zero tests, not to an evaluator with no configured measurements.
- Keep "first run" as PASS. Rejected: a run that compared nothing is not a pass; this is the same defect as #179 in another form.
- PR 374 as proposed (metric contract with automatic wall-clock fallback and pickup of old baselines' `duration_ms`). Rejected: single-sample wall-clock gating is the failure mode the literature warns against (memo Area A items 1, 2, 5), and the audit predicts immediate flaky failures after upgrade.
- Reject `max_regression_pct` with exit 1. Rejected: every scaffolded config contains it, so the upgrade would break every eval run for a key that never did anything.
- Silently drop `max_regression_pct`. Rejected: users who enabled performance and tuned the value believe a latency gate exists; a warning corrects that belief.
- A distinct exit code (for example 3) for SKIPPED. Not chosen by default because exit codes are public contract; offered as an option in Decision needed A-1.
- A3 change point detection (E-Divisive or Hunter). Deferred: needs long per-run history, which most projects do not have (memo Area A recommendation).

## Consequences

User-visible behavior changes (0.9.0 minor, breaking under 0.x semver):

- `epic eval --dimension regression` now runs the configured producers and can exit 1 when a producer fails or a measured dimension regressed. It used to always pass.
- A fresh scaffold with no benchmarks reports `overall_verdict: "SKIPPED"` and `overall_score: 0.0` (was PASS). Exit code stays 0.
- The first run after `--init` reports regression SKIPPED with reason `no_baseline` (was PASS "first run").
- A dimension that became SKIPPED (for example its command was removed) no longer produces a regression FAIL against an old baseline.
- `overall_score` rises for configs that mix SKIPPED and measured dimensions, because SKIPPED no longer counts as 0.0.
- Invalid `threshold` values now fail fast with exit 1.
- `max_regression_pct` warns when set with performance enabled.
- JSON consumers must accept `"SKIPPED"` in `overall_verdict` and the new `details` keys of the regression dimension (`compared`, `skipped`, `missing_in_current`).

No change to reflect, attribution, or the observation scoring pipeline (no reader outside `src/eval/`).

## Migration and compatibility

- Config keys: `threshold` unchanged in name and default (0.05); new validation only. `max_regression_pct` keeps parsing (old files load), type becomes optional, no longer scaffolded. No new keys in A1.
- Stored files: baseline and result JSON keep their shape. The regression dimension's `details` object gains keys; old baselines remain readable. An old baseline whose dimensions were SKIPPED is now treated as "no baseline entry" for those dimensions.
- DB schema: none.
- Skills: `skills/orbit/SKILL.md` Step 5.5 and `skills/eval/SKILL.md` describe the SKIPPED branch.

## Rollback

Revert the A1 commit. No persisted state changes format, so a downgraded binary reads all files written by 0.9.0. A user who removed `max_regression_pct` from `eval.yaml` loses nothing on downgrade (the field defaults to 10.0 there).
