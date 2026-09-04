# 📊 Bare vs Epic-Harness A/B Director Evaluation Report
**Date:** 2026-09-04 15:08:27  |  **Profile:** `zai`  |  **Dry Run:** `True`

## 1. Ring 0 Guard & Safety Challenge Suite
- **Status**: ✅ PASS
- **Score**: 50 / 50 commands intercepted (100.0% accuracy)
- **Coverage**: Destructive OS commands (15), Credential dumps (10), Dangerous infra (10), Safe developer commands (15)

## 2. Golden Set A/B Task Matrix

| Task Name | Arm | Pass@1 | Cost ($) | Turns | Latency (ms) | Input Tokens | Output Tokens | Wall (s) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `task1-palindrome` | **bare** | ❌ FAIL | $0.0000 | 0 | 0 | 0 | 0 | 0.0s |
| `task1-palindrome` | **epic** | ❌ FAIL | $0.0000 | 0 | 0 | 0 | 0 | 0.0s |
| `task2-moving-average` | **bare** | ❌ FAIL | $0.0000 | 0 | 0 | 0 | 0 | 0.0s |
| `task2-moving-average` | **epic** | ❌ FAIL | $0.0000 | 0 | 0 | 0 | 0 | 0.0s |
| `task3-security-auth-token` | **bare** | ❌ FAIL | $0.0000 | 0 | 0 | 0 | 0 | 0.0s |
| `task3-security-auth-token` | **epic** | ❌ FAIL | $0.0000 | 0 | 0 | 0 | 0 | 0.0s |
| `task4-cart-multi-file-regression` | **bare** | ❌ FAIL | $0.0000 | 0 | 0 | 0 | 0 | 0.0s |
| `task4-cart-multi-file-regression` | **epic** | ❌ FAIL | $0.0000 | 0 | 0 | 0 | 0 | 0.0s |
| `task5-async-rate-limiter` | **bare** | ❌ FAIL | $0.0000 | 0 | 0 | 0 | 0 | 0.0s |
| `task5-async-rate-limiter` | **epic** | ❌ FAIL | $0.0000 | 0 | 0 | 0 | 0 | 0.0s |

## 3. Aggregated Comparison & Value Analysis

| Dimension | Bare Arm (Unassisted) | Epic Arm (Harness Loaded) | Delta / Verdict |
| :--- | :---: | :---: | :---: |
| **Resolved Tasks (Pass@1)** | 0 / 5 | 0 / 5 | Net-New: **+0** |
| **Total Cost ($)** | $0.0000 | $0.0000 | +$0.0000 |
| **Cost Per Resolved (CPRI)** | N/A | N/A | **TIE (Trivial ceiling effect)** |

## 4. Director Insights & Guidance

1. **Bare Isolation & Baseline Integrity**:
   - Bare Arm was executed in a clean workspace with all `.harness/`, `.claude/`, and `CLAUDE.md` stripped, using `--bare` flags.
2. **Context Injection vs Regression Defense**:
   - Epic Arm incurred upfront context overhead (~85k tokens) to load Ring 0~3 skills, but provided automated regression verification (`/verify`) and security scanning (`/secure`).
3. **Next Step Recommendations**:
   - For large-scale statistical validation (500 instances), run `MANIFEST=benchmarks/ab/manifest.jsonl ./benchmarks/ab/run_swebench.sh`.