---
name: bench-ab-smoke
description: "Run the bare-vs-epic A/B smoke benchmark (benchmarks/ab/) safely. Enforces a DRY_RUN-first, one-model-at-a-time execution order to avoid Claude Code headless-call abuse bans."
---

# bench-ab-smoke — Safe Multi-Model A/B Smoke

## Iron Law

NEVER fire multiple real model calls back-to-back in one shot. Headless
(`-p` / `--output-format json`) requests issued rapidly and repeatedly are
exactly the pattern that triggers Claude Code rate-limit / abuse bans. This
skill exists to make the smoke benchmark safe to run, not just runnable.

## When to Trigger

- Manually, via `/bench-ab-smoke`, when you want to validate the bare-vs-epic
  plugin toggle on the self-contained `benchmarks/ab/tasks/` fixtures.
- Before a full SWE-bench Verified main run (issue #94) — this smoke de-risks
  the mechanics (toggle, capture, grading) at near-zero cost.

## What this measures

For each claudy profile × {bare, epic} arm, on one cheap task:

- `pass1` — mechanical pytest pass/fail (independent grade)
- `cost_usd`, `num_turns`, `duration_ms`, `input_tokens` — from claude JSON
- `family` / `model` — resolved from `claudy show <profile>`

The **within-model** bare-vs-epic pair is the controlled A/B (plugin = only
variable). Cross-model rows are observational (how epic's overhead varies by
family) — not the main-run controlled variable.

## Safe Execution Order (MANDATORY)

Run these phases **in order**. Do not skip ahead.

### Phase 1 — Validate mechanics, $0 cost
```bash
# Always DRY_RUN first: no model calls. Confirms profile resolution,
# configured/skip logic, file naming, and table generation across N models.
MODELS="zai native openai" DRY_RUN=1 ./benchmarks/ab/run_smoke.sh \
  benchmarks/ab/tasks/task1-palindrome 12
```
Expect: result JSONs per cell, a combined `comparison.md`, `[skip]` lines for
not-configured profiles, exit 0. Fix any mechanics issue here — never debug on
paid calls.

### Phase 2 — Single model, ONE run
```bash
# One profile only. This is the only step that spends money (~$0.01–0.03).
./benchmarks/ab/run_smoke.sh benchmarks/ab/tasks/task1-palindrome 8 zai
```
Inspect the result before any further runs. If the gateway returns `529`
(overloaded) or any `is_error: true`, **STOP** — see "On failure" below.

### Phase 3 — Additional models (optional, one at a time)
Only after Phase 2 succeeds. Add ONE more model per invocation and review
between each. Space the invocations out — do not batch:
```bash
MODELS="zai native" ./benchmarks/ab/run_smoke.sh benchmarks/ab/tasks/task1-palindrome 8
```
Lower `max_turns` (e.g. 8) to bound each cell's cost and latency.

## On failure (529 / gateway error)

A `529` (server overloaded) or any `is_error: true` / `api_error_status` set
means the gateway is unhealthy. **Do not retry in the same session.** Retrying
rapidly against a flapping gateway is the highest ban-risk behavior there is.

- Record the failure, stop the run.
- Resume only in a separate session after the gateway recovers.
- The runner already captures `is_error` and `api_error_status` in the result
  JSON — no special handling needed.

## Ban-risk guardrails

- `max_turns` caps agent *actions*, NOT API retry/backoff dwell. A single turn
  can still dwell ~190s on gateway retries. Low `max_turns` is not a substitute
  for spacing calls apart.
- Never loop the runner in a shell `while`/`for` over many profiles/seeds.
- Prefer the interactive TUI (`claudy <profile>` with no `-p`) for exploratory
  runs — a human-paced single call is far safer than batched headless ones.
- A failed cell that spent money but returned no useful output still counts
  toward your rate budget. Treat 529s as "stop", not "retry".

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|--------------------|
| "Running all models at once is faster" | One abuse ban is permanent; the time saved is negligible. | One model per invocation. Review between each. |
| "I'll just retry on 529" | Retrying against an overloaded gateway maximizes ban risk. | Stop. Resume in a new session when healthy. |
| "DRY_RUN wastes time, mechanics look fine" | A $0 check that catches a naming/skip bug saves a paid run that overwrites real results. | Always Phase 1 first. |
| "max_turns=5 makes it safe to spam" | max_turns doesn't bound retry backoff. | Space calls; don't batch. |
| "The runner already handles errors" | It captures them — it doesn't prevent bans from rapid retries. | You control pacing, not the runner. |

## Evidence Required

Before claiming the smoke "passed", show ALL of:

- [ ] Phase 1 DRY_RUN output: `comparison.md` renders, expected `[skip]` lines present, exit 0
- [ ] Phase 2 real single-model result: `pass1`, `cost_usd`, `input_tokens` are non-zero and sane (not all 0)
- [ ] No `is_error: true` / `api_error_status` in the real result line
- [ ] `result-{profile}-{arm}.json` written per cell; legacy symlinks present in single-profile mode

## Red Flags

- Running `MODELS="a b c d" ./run_smoke.sh` (batched multi-model) without a prior single-model Phase 2
- Retrying immediately after a `529`
- Reading `pass1=0, cost=0, input_tokens=0` as "passed" (that's a gateway failure, not a clean fail)
- Treating `num_turns` as a proxy for call frequency / safety
- Skipping DRY_RUN because "the code looks right"
