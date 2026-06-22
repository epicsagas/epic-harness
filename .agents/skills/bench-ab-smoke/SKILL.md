---
name: bench-ab-smoke
description: "A/B smoke benchmark for the bare-vs-epic plugin toggle. Supports two execution modes: (A) headless via run_smoke.sh for scripted multi-model runs; (B) in-TUI for measuring this session's token usage and task completion directly."
dimension: benchmark
---

# bench-ab-smoke — A/B Smoke Benchmark

## Execution Modes

| Mode | When to use | Arm measured |
|------|-------------|--------------|
| **A — Headless** | Multi-model matrix, scripted run | Both bare + epic via `claudy -p` |
| **B — In-TUI** | Measure this session's tokens directly | Epic arm = this session; bare arm = separate `--bare` TUI |

Mode B is preferred for quick validation — no headless calls, no ban risk, no gateway overload.

---

## Mode A — Headless Runner (run_smoke.sh)

### Iron Law for Mode A

NEVER fire multiple real model calls back-to-back. Headless (`-p` / `--output-format json`)
requests issued rapidly trigger Claude Code rate-limit / abuse bans.

### Safe Execution Order (Mode A — MANDATORY)

#### Phase 1 — Validate mechanics, $0 cost
```bash
MODELS="zai native openai" DRY_RUN=1 ./benchmarks/ab/run_smoke.sh \
  benchmarks/ab/tasks/task1-palindrome 12
```
Expect: result JSONs per cell, combined `comparison.md`, `[skip]` lines for
not-configured profiles, exit 0. Fix mechanics here — never debug on paid calls.

#### Phase 2 — Single model, ONE run (~$0.01–0.03)
```bash
./benchmarks/ab/run_smoke.sh benchmarks/ab/tasks/task1-palindrome 8 zai
```
Inspect before any further runs. On `529` or `is_error: true` → **STOP**.

#### Phase 3 — Additional models (optional, one at a time)
```bash
MODELS="zai native" ./benchmarks/ab/run_smoke.sh benchmarks/ab/tasks/task1-palindrome 8
```

### On failure (529 / gateway error) — Mode A

Stop immediately. Do not retry in the same session. Resume in a new session after recovery.

### Mode A Ban-risk guardrails

- `max_turns` caps agent actions, NOT API retry/backoff dwell (~190s on 529).
- Never loop the runner over many profiles/seeds.
- A failed cell still counts toward rate budget.

---

## Mode B — In-TUI Execution (this session = epic arm)

### Overview

The skill itself performs the smoke task within this TUI session. After completion,
a helper script reads the session JSONL to extract token usage for the turns that
covered the task. Results are written in the same `result-{model}-{arm}.json` schema.

```
this TUI session (epic arm)  →  fix bug  →  pytest grade  →  parse JSONL  →  result-claude-sonnet-4-6-epic.json
separate --bare TUI session  →  fix bug  →  pytest grade  →  parse JSONL  →  result-claude-sonnet-4-6-bare.json
```

### Step 0 — Setup: copy task to isolated workdir

```bash
TASK_SRC="benchmarks/ab/tasks/task1-palindrome"
WORKDIR=$(mktemp -d /tmp/bench-task-XXXXXX)
cp -r "$TASK_SRC/repo/." "$WORKDIR/"
echo "workdir: $WORKDIR"
```

Record the start marker (ISO8601 timestamp) before doing any work:
```bash
BENCH_START=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
echo "bench_start=$BENCH_START"
```

### Step 1 — Perform the task (epic arm)

Fix `string_utils.py` in `$WORKDIR` so all pytest tests pass. This is the epic arm
because this TUI session has epic-harness loaded.

Constraints:
- Work only in `$WORKDIR` — never touch the original `benchmarks/ab/tasks/*/repo/`
- Do NOT modify `test_string_utils.py`
- Run `pytest` from `$WORKDIR` to verify

### Step 2 — Grade with pytest

```bash
cd "$WORKDIR"
pytest -q 2>&1
PYTEST_RC=$?
PASS1=$([ $PYTEST_RC -eq 0 ] && echo 1 || echo 0)
echo "pass1=$PASS1"
```

### Step 3 — Record end marker

```bash
BENCH_END=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
echo "bench_end=$BENCH_END"
```

### Step 4 — Parse session JSONL for token delta

Run the capture script to aggregate tokens from assistant turns between
`bench_start` and `bench_end`:

```bash
benchmarks/ab/scripts/capture_tui_tokens.py \
  --jsonl "$(ls -t ~/.claude/projects/-Users-hackme-workspace-projects-epiccounty-epic-harness/*.jsonl | head -1)" \
  --start "$BENCH_START" \
  --end "$BENCH_END"
```

