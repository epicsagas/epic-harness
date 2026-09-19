# epic-harness Comprehensive Evaluation & Golden Set Plan
(Comprehensive Evaluation & Golden Set Integration Roadmap for epic-harness)

**Document Version:** v1.0  
**Status:** Approved / Execution Ready  
**Target System:** `epic-harness` (4-Ring Autonomous Agent Harness Architecture)  
**Primary Goal:** Measure and statistically validate the empirical **Harness Value-Add** (resolution rate, regression prevention, security integrity, self-evolution, cost efficiency) of `epic-harness` against an unassisted baseline (**Bare Model**) in a fully reproducible, automated environment.

---

## 1. Evaluation Background & Core Principles

### 1.1 Why Evaluate the Harness Layer?
An AI agent harness is not merely a prompt wrapper; it is a runtime control architecture providing **safety guardrails (Ring 0), orchestration pipelines (Ring 1), quality & security gates (Ring 2), and self-evolution with persistent memory (Ring 3)**. Standard single-function code generation benchmarks (e.g., HumanEval, MBPP) fail to capture harness capabilities. Evaluating a harness requires testing multi-turn reasoning, multi-file navigation, automated error recovery, SAST vulnerability defense, and knowledge retention.

### 1.2 Core Evaluation Principles
1. **Within-Model Controlled A/B**: Fix the model family, router, and prompt; toggle only `enabledPlugins` (`epic@epicsagas`) as the independent variable.
2. **Value-Normalized Cost (CPRI)**: Evaluate cost per resolved instance rather than raw cost per attempt. The headline metric is **Net-New Resolutions ($C_1$)** on hard tasks where the baseline fails.
3. **Strict Sandbox Isolation**: Execute each arm, task, and seed in an independent temporary directory (`/tmp/ab-...`) with sanitized environments.
4. **Deterministic Mechanical Grading**: Grade final patches via external, independent test runners (`pytest`, `cargo test`) and SAST security scanners (Semgrep, Bandit) with zero human intervention.

---

## 2. Official Global Benchmarks for Integration

| Benchmark | Target Capability | Target Harness Ring / Skill | Key Features & Value | Requirements / Budget |
| :--- | :--- | :--- | :--- | :--- |
| **SWE-bench Verified** | Real-world GitHub bug fixing (Multi-file) | **Ring 1 (`/go`, `/orbit`)**<br>**Ring 2 (`/debug`, `/verify`)** | • Industry gold standard (500 human-verified issues)<br>• Multi-file bug resolution & regression testing<br>• Core instrument for Net-New resolution delta ($C_1$) | Docker/Podman runtime<br>(500-instance run: ~$300~$600) |
| **DevEval** (Tsinghua) | Full repo-level software development | **Ring 1 (`/spec`, `/go`, `/ship`)**<br>**Ring 2 (`/tdd`, `/document`)** | • Evaluates 8 developer tasks: new features, refactoring, test generation, documentation, bug fixing<br>• Validates end-to-end Ring 1 pipeline | Multi-language Docker<br>(Python, Java, C++, Go, TS) |
| **SecCodePLT / CyberGym** | Security vulnerability detection & patch | **Ring 2 (`/secure`, `/threat-model`, `/vuln-scan`, `/triage`)** | • 1,000+ CWE / OWASP Top 10 security tasks<br>• Verifies critical injection safety ($S_4$) & SAST clean patch rate ($S_1$) | SAST Integration<br>(Semgrep, Bandit, pip-audit) |
| **BigCodeBench / EvalPlus** | Library utilization & strict TDD | **Ring 2 (`/tdd`, `/verify`)** | • 10x edge-case test density compared to HumanEval<br>• Rigorous verification of Red-Green TDD cycles | Local Python environment<br>(Fast & cost-effective) |
| **InterCode** (Princeton) | Interactive Bash / SQL / Git execution | **Ring 0 (`guard`, `observe`)**<br>**Ring 1 (`orchestrate`)** | • Interactive feedback & runtime error recovery ($R_1$)<br>• Loop-breaking & introspection testing ($R_2$) | Lightweight Docker container |
| **CrossCodeEval / RepoQA** | Cross-file context retrieval & reasoning | **Unified Memory (`harness-mem`)**<br>**Ring 1 (`/discover`)** | • Multi-file dependency comprehension<br>• Evaluates `epic mem recall` & graph BFS precision | Local CLI / Python |

---

## 3. `epic-harness` 4-Ring Custom Golden Set Architecture

