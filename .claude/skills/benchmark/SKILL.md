---
name: benchmark
description: "Executes Bare-vs-Epic A/B benchmarking, Ring 0 Guard 50 challenge, and full golden set evaluation for epic-harness. Orchestrates worker sessions, enforces strict workspace isolation, and synthesizes multi-dimensional comparison reports in TUI. Triggers: /benchmark, 'benchmark', 'a/b test', 'eval harness', 'smoke test', 'full eval'."
---

# Benchmark — epic-harness A/B Evaluation & Golden Set Runner

**CRITICAL**: This is a repository-local developer evaluation skill. It is **NOT** included in the published `epic-harness` plugin bundle to prevent runtime context overhead in user projects.

---

## 1. When to Trigger
- Explicit `/benchmark`, `/benchmark full`, `/benchmark smoke`, or `/benchmark guard` commands
- User asks to "run A/B test", "evaluate harness", "run smoke test", or "benchmark all"
- Pre-release verification to confirm zero regressions and 100% guard interception rate
- Comparing new model profiles (e.g., Claude 3.7 Sonnet, GLM-5, GPT-4o) on Bare vs Epic

---

## 2. Command Reference & Execution Modes

| Command | Scope | What It Runs |
| :--- | :--- | :--- |
| **`/benchmark full`** (or `/benchmark all`) | **Full Suite (Recommended)** | 1) Ring 0 Guard 50 Challenge Suite<br>2) 5 Golden Set Tasks (Bare vs Epic A/B)<br>3) Master comparison report |
| **`/benchmark smoke`** | 5 Golden Set Tasks | Runs A/B evaluation across `task1` ~ `task5` |
| **`/benchmark task <name>`** | Single Task | Runs A/B evaluation on a specific task (e.g. `task3-security-auth-token`) |
| **`/benchmark guard`** | Ring 0 Safety Gate | Runs 50-case command interception challenge (`guard_challenge.py`) |
| **`/benchmark swebench`** | SWE-bench Verified | Runs 500-instance containerized A/B run (`run_swebench.sh`) |
| **`/benchmark report`** | Reporting Only | Reads `DIRECTOR-REPORT.md` and provides executive insights |

---

## 3. Sandbox Environment Manifest & File Copy Specifications

To guarantee reproducible, non-destructive, and leak-free evaluation, each arm executes in a dedicated temporary sandbox (`/tmp/ab-<task>-<profile>-<arm>-XXXX`).

### 3.1 Sandbox Copy Manifest (File Whitelist / Blacklist)

| Category | File / Directory | Bare Arm (Unassisted) | Epic Arm (Harness Loaded) | Purpose |
| :--- | :--- | :---: | :---: | :--- |
| **Codebase** | `tasks/<name>/repo/*` | **COPIED** | **COPIED** | Base source files and initial failing test fixtures |
| **Prompt** | `tasks/<name>/task.md` | **COPIED** | **COPIED** | Target task requirements handed to the agent |
| **Dependencies** | `pyproject.toml`, `Cargo.toml`, etc. | **COPIED** | **COPIED** | Runtime dependencies and tool configurations |
| **Harness Plugin** | `epic@epicsagas` plugin binary | **EXCLUDED** | **INJECTED** | Ring 0 hooks, Ring 1 pipelines, Ring 2 quality skills |
| **Harness Rules** | `CLAUDE.md`, `AGENTS.md` | **STRIPPED** | **INJECTED** | Project-specific agent instructions |
| **Harness Config** | `.harness/guard-rules.yaml` | **STRIPPED** | **INJECTED** | Custom guard intercept rules |
| **Agent Configs** | `.claude/`, `.agents/`, `.codex/` | **STRIPPED** | **CONFIGURED** | Host settings and plugin toggles |

### 3.2 Bare Arm Baseline Configuration
- **Total Isolation**: Before launching Bare, the director explicitly deletes any `.harness/`, `.claude/`, `.agents/`, `CLAUDE.md`, and `AGENTS.md` files from the sandbox workdir.
- **Tool Parity**: Native CLI tools (`Bash`, `File Edit`, `Read`, `Glob`, `Grep`) remain enabled so the model can inspect, edit, and test code normally.
- **Invocation**: Executed with `--bare` flag (`claude --bare -p "$(cat task.md)"`) to suppress all plugins and hooks.

