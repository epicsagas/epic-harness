---
description: "Complete orbit — autonomous spec through ship in one shot"
---

# /orbit — Complete Orbit

CRITICAL: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

Autonomous pipeline: spec → go → check → ship. All phases run sequentially (Gemini does not support parallel agents).

**Mode selection** — ask the user:
1. **Interactive**: User runs `/discover` → `/spec` manually, then says "orbit go"
2. **Council auto-spec**: 4-voice analysis generates spec, user approves

**Council mode** (if chosen):
- Ask each voice sequentially: Architect → Skeptic → Pragmatist → Critic
- Each gets ONLY the request + codebase context (anti-anchoring)
- Synthesize → generate spec → user approves/rejects

**After spec approved:**
- Plan tasks from Requirements, execute one at a time (TDD: red → green → refactor)
- Run full test suite after all tasks
- Check: review code quality, security, performance, test coverage, spec coverage
- On FAIL: fix and re-check, max 3 retries. After 3, pause for user decision
- On PASS: isolated build+test, git hygiene, create PR via `gh pr create`, watch CI

**Pipeline state**: `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json` — update after each phase.

**Report**: Consolidated phase summary with spec, branch, PR URL, check retries.

"One orbit complete. Run `/evolve` to analyze observations."
