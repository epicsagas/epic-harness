# Numeric evaluation settings

`epic eval` validates numeric configuration before executing workload commands.
Invalid values return an error; they cannot silently disable a regression gate.

| Setting under `dimensions` | Unit | Allowed values | Default |
|---|---|---|---|
| `correctness.extra.min_pass_rate` | Passing fraction | Finite, from 0 through 1 | 1 |
| `performance.extra.max_regression_pct` | Percentage | Finite, nonnegative | 10 |
| `regression.extra.threshold` | Absolute score drop | Finite, nonnegative | 0.02 |

YAML `.nan`, `.inf`, `-.inf`, negative allowances, and correctness minima above 1
are rejected. Zero boundaries and generous finite allowances remain supported.
Omitted settings retain their existing defaults. The whole configuration is
validated, including settings for currently disabled dimensions.

A regression threshold of 0.02 permits a score drop of two percentage points;
it is an absolute score difference. The performance limit is a percentage of the
previous compatible measurement. These settings do not change how a trusted
project benchmark produces its scores or measurements.