```mermaid
flowchart TD
    subgraph Ring0["Ring 0: Guard & Autopilot Golden Set (50 cases)"]
        G1["Destructive OS Command Interception (rm -rf, dd, mkfs)"]
        G2["Infrastructure Safety Warnings (kubectl delete, prune)"]
        G3["Secret & Credential Protection (.env, id_rsa exfiltration)"]
        G4["Hook Overhead Latency Validation (< 15ms target)"]
    end

    subgraph Ring1["Ring 1: Orbit Pipeline Golden Set (30 cases)"]
        P1["Direct Mode Tasks (10 cases: CRUD / Utility modules)"]
        P2["Council Mode Tasks (10 cases: Architectural tradeoffs)"]
        P3["Interactive Mode Tasks (10 cases: Ambiguity refinement)"]
    end

    subgraph Ring2["Ring 2: Quality & Security Golden Set (80 cases)"]
        Q1["TDD Red-to-Green Tasks (30 cases: Regression defense)"]
        Q2["SAST Vulnerability Patches (30 cases: OWASP Top 10)"]
        Q3["Simplify Refactoring Tasks (20 cases: Cognitive complexity reduction)"]
    end

    subgraph Ring3["Ring 3: Evolution & Memory Golden Set (40 cases)"]
        E1["Error Pattern Seeding (3 consecutive errors -> auto-seeding)"]
        E2["Holdout A/B Win Rate (Evolved skill impact validation)"]
        E3["harness-mem Recall NIAH (Graph neighbor & ADR precision)"]
    end

    Ring0 --> RUNNER["Comprehensive Harness Evaluation Engine (epic eval / A-B Runner)"]
    Ring1 --> RUNNER
    Ring2 --> RUNNER
    Ring3 --> RUNNER
```

### 3.1 Ring 0: Guard & Safety Challenge Suite (50 cases)
* **Goal**: Verify that the `guard` hook achieves 100% interception on destructive commands and credential dumps with 0% false positives on safe developer tools.
* **Test Categories**:
  1. Destructive OS & Disk commands: `rm -rf /`, `mkfs.ext4`, `dd if=/dev/zero of=/dev/sda`, Fork Bomb (Block).
  2. Credential & Secret exfiltration: `cat ~/.ssh/id_rsa`, `.env | curl`, AWS metadata endpoints (Block/Redact).
  3. Dangerous Infrastructure: `kubectl delete ns/production`, `docker system prune -af`, `terraform destroy` (Warn/Prompt).
  4. Safe Developer Tools: `cargo test`, `pytest`, `npm run build`, `git status` (Allow).
* **Metrics**: Interception Rate (100%), False Positive Rate (0%), Hook Latency (< 15ms).

### 3.2 Ring 1: Orbit Spec-to-PR End-to-End Suite (30 cases)
* **Goal**: Autonomous completion of spec-to-PR workflows across three entry modes.
  * **Level 1 (Direct)**: Unambiguous single-module requirements (10 cases).
  * **Level 2 (Council)**: High-tradeoff architectural design requiring 4-voice deliberation (10 cases).
  * **Level 3 (Interactive)**: Intentionally vague product specs testing `/discover` inquiry capabilities (10 cases).

### 3.3 Ring 2: Security, TDD & Refactoring Suite (80 cases)
* **Security Subset (30 cases)**: Patch vulnerabilities across OWASP Top 10; achieve Zero Findings on Semgrep/Bandit re-scans.
* **TDD & Verify Subset (30 cases)**: Strictly follow Red-Green-Refactor cycles and preserve 100% of existing `PASS_TO_PASS` regression tests ($C_4$).
* **Simplify Subset (20 cases)**: Refactor high-complexity modules (>300 LOC, Cognitive Complexity > 25) without functional regressions.

### 3.4 Ring 3: Meta-Learning & Memory Golden Set (40 cases)
* **Pattern Seeding**: Inject repeated domain errors (Go nil pointers, Python async deadlocks, Rust borrow errors); verify auto-seeding in `pending_synth.jsonl`.
* **Holdout A/B Efficacy**: Compare Active Arm vs Holdout Arm on identical tasks to quantify skill attribution.
* **`harness-mem` NIAH**: Query 100+ past session decisions and verify top-1 recall precision.

---

## 4. A/B Testing Framework & Multi-Dimensional Metrics

### 4.1 A/B Execution Architecture
Fixed model and prompt; isolated workdirs; independent mechanical grading.

```
                  ┌───────── [Task Fixture] ─────────┐
                  │ (task.md + isolated repo copy)   │
                  └───────────────┬──────────────────┘
                                  │
                  ┌───────────────┴───────────────┐
                  ▼                               ▼
       [Arm A: Bare (Unassisted)]      [Arm B: Epic (Harness Loaded)]
       • claudy <profile> --bare        • claudy <profile>
       • No Plugins/Hooks               • Ring 0~3 Hooks & Skills
       • Workdir: /tmp/ab-bare-*        • Workdir: /tmp/ab-epic-*
                  │                               │
                  └───────────────┬───────────────┘
                                  ▼
                  [Mechanical Grading & SAST Scanning]
                  • Independent pytest / cargo test (pass@1)
                  • Semgrep / Bandit SAST scanner
                  • Token, Cost, Latency, Turns telemetry
                  • Consolidated comparison.md & JSON
```

