# benchmarks/ab — bare-vs-epic A/B smoke (issue #94)

Controlled A/B comparison of loading the **epic-harness plugin**, with the
**model held fixed within each comparison**. The only within-model variable is
whether the plugin is loaded.

- **Profiles**: multi-model via claudy profiles (default `zai`).
  - `zai` → GLM-5.2[1m] (Z.AI, anthropic-compatible endpoint)
  - `native` → Claude (family `claude_strict`)
  - openrouter `or-*`, local `ollama`/`lmstudio`/`llamacpp` when configured
- **bare arm**: `claudy <profile> --bare -p ...` → `claude --bare` skips plugins/hooks/LSP/mcp
- **epic arm**: `claudy <profile> -p ...` → normal launch, epic plugin loaded

The bare/epic comparison is valid **within a single model** (plugin = the only
variable). Cross-model rows in the comparison table are informational — they show
how epic's overhead and score vary across model families — and are **not** the
controlled variable of the main run (issue #94).

This is the **smoke test** that de-risks the full SWE-bench Verified main run. It
validates three things on 1–2 cheap self-contained tasks (Docker-free, mechanical
pytest grading) before committing to ~$100+ of SWE-bench instances:

1. A model can drive a coding task headless end-to-end.
2. The bare/epic toggle works (observable via the `input_tokens` overhead delta).
3. pass@1, cost, turns, and latency are all capturable from claude's JSON output.

## Layout

```
ab/
├── run_smoke.sh                 # runner — N models × {bare,epic}, combined table
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
# single profile (legacy form — 3rd positional arg)
./benchmarks/ab/run_smoke.sh benchmarks/ab/tasks/task1-palindrome 12 zai

# multi-model matrix via MODELS env (space- or comma-separated)
MODELS="zai native ollama" ./benchmarks/ab/run_smoke.sh benchmarks/ab/tasks/task1-palindrome 12

# dry-run: validate the loop mechanics (profile resolution, skip logic,
# file naming, table generation) with NO model calls — costs $0
MODELS="zai native openai deepseek" DRY_RUN=1 ./benchmarks/ab/run_smoke.sh benchmarks/ab/tasks/task1-palindrome 12
```

### SWE-bench main run — manifest

`build_manifest.py` samples `difficulty_map.json` into the `manifest.jsonl`
that `run_swebench.sh` consumes (deterministic per `--seed`):

```bash
python3 benchmarks/ab/build_manifest.py --list-bands              # pool sizes
python3 benchmarks/ab/build_manifest.py --per-band 5 --seed 7     # pilot
python3 benchmarks/ab/build_manifest.py --per-band 125 --seed 7   # full run
MANIFEST=benchmarks/ab/manifest.jsonl ./benchmarks/ab/run_swebench.sh
```

Note: the current map has only 3 B4 ("hardest") instances — a 5/band pilot
fails on B4; use `--bands B1,B2,B3 --per-band 5` or accept the B4 cap.


Precedence: the 3rd positional arg (single profile) wins over `MODELS`. With
neither set, the default is `zai`.

Each (profile, arm) cell writes `result-{profile}-{arm}.json`. In single-profile
mode, legacy names `result-bare.json` / `result-epic.json` are created as
symlinks to the actual result files. A combined `comparison.md` holds every ran
row. Env overrides: `COST_CAP` (default 5.0 USD per cell), `RUN_TIMEOUT`
(default 600s), `DRY_RUN` (default 0).

## Profiles

Run `claudy list` to see available profiles and their `configured` /
`not configured` status. Profiles that are **not configured** are skipped with a
warning (`[skip] <name> not configured`) and the run continues — it never fails
the whole matrix on one missing profile.

To configure a profile, run `claudy <name>` and follow the prompts, then exit.
If `claudy list` is unavailable or unparseable, all listed profiles are assumed
configured (fail-soft) and real failures surface per-cell.

## Metrics

| metric | source | meaning |
|--------|--------|---------|
| `family` | `claudy show <profile>` `Family:` | provider family (`claude_strict`, `anthropic_compatible_non_claude`, `local`, …) |
| `model` | `claudy show <profile>` `Model:` | resolved model name (e.g. `glm-5.2[1m]`); `unknown` if the profile has no model set |
| `profile` | claudy profile used | e.g. `zai`, `native`, `ollama` |
| `pass1` | independent `pytest` run after the agent | mechanical pass@1 (1 run) |
| `cost_usd` | `total_cost_usd` from claude JSON | $/cell |
| `num_turns` | `num_turns` | agentic steps |
| `duration_ms` | `duration_ms` | wall latency (API) |
| `input_tokens` | `usage.input_tokens` | context overhead proxy (toggle signal) |

## Notes / limitations (for the main run)

- `--bare` also strips LSP/mcp/hooks, not only the plugin. Acceptable for the smoke
  test (guarantees epic is absent in bare). The main run should switch to a
  per-plugin toggle (`enabledPlugins['epic@epicsagas']` via `--settings`) so that
  **only** the plugin varies.
- SWE-bench Verified grading needs a Docker-compatible container runtime — **Docker or
  Podman** (Podman verified: `swebench` 4.x uses the `docker` Python SDK, which talks to the
  podman socket). One gotcha: a `~/.docker/config.json` with `credsStore: "desktop"` breaks
  pulls/builds (`docker-credential-desktop not installed`); use a clean `DOCKER_CONFIG`.
  These smoke tasks use plain `pytest` so no container is needed here.
- `pass@1` from a single run is a feasibility signal, not a statistic.
- Cross-model rows are **observational**: different models have different token
  pricing and capabilities, so epic's cost/latency overhead is not comparable
  across model families in absolute terms. The controlled A/B lives within each
  model's bare-vs-epic pair.
