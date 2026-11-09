# Evaluation performance measurements

`epic eval` runs configured programs directly with quoted arguments, without shell expansion.
The performance dimension records one nonnegative finite measurement per command:

- JSON `latency_ms`: lower is better.
- JSON `throughput`: higher is better.
- JSON `duration_ms`: lower is better.
- With no metric field, native elapsed command duration in milliseconds: lower is better.

The first field in that order wins. Whole JSON and logged multiline JSON objects are supported;
the last complete object is used. A present metric with an invalid type/value fails the dimension.

Configure `dimensions.performance.extra.max_regression_pct` (default `10`) to limit deterioration
relative to the baseline. `dimensions.regression` must be enabled for baseline comparison.
The report includes the metric, baseline/current values, absolute delta and deterioration percentage.
A baseline with an incompatible/missing measurement fails explicitly; update it after reviewing
the new measurement contract. Older baseline stdout metrics and native durations are recognized.

A first measurement without a baseline
is allowed. Use `--baseline-update` only when intentionally establishing the reviewed reference.

The comparison uses metrics reported by a trusted project benchmark, or process wall time. It
does not automatically instrument application latency, CPU, allocation or memory use.