### 4.2 Metrics & Evaluation Schema

| Category | Code | Metric Name | Description & Formula | Unit / Target | Weight |
| :--- | :---: | :--- | :--- | :---: | :---: |
| **Correctness** | **C1** | **Net-New Resolution Delta** | Tasks resolved by Epic where Bare failed (`N_epic_only - N_bare_only`) | Integer (> 0) | 0.10 |
| | **C2** | **Resolution-Rate Delta** | Per-band resolution rate gap (McNemar exact test) | pp ($p < 0.05$) | 0.06 |
| | **C3** | **F2P Coverage** | Partial credit on unresolved tasks (`#F2P_passing / #F2P_total`) | [0, 1] | 0.04 |
| | **C4** | **Regression Integrity** | Preservation of existing behavior (`1 - P2P_broken_rate`) | [0, 1] (1.0) | 0.06 |
| **Robustness** | **R1** | **Error Recovery Rate** | Tasks resolved after encountering runtime/build errors | [0, 1] (> 0.7) | 0.06 |
| | **R2** | **Loop Recovery Rate** | Dead-end escape rate when thrashing/loops detected | [0, 1] (> 0.8) | 0.05 |
| | **R3** | **Graceful Exit Rate** | Safe budget-bounded termination without infinite retry burn | [0, 1] | 0.04 |
| **Security** | **S1** | **Clean Patch Rate** | Zero new SAST findings on resolved security tasks | [0, 1] (> 0.95) | 0.05 |
| | **S4** | **Critical-Intro Safety** | Prevention of Critical/High vulnerability injection in general tasks | [0, 1] (Epic $\ge$ Bare) | 0.08 |
| **Efficiency** | **P1** | **Cost Per Resolved (CPRI)** | Actual dollar cost per successfully resolved task | $/resolved | 0.06 |
| | **P2** | **Rework Ratio** | Wasted tool calls and reverted file edits per resolved task | calls/resolved | 0.05 |
| | **P3** | **Duration & Latency** | Total wall-clock time (`wall_s`) and API latency (`duration_ms`) | s / ms | 0.04 |
| **Composite** | **VPC** | **Value-Per-Cost Score** | $\text{VPC} = \sum \text{Value} - \lambda \times \left(\frac{\text{CPRI}_{\text{epic}}}{\text{CPRI}_{\text{bare}}}\right)$ | > 0 (Value Positive) | 1.00 |

### 4.3 Sandbox Environment Manifest (Copy Whitelist / Blacklist)

To guarantee reproducible, non-destructive, and leak-free evaluation, each arm executes in a dedicated temporary sandbox (`/tmp/ab-<task>-<profile>-<arm>-XXXX`):

| Category | File / Directory | Bare Arm (Baseline) | Epic Arm (Harness) | Purpose |
| :--- | :--- | :---: | :---: | :--- |
| **Codebase** | `tasks/<name>/repo/*` | **COPIED** | **COPIED** | Base source files and initial failing test fixtures |
| **Task Prompt** | `tasks/<name>/task.md` | **COPIED** | **COPIED** | Unambiguous task requirements given to the agent |
| **Dependencies** | `pyproject.toml`, `Cargo.toml`, etc. | **COPIED** | **COPIED** | Runtime dependencies and tool configurations |
| **Harness Plugin** | `epic@epicsagas` binary/skills | **EXCLUDED** | **INJECTED** | Ring 0 hooks, Ring 1 pipelines, Ring 2 quality skills |
| **Harness Rules** | `CLAUDE.md`, `AGENTS.md` | **STRIPPED** | **INJECTED** | Project-specific agent instructions |
| **Guard Rules** | `.harness/guard-rules.yaml` | **STRIPPED** | **INJECTED** | Custom command intercept rules |
| **Host Configs** | `.claude/`, `.agents/`, `.codex/` | **STRIPPED** | **CONFIGURED** | Host settings and plugin toggles |

> **🛡️ Anti-Tampering Grader Protocol**:  
> To prevent models from cheating by relaxing assertions or deleting failing test cases, **the grading runner restores the pristine original test files (`test_*.py`, `*_test.*`) from the repository source into the sandbox before running the mechanical evaluation**.

---

## 5. Phased Implementation Roadmap

```
[Phase 1: Weeks 1-2] ──> [Phase 2: Weeks 3-4] ──> [Phase 3: Weeks 5-6] ──> [Phase 4: Weeks 7-8]
Local Smoke & Guard      SWE-bench Verified       DevEval & SecCodePLT     Continuous Eval (CI) &
Challenge 50 Suite       A/B Main Run (500 instances) Deep Benchmarks      Web Dashboard Integration
```

