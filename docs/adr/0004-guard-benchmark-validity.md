# ADR 0004: The guard benchmark drives the shipped binary and reports false positives

- Status: Proposed
- Date: 2026-11-16
- Area: D (issue #274; PR #373)
- Spec: `docs/specs/0.9.0-pipeline.md`, section D
- Literature: `docs/references/research-0.9.0-pipeline.md`, Area D

## Context

All references are to main at 74424a2.

- The native guard blocks three things by default: force push to main or master (checked in code, `src/hooks/guard.rs:127-133`), `rm -rf /` and `DROP DATABASE|TABLE ... prod` (`src/hooks/guard.rs:13-22`). It also blocks commit messages that are not Conventional Commits (`src/hooks/guard.rs:459-464`). It warns on force push, `git reset --hard`, and `rm -rf` (`src/hooks/guard.rs:98-111`). Projects add their own rules through `guard-rules.yaml` (`src/hooks/guard.rs:466-492`).
- Verdicts are observable on the process boundary: exit code 2 means block; exit 0 with a stderr line `[guard] WARNING: ...` means warn; exit 0 without it means allow (`src/hooks/guard.rs:453-500`; `hint` writes to stderr, `src/shared/helpers.rs:161-165`). The hook reads a JSON payload on stdin (`src/main.rs:165-195`). `EPIC_HOOK_PROFILE` selects the profile (`src/shared/types.rs:76-105`); the guard runs under every profile.
- `benchmarks/ab/guard_challenge.py` classifies its 50 cases with its own Python regex list (`benchmarks/ab/guard_challenge.py:78-116`) and never runs the binary. It validates a surrogate. Its dataset (`:17-75`) has 25 must-block, 10 must-warn and 15 must-allow cases, and contains a real user-home path literal on line 22.
- `README.md:468` advertises "50/50 (100%) Interception" and "0% false positives". `benchmarks/ab/run_director.py:67-97` parses the script's "Results: X/Y Passed" line, and `docs/harness-evaluation-plan.md:174` and `:229` describe the suite.
- The audit (`.lead/audit/all.json`, PR 373) confirms the surrogate and finds the PR's rewrite to spawn the binary correct, but rejects its 13 new blocking and 5 new warning default rules as an unrequested security-policy change with false-positive risk for every user. Recommendation: split.

## Decision

Adopt memo option D1:

1. `guard_challenge.py` runs the real `epic-harness guard` binary once per case, with stdin JSON, an isolated `HOME`, `HARNESS_DIR`, and working directory, and maps exit code plus stderr to block, warn, allow, or error.
2. The existing 50-case dataset stays as the challenge set, labeled with the desired policy. The user-home literal is replaced by a synthetic path.
3. A new benign corpus (hand-curated, synthetic, at least 40 commands developers run daily, including near misses for every native rule) is added. The report carries false-block and false-warn rates next to recall.
4. Block and warn are reported separately. The headline recall metric is "must-block cases blocked"; warn is not counted as interception (Decision needed D-1).
5. The script fails (exit 1) when it executes zero cases, when any case errors, or when any benign case is blocked. Low recall on the challenge set is reported, not failed: it measures the default policy, it does not gate it.
6. README and the evaluation plan state the measured native numbers. `run_director.py` reads the script's `--json` output.
7. No guard policy change. New rules, if wanted, ship later as opt-in presets (memo option D2), each paired with allow-cases in the benign corpus, under a separate maintainer decision.

## Alternatives considered

- D3, PR 373 as proposed (new default rules). Rejected: changes default policy for every user without false-positive evidence (audit); a guard is a defense-in-depth layer, and OS sandboxing does the heavy lifting (memo Area D item 3).
- Keep the Python surrogate and only fix the README. Rejected: the oracle would remain a reimplementation of the unit under test, a circular oracle (memo Area D item 7).
- Gate the benchmark on challenge recall. Rejected: with the honest default rule set, recall is low by design, so the gate would be permanently red and would push toward adding default rules to make it green.
- Build the benign corpus from observe logs. Rejected for 0.9.0: real commands need masking and may contain restricted data; a synthetic corpus has none of that risk. Offered as Decision needed D-2.

## Consequences

- The README headline drops from 50/50 to the measured native numbers. That is the intended correction.
- The benchmark needs a built binary (`cargo build --release`) and exits 2 with a clear message if it is missing.
- `run_director.py`'s guard step reports measured numbers and treats only script failures (exit not 0) as FAIL.
- PowerShell and Windows command forms remain thinly covered (two benign cases); a dedicated Windows corpus is future work.

## Migration and compatibility

- No change to the binary, config keys, DB, or stored files.
- Script CLI: new `--binary`, `--json`, `--suite` and `--self-test` flags; running it without flags still prints a human-readable report.

## Rollback

Revert the commit. The surrogate script and the old README row come back. Nothing persisted depends on it.
