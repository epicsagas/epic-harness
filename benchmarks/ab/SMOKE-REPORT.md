# SMOKE-REPORT — bare vs epic A/B feasibility (issue #94)

**Date:** 2026-06-19 · **Model:** GLM-5.2[1m] via claudy `zai` (Z.AI) · **Scope:** smoke test (1–2 tasks)

## Verdict

**FEASIBILITY: GO.** The three mechanics the main run depends on all work on GLM-5.2[1m]:
GLM drives a coding task headless end-to-end, the bare/epic toggle takes effect, and
cost/turns/latency/tokens are all capturable.

**On these tasks, epic = pure overhead** — same pass@1 as bare at 9–20× the cost and
2.5–4× the latency. **This does not mean epic is useless**: the tasks here are trivial
single-function fixes that never engage epic's skills (spec/tdd/secure/debug/orbit). The
result is predetermined for trivial tasks. The main run must use tasks hard enough to
exercise epic before any conclusion about the plugin's value is valid.

## Feasibility questions (answered)

| # | Question | Answer | Evidence |
|---|----------|--------|----------|
| a | Can GLM-5.2[1m] drive a coding task headless end-to-end? | **YES** | Both arms solved both tasks (pass@1 = 1 on all 4 runs), exit 0, no `is_error` |
| b | Does the bare/epic toggle take effect? | **YES** | epic injects ~85–88k input tokens; bare ~1.6–3.8k — a 23–51× context delta |
| c | Are pass@1 / cost / turns / latency captured? | **YES** | All 5 metrics present in claude `--output-format json` for every run |

## Setup

- **Router/model held fixed:** `claudy zai` → GLM-5.2[1m] (Z.AI anthropic-compatible endpoint). The plugin is the only variable.
- **bare arm:** `claudy zai --bare -p …` → `claude --bare` (skips plugins/hooks/LSP/mcp)
- **epic arm:** `claudy zai -p …` → normal launch, `epic@epicsagas` plugin loaded
- **Tasks:** 2 self-contained Python tasks, Docker-free, graded by independent `pytest` (tests fail before, pass after a correct fix — validated).
  - `task1-palindrome` (easy): fix a buggy `is_palindrome`
  - `task2-moving-average` (medium, TDD-framed): implement `moving_average` to a test spec
- **Isolation:** fresh `mktemp` workdir per arm; agent launched from inside it. `--max-turns` 12/15, `bypassPermissions`, `$5/arm` cost cap (never hit).

## Results

| task | arm | pass@1 | cost | turns | dur(ms) | input_tok | wall(s) |
|------|-----|--------|------|-------|---------|-----------|---------|
| task1 palindrome | bare | 1 | $0.027 | 6 | 17,063 | 1,671 | 18 |
| task1 palindrome | epic | 1 | $0.542 | 6 | 68,970 | 85,278 | 72 |
| task2 moving-avg | bare | 1 | $0.067 | 6 | 34,903 | 3,757 | 36 |
| task2 moving-avg | epic | 1 | $0.625 | 7 | 86,050 | 88,441 | 89 |
| **totals** | **bare** | 2/2 | **$0.094** | 12 | — | 5,428 | 54 |
| **totals** | **epic** | 2/2 | **$1.168** | 13 | — | 173,719 | 161 |

**Per-instance overhead of loading epic (same model, same outcome):**
- cost: **9–20×** ($/instance)
- latency: **2.5–4×**
- context: **+83–85k input tokens/instance** (the cost driver)
- turns: +0 to +1

## Analysis

**Correctness:** tie. Both arms solved both tasks. The plugin neither helped nor hurt pass@1.

**Cost/efficiency:** epic is dramatically more expensive per instance for an identical
outcome. The overhead is almost entirely **context injection** — the plugin loads ~85k
tokens of skills + CLAUDE.md + system scaffolding into every request, regardless of task
size. On a trivial task there is no chance for that context to pay for itself.

**Procedural adherence (qualitative, n=1 per cell):** on task2 the **bare** arm produced
higher-quality output — a full Google-style docstring and clean slice logic — while the
**epic** arm emitted a docstring-less one-liner and took one extra turn. This is a single
sample and could be noise, but it raises a hypothesis worth testing in the main run:
*does epic's skill churn reduce output polish on GLM for small tasks?*

**Mapping to the issue's interpretation table:** `pass@1 same | $/instance ↑↑ | → pure
overhead → recheck smoke test`. We did recheck — the overhead is real, but attributable to
trivial tasks that cannot engage the plugin.

## Cost of this smoke test

| bucket | spend |
|--------|-------|
| Clean A/B runs (4 cells) | $1.26 |
| Mechanics pre-check (2 probe calls) | $0.14 |
| One contaminated debug run (cwd bug; killed mid-arm) | ~$0.5–1.0 (unmeasured) |
| **Total** | **~$1.9–2.4** |

(All on GLM-5.2[1m]; the cwd-isolation bug that contaminated one run is fixed in the runner and documented as a memory.)

## Recommendation for the main run — GO, with four requirements

1. **Use tasks of real difficulty.** Trivial single-function fixes cannot distinguish
   "epic helps" from "epic is overhead" — epic can only lose. Select SWE-bench Verified
   instances that are genuinely multi-file / multi-step so epic's spec→go→tdd→secure→debug
   skills have a chance to engage. Without this, the result is predetermined.
2. **Test the actual intended epic surface.** This smoke used single-shot `-p` coding
   prompts. If the main run means driving **`/epic:orbit`** end-to-end, that is a separate,
   heavier capability (worktrees, sub-agents, PR creation) that this smoke did **not**
   exercise — and it is the issue's stated top risk ("GLM may not drive the multi-stage
   pipeline stably"). Run a 1-instance `/orbit` smoke before scaling.
3. **Tighten the toggle for purity.** `--bare` also strips LSP/mcp/hooks, not only the
   plugin. Acceptable for smoke; for the main run switch to a per-plugin toggle
   (`enabledPlugins['epic@epicsagas']` via `--settings`) so *only* the plugin varies.
4. **Container runtime + budget.** SWE-bench Verified grading needs a Docker-compatible
   container runtime — **not Docker specifically: Podman works** (verified, see Appendix).
   `swebench` 4.x drives containers via the `docker` Python SDK, which talks to podman's
   socket transparently (`/var/run/docker.sock` → podman machine). Actual host requirements:
   (a) a runtime exposing a Docker-compatible socket; (b) a clean docker config —
   `~/.docker/config.json` with `credsStore: "desktop"` breaks every pull/build with
   `docker-credential-desktop not installed`; point `DOCKER_CONFIG` at a `{}` config;
   (c) ~1.3 GB base + ~2.2 GB env (shared) + ~2.4 GB/instance disk. Budget: epic
   ~$0.6/instance here × 10–20 instances × (harder tasks ⇒ more turns) ⇒ plan **$30–120+**;
   set the `$5/arm` cap and watch it.

## Limitations

- `pass@1` from a single run per cell is a feasibility signal, not a statistic (no variance, no significance).
- Trivial tasks bias against the plugin (see Analysis).
- `--bare` is not a surgically plugin-only toggle (see requirement 3).
- Cost figures are GLM-5.2[1m] spot prices; rerun for current pricing.

## Appendix — Podman grading verified (2026-06-19, post-smoke)

The smoke tasks are Docker-free by design (plain `pytest`). To confirm the main run's
container grading is not actually Docker-gated, the harness (`swebench` 4.1.0) was driven
end-to-end under **Podman 5.8.2** on an arm64 host against one real SWE-bench Verified
instance (`sympy__sympy-23950`, sympy 1.12).

| patch | verdict | FAIL_TO_PASS (`test_as_set`) | PASS_TO_PASS (4) | wall |
|-------|---------|------------------------------|------------------|------|
| gold (`return self.args[1]`) | **resolved** | success | all success | 91.7 s* |
| wrong (`return self.args[0]`) | **unresolved** | failure | all success | 18.6 s |

\* includes base+env+eval image build (native arm64); the test run itself was 2.2 s. The
wrong-patch row reused cached images. Verdict discrimination confirmed: the gold patch
resolves, a one-character-off patch is rejected.

- **Runtime:** Podman 5.8.2 (arm64 machine). `docker.from_env()` connects to the podman
  socket; no shim or `docker` CLI needed.
- **Blocker found:** `credsStore: "desktop"` in `~/.docker/config.json` →
  `docker-credential-desktop not installed` → every pull/build died with a `StoreError`.
  Fixed by pointing `DOCKER_CONFIG` at a dir containing a `{}` config. This was the real
  failure on the host — not Docker-vs-Podman.
- **Arch:** the harness defaults to x86_64; this run forced native arm64
  (`arch="arm64"` + local build, since the registry pre-built images are x86_64 only).
  Native arm64 is clean for pure-Python repos. **C-extension repos confirmed to break**:
  `scikit-learn__scikit-learn-14141` (sklearn 0.22) env build died because its
  `setup_env.sh` pins `pip install scipy==1.5.2`, which has no aarch64 wheel → source
  build → needs `gfortran` (absent from the base image) → `library mach has Fortran
  sources but no Fortran compiler found`. (conda-forge's aarch64 scipy 1.5.3 was fine; the
  pip downgrade is what broke it.)
- **Implication for requirement 4:** the main run does **not** require relocating to "a
  Docker host" — any Docker-compatible runtime with a clean config suffices. Prefer a
  native x86_64 host to match the pre-built images and skip the build.