Output: `input_tokens`, `output_tokens`, `model`, `num_turns`, `duration_ms`.

### Step 5 — Write result JSON

```bash
# Replace <values> with actual captured numbers
TASK_DIR="benchmarks/ab/tasks/task1-palindrome"
MODEL="claude-sonnet-4-6"   # from JSONL parse
jq -n \
  --arg arm "epic" \
  --arg task "task1-palindrome" \
  --argjson pass1 $PASS1 \
  --arg model "$MODEL" \
  --arg family "claude" \
  --arg profile "native" \
  --argjson input_tokens <INPUT_TOKENS> \
  --argjson output_tokens <OUTPUT_TOKENS> \
  --argjson num_turns <NUM_TURNS> \
  --argjson duration_ms <DURATION_MS> \
  '{arm:$arm, task:$task, ok:($pass1==1), pass1:$pass1,
    is_error:false, model:$model, family:$family, profile:$profile,
    input_tokens:$input_tokens, output_tokens:$output_tokens,
    num_turns:$num_turns, duration_ms:$duration_ms,
    wall_s:($duration_ms/1000|floor), cost_usd:null, cost_capped:0,
    workdir:null, tui_mode:true}' \
  > "$TASK_DIR/result-${MODEL}-epic.json"
```

`cost_usd: null` — pricing varies; user can fill in based on model rates.

### Step 6 — Bare arm (separate session)

Open a new TUI session with `--bare` flag (strips plugins/hooks). Repeat Steps 0–5
in that session, save result as `result-{model}-bare.json`. Then compare.

### Step 7 — Comparison

```bash
# After both arms collected:
RESULT_EPIC="$TASK_DIR/result-${MODEL}-epic.json"
RESULT_BARE="$TASK_DIR/result-${MODEL}-bare.json"
jq -r '[.arm, .pass1, .input_tokens, .output_tokens, .num_turns, .duration_ms] | @tsv' \
  "$RESULT_BARE" "$RESULT_EPIC" | column -t
```

---

## capture_tui_tokens.py — Token Capture Script

Location: `benchmarks/ab/scripts/capture_tui_tokens.py`

This script aggregates `assistant` records from the session JSONL between two
ISO8601 timestamps and emits a JSON summary.

**Fields extracted per assistant record:**
- `message.usage.input_tokens`
- `message.usage.output_tokens`
- `message.model` (last non-`<synthetic>` value)
- Count of records = `num_turns`

**Duration:** wall time from first to last assistant record timestamp in range.

---

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|--------------------|
| "Mode A is faster — one command" | One ban is permanent; Mode B has zero ban risk. | Use Mode B for quick validation, Mode A for multi-model matrix. |
| "I'll retry the headless run on 529" | Retrying against overloaded gateway maximizes ban risk. | Stop Mode A. Switch to Mode B or new session. |
| "DRY_RUN wastes time (Mode A)" | A $0 check catches naming/skip bugs before paid calls. | Always Phase 1 first in Mode A. |
| "Skipping bare arm for now" | Epic overhead is meaningless without a baseline. | Run bare arm (separate --bare session) before claiming results. |
| "I can estimate tokens without JSONL" | Estimates are useless for benchmarking. Parse the actual JSONL. | Run capture_tui_tokens.py after every task. |

## Evidence Required

### Mode A
- [ ] Phase 1 DRY_RUN: `comparison.md` renders, `[skip]` lines present, exit 0
- [ ] Phase 2 real result: `pass1`, `cost_usd`, `input_tokens` non-zero and sane
- [ ] No `is_error: true` / `api_error_status`
- [ ] `result-{profile}-{arm}.json` written per cell

### Mode B
- [ ] `WORKDIR` created; original `repo/` untouched
- [ ] `bench_start` / `bench_end` timestamps recorded (ISO8601)
- [ ] `pytest` grade shown (`pass1=0` or `1`)
- [ ] `capture_tui_tokens.py` output: `input_tokens`, `output_tokens`, `num_turns` non-zero
- [ ] `result-{model}-epic.json` written with correct schema
- [ ] Bare arm result collected in separate `--bare` session

## Red Flags

- Performing the task without recording `bench_start` first (can't compute delta)
- Modifying `test_string_utils.py` (invalidates grade)
- Touching original `repo/` instead of `WORKDIR` copy (corrupts baseline)
- Running Mode A headless calls without Phase 1 DRY_RUN
- Reporting `input_tokens=0` as valid (gateway failure or parse error)
- Skipping bare arm and comparing epic-only numbers in isolation