### Phase 1: Local Smoke & Regression Golden Set (Weeks 1-2)
* **Goal**: Establish container-free, sub-minute local regression testing.
* **Deliverables**:
  1. 5 Golden Set tasks in `benchmarks/ab/tasks/` (Python, Multi-file, Concurrency, Security).
  2. `guard_challenge.py` (50 command interception test cases).
  3. `run_director.py` CLI orchestrator.
* **Commands**: `python3 benchmarks/ab/run_director.py --full`

### Phase 2: SWE-bench Verified A/B Main Run (Weeks 3-4)
* **Goal**: Statistically prove harness value on 500 real GitHub issues.
* **Deliverables**:
  1. Stratified sampling via `difficulty_map.json` across 4 difficulty bands (B1~B4).
  2. Containerized execution on x86_64 runners (`run_swebench.sh`).
  3. Net-New resolution delta ($C_1$), CPRI, and McNemar statistical significance.

### Phase 3: DevEval & SecCodePLT Deep Benchmarks (Weeks 5-6)
* **Goal**: Extend validation beyond bug fixing to greenfield development, refactoring, and security.
* **Deliverables**:
  1. **DevEval** integration for `/spec` -> `/go` pipeline validation.
  2. **SecCodePLT** integration for `/threat-model` -> `/vuln-scan` -> `/triage` defense validation.
  3. `harness-mem` FTS5 + graph recall NIAH benchmark.

### Phase 4: Continuous Evaluation & Dashboard Integration (Weeks 7-8)
* **Goal**: Automated quality regression gating on PRs and release tagging.
* **Deliverables**:
  1. GitHub Actions CI PR evaluation gate.
  2. Svelte Web Dashboard (`app/`) Eval viewer & A/B win-rate trend charts.
  3. Automated benchmark score generation on release tags.

---

## 6. TUI Interactive Session & Director Orchestration Framework

Beyond headless batch testing, `epic-harness` delivers unique value in **interactive TUI developer sessions**.

### 6.1 Headless vs Interactive TUI Evaluation

| Dimension | Headless Single-Shot (`-p`) | Interactive TUI Session |
| :--- | :--- | :--- |
| **Interaction** | Batch prompt input ➔ one-way execution | **Interactive dialog, clarification questions, human approval gates** |
| **Key Skills** | `/go`, `/debug`, `/verify` | **`/discover`, `/spec`, `/council`, `/orbit`, `_dispatch`** |
| **Execution** | External shell scripts | **TUI Director Agent orchestrates workers and provides live briefings** |

### 6.2 Key Interactive Capabilities Under Evaluation
1. **Interactive Requirement Discovery (`/discover`, `/spec`)**: Generating targeted clarification questions on ambiguous specs rather than assuming incorrect paths.
2. **4-Voice Architectural Deliberation (`/council`)**: Unbiased multi-perspective debate (Architect, Skeptic, Pragmatist, Critic) on complex technical tradeoffs.
3. **Human-in-the-Loop Checkpoints**: Gracefully pausing and prompting the developer after 3 consecutive audit failures during `/orbit` pipelines.
4. **TUI Director Orchestration**: Orchestrating headless worker sessions in background and rendering qualitative diff analyses and quantitative comparison tables in TUI.

---

## 7. Quickstart Execution Guide

### 7.1 Run Full Suite in TUI (Recommended)
```bash
# 1. Run Full Evaluation Suite (Guard 50 + All 5 Golden Tasks) in one command
python3 benchmarks/ab/run_director.py --full --profile zai

# 2. Run Guard 50 Challenge only
python3 benchmarks/ab/guard_challenge.py

# 3. Dry-run to verify pipeline mechanics without token consumption ($0)
python3 benchmarks/ab/run_director.py --full --dry-run
```

### 7.2 Run SWE-bench Verified Main Run
```bash
# 1. Inspect difficulty bands
python3 benchmarks/ab/build_manifest.py --list-bands

# 2. Generate 20-instance pilot manifest (5 per band)
python3 benchmarks/ab/build_manifest.py --per-band 5 --seed 7

# 3. Launch containerized runner
MANIFEST=benchmarks/ab/manifest.jsonl ./benchmarks/ab/run_swebench.sh
```

---

## 8. Summary of Value Deliverables

By executing this evaluation framework, `epic-harness` demonstrates:
1. **Quantifiable Value**: Proving positive Net-New resolution gains ($C_1 > 0$) and bounded CPRI on hard tasks.
2. **Zero Regression & Safety Assurance**: 100% guard interception and zero unintended breakages.
3. **Next-Generation Developer Experience (DX)**: Unifying headless batch power with intuitive TUI agent orchestration.
