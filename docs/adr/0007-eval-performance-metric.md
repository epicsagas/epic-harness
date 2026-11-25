# ADR 0007: Eval performance dimension gates on one configured metric against the baseline median

- Status: Accepted (maintainer approved decisions D1 to D5 for issue #275)
- Date: 2026-11-25
- Area: A (issue #275; supersedes decision 7 of ADR 0001)
- Spec: issue #275 spec, narrowed form of A2 in `docs/specs/0.9.0-pipeline.md` section A.8
- Literature: `docs/references/research-0.9.0-pipeline.md`, Area A

## Context

- `run_performance` scores exit status only (`src/eval/runner.rs`). `ResolvedCommands` carries no performance options, so `extra.max_regression_pct` never reached the runner.
- ADR 0001 decision 7 made `max_regression_pct` a warning that never gates, and rejected PR 374 because it gated on single-sample wall-clock time with an automatic fallback to `duration_ms`. Decision A-2 of the 0.9.0 spec deferred opt-in measured metrics (A2) to a later minor with its own ADR.
- Issue #275 reproduces the gap: a command that prints `{"latency_ms": 7.67}` and later `{"latency_ms": 250.35}` reports performance PASS, score 1.0, regression delta 0 and exit 0, with `max_regression_pct: 20` configured.
- Orbit Step 5.5 runs `epic eval --json` and loops to fixes on FAIL, so a noisy gate can cause a fix loop on unchanged code.

## Decision

Ship a narrowed A2 (D1). It is opt-in, explicit, and never measures time itself.

1. New `extra` keys on the performance dimension: `metric` (name of a top-level numeric field in the command's JSON stdout), `better` (`lower` default, or `higher`), `trials` (default 3, range 1..=50, D2), and the existing `max_regression_pct`, which becomes required (finite, `0 < x <= 1000`) when `metric` is set. Validation runs only when `metric` is set, so pre-0.9.0 files that contain `max_regression_pct: 10.0` still load. All new fields are skipped when serializing, so `epic eval --init` scaffolds none of them.
2. Reading the metric (D4): the last JSON object on stdout that has the top-level key wins, using the same scanner as composite benchmarks. Nested keys are not searched. A string, null, boolean or non-finite number is `metric_invalid`; an absent key is `metric_missing`. Both FAIL.
3. Trials run in order and stop at the first non-zero exit (`command_failed`) or metric error. When all succeed, the median is compared against the median stored in the baseline (`details.metric.median`).
4. Regression rule: `pct_worse` is `(cur - base) * 100 / |base|` for `lower` and `(base - cur) * 100 / |base|` for `higher`. The dimension FAILs when `pct_worse > max_regression_pct` (strict, so exactly at the limit passes).
5. Not compared, never a failure: no baseline, a baseline written before this change or without a metric median, a different metric name or direction, and a zero baseline median. The report carries `details.metric.status` (`no_baseline`, `no_baseline_metric`, `baseline_mismatch`, `zero_baseline`) and the CLI prints a one-line stderr note. A metric run therefore never silently reports a comparison it did not make. A metric config that skipped validation (no `max_regression_pct`, or no trials ran) FAILs with status `invalid_config` instead of panicking.
6. A metric regression stays FAIL even when `regression.extra.fail_on_regression` is false (D3). The regression dimension then reports WARN for the resulting score drop, and the overall verdict is FAIL with exit 1.
7. The baseline is loaded whenever the metric needs it, not only when the regression dimension runs, so `--dimension performance` and a disabled regression dimension still compare. An unreadable baseline exits 1 with the existing message.
8. `--baseline-update` is not guarded (D5). A run whose performance FAILed on regression is still saved if requested and becomes the new comparison point, as for every other dimension. The eval skill documents this.
9. Configs without `metric` keep the exact legacy behavior and legacy `details` keys. The warning for `max_regression_pct` without `metric` now says to set `metric` or remove the key, and a second warning covers `better` and `trials` without `metric`.

## Alternatives considered

- The full A.8 design (list of metrics, `min_samples`, `noise_floor_pct`, `schema_version: 2`). Not chosen (D1): it is far larger than the issue needs. The narrowed form can grow into it later because `details.metric` can become one entry of a map without a format break.
- Wall-clock timing with a `duration_ms` fallback (PR 374). Rejected again: single-sample wall-clock gating is the flakiness ADR 0001 recorded.
- A `"metrics": {...}` envelope in command stdout (D4). Not chosen: a top-level key matches the #275 reproduction and common benchmark scripts. An envelope can be accepted later as a second lookup.
- Default `trials` of 1 or 5 (D2). One sample is the flakiness PR 374 was rejected for; five multiplies runtime for every orbit loop. Three gives a median that ignores one outlier.
- Downgrading a metric regression to WARN under `fail_on_regression: false` (D3). Rejected: `max_regression_pct` is an explicit, separate limit, and mixing the two flags would make a configured gate silently advisory.
- Refusing `--baseline-update` after a metric regression (D5). Not chosen: it would be inconsistent with other dimensions. A guard belongs in a separate change that covers all dimensions.

## Consequences

- A project that sets `metric` can fail `epic eval` and orbit Step 5.5 on a real regression. A noisy metric can still cause a fix loop on unchanged code; the mitigations are opt-in use, the default median of 3 trials, and a user-chosen limit. Use 3 or more trials for timing metrics.
- Runtime grows by the number of trials, since the command runs once per trial.
- The first metric run after upgrading reports `no_baseline_metric` and passes; the next `--baseline-update` records a median.
- JSON consumers of the performance dimension see a new `details.metric` object only when `metric` is configured.
- Out of scope, possible later: several metrics per config, `noise_floor_pct`, `min_samples`, a `schema_version` field, change point detection (A3).

## Migration and compatibility

- Config keys: all new keys are optional and unscaffolded. Existing files load unchanged. `max_regression_pct` without `metric` keeps parsing with any value and only warns.
- Stored files: baseline and result JSON keep their shape. The presence of `details.metric.median` marks a baseline usable for comparison; older baselines fall into `no_baseline_metric`.
- Exit codes: unchanged table. New FAIL sources are a metric regression, a missing or invalid metric, and a non-zero trial exit.
- DB schema: none.

## Rollback

Revert the change. No persisted state changes format, so a downgraded binary reads every file written by this version. The extra `extra` keys in `eval.yaml` are ignored by older binaries.