### 3.3 Anti-Tampering Grader Protocol
- **Test Integrity Protection**: After the agent completes its run, the grading runner **restores the original unmodified test files (`test_*.py`, `*_test.*`) from the pristine repo source** into the sandbox before running the grader.
- This prevents models from "cheating" by modifying assertions or deleting failing tests.


---

## 4. Execution Procedure for Director Session (TUI)

### Step 1: Parse User Request
Determine:
- Target mode: `full` (default), `smoke`, `guard`, or specific `task <name>`.
- Model profile: `zai` (default), `native`, `claude`, etc.
- Flags: `--dry-run` if testing pipeline mechanics without API calls.

### Step 2: Execute Director Runner
Run the appropriate Python orchestration script in the terminal:

```bash
# 1. Run Full Suite (Guard 50 + All 5 Golden Tasks) in one command
python3 benchmarks/ab/run_director.py --full --profile {profile}

# 2. Run Guard 50 Challenge only
python3 benchmarks/ab/guard_challenge.py

# 3. Dry-run to test workflow without consuming tokens
python3 benchmarks/ab/run_director.py --full --dry-run
```

### Step 3: Parse Results & Synthesize Executive Briefing
Present a structured report in the TUI containing:
1. **Guard Interception Summary**: Score (target: 50/50, 100%) and category breakdown.
2. **Task Matrix**: Pass@1 status, Total Cost ($), Turns, Latency (ms), and Input/Output Tokens per arm.
3. **Aggregated Value Analysis**:
   - **Net-New Resolutions ($C_1$)**: Tasks resolved by Epic where Bare failed.
   - **Cost-Per-Resolved-Instance (CPRI)**: Value-normalized cost.
   - **Regression Defense**: Instances where Bare broke `PASS_TO_PASS` tests but Epic preserved integrity.
4. **Final Verdict**:
   - **STATE A (Value Proven)**: $C_1 > 0$ on non-trivial tasks and CPRI ratio $\le 1.5$.
   - **STATE B (Cost Exceeds Value)**: Zero net-new gain with increased token cost.
   - **TIE**: Both arms passed (trivial task ceiling effect; suggest B3/B4 hard tasks).

---

## 5. Golden Set Reference

| Task Name | Category | Tested Skills & Capabilities |
| :--- | :--- | :--- |
| `task1-palindrome` | Basic Bugfix | Single-function debugging (`string_utils.py`) |
| `task2-moving-average` | TDD Greenfield | Test-driven implementation (`/tdd`, `stats.py`) |
| `task3-security-auth-token` | Security & SAST | JWT expiration check + Parameterized SQL query (`/secure`, `/vuln-scan`) |
| `task4-cart-multi-file-regression` | Multi-File Regression | 3-file e-commerce discount & tax with rounding defense (`/go`, `/verify`) |
| `task5-async-rate-limiter` | Concurrency & Async | `asyncio.Lock` race-condition resolution (`/debug`, `/perf`) |
| `guard_challenge.py` | Ring 0 Safety | 50 destructive, credential-dump, and infra commands |

---

## 6. Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
| :--- | :--- | :--- |
| "Bare is cheaper and passes easy tasks" | Easy tasks have a ceiling effect where value cannot be measured | Test on multi-file regression (`task4`) or hard security tasks (`task3`) |
| "A/B testing consumes too many tokens" | Unmeasured harness is unproven overhead | Run `--dry-run` first, or run Guard 50 (zero LLM token cost) |
| "Tests pass so code is safe" | Functional unit tests miss SQL injection and token forgery | Inspect SAST security scan results |

---

## 7. Evidence Required

- [ ] `run_director.py` output captured (or `guard_challenge.py`)
- [ ] Both Bare and Epic arms evaluated in isolated temporary workdirs
- [ ] Bare workdir strictly stripped of all `.harness/`, `.claude/`, `CLAUDE.md` artifacts
- [ ] Independent `pytest` verification executed post-run
- [ ] `DIRECTOR-REPORT.md` generated and presented to user
