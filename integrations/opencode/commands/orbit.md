---
description: "Complete orbit - autonomous spec through ship, max 3 check retries, choose interactive or council mode"
---

CRITICAL: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

Autonomous pipeline: spec → go → check → ship. All tasks run sequentially.

**Ask the user which mode:**
1. **Interactive** — user does `/discover` + `/spec` manually, then says "orbit go"
2. **Council auto-spec** — 4 voices (Architect, Skeptic, Pragmatist, Critic) analyze request and generate spec, user approves

**After spec approved (interactive or council):**
- Create feature branch from `goal_slug`
- Plan tasks from spec Requirements, execute one at a time (TDD)
- Run full test suite, verify Acceptance Criteria
- Check: code review + security + performance + test coverage + spec coverage
- On FAIL: fix and re-check, max 3 retries then pause for user
- On PASS: isolated build+test, git hygiene, create PR, watch CI

Pipeline state tracked in `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json`.

Produce consolidated report with all phase summaries at end.

"One orbit complete. Run `/evolve` to analyze observations."
