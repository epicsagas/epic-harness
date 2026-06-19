# benchmarks/ab — bare-vs-epic A/B smoke (issue #94)

Controlled A/B comparison of loading the **epic-harness plugin**, with model and
router held fixed. The only variable is whether the plugin is loaded.

- **Model/router**: `claudy zai` → GLM-5.2[1m] (Z.AI, anthropic-compatible endpoint)
- **bare arm**: `claudy zai --bare -p ...` → `claude --bare` skips plugins/hooks/LSP/mcp
- **epic arm**: `claudy zai -p ...` → normal launch, epic plugin loaded

This is the **smoke test** that de-risks the full SWE-bench Verified main run. It
validates three things on 1–2 cheap self-contained tasks (Docker-free, mechanical
pytest grading) before committing to ~$100+ of SWE-bench instances:

1. GLM-5.2[1m] can drive a coding task headless end-to-end.
2. The bare/epic toggle works (observable via the `input_tokens` overhead delta).
3. pass@1, cost, turns, and latency are all capturable from claude's JSON output.

## Layout

```
ab/
├── run_smoke.sh                 # runner — one task, both arms, comparison table
├── tasks/
│   ├── task1-palindrome/        # easy: fix a buggy is_palindrome
│   │   ├── task.md              # prompt handed to the agent
│   │   ├── repo/                # the project the agent edits (copied per arm)
│   │   └── test_string_utils.py
│   └── task2-moving-average/    # medium: implement moving_average (TDD-framed)
│       ├── task.md
│       └── repo/
└── SMOKE-REPORT.md              # generated feasibility report + verdict
```

## Usage

```bash
# from the repo root (inside the worktree)
./benchmarks/ab/run_smoke.sh benchmarks/ab/tasks/task1-palindrome 12 zai
./benchmarks/ab/run_smoke.sh benchmarks/ab/tasks/task2-moving-average 15 zai
```

Each run writes `result-bare.json`, `result-epic.json`, and `comparison.md` into
the task directory. Env overrides: `COST_CAP` (default 5.0 USD per arm),
`RUN_TIMEOUT` (default 600s).

## Metrics

| metric | source | meaning |
|--------|--------|---------|
| `pass1` | independent `pytest` run after the agent | mechanical pass@1 (1 run) |
| `cost_usd` | `total_cost_usd` from claude JSON | $/instance |
| `num_turns` | `num_turns` | agentic steps |
| `duration_ms` | `duration_ms` | wall latency (API) |
| `input_tokens` | `usage.input_tokens` | context overhead proxy (toggle signal) |

## Notes / limitations (for the main run)

- `--bare` also strips LSP/mcp/hooks, not only the plugin. Acceptable for the smoke
  test (guarantees epic is absent in bare). The main run should switch to a
  per-plugin toggle (`enabledPlugins['epic@epicsagas']` via `--settings`) so that
  **only** the plugin varies.
- SWE-bench Verified grading still requires Docker — run the main run on a
  Docker-equipped host. These tasks use plain `pytest` so no Docker is needed here.
- `pass@1` from a single run is a feasibility signal, not a statistic.
