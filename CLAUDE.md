# epic-harness

6 commands + auto-trigger skills + self-evolving agent harness.

## Structure

- `commands/` — 6 slash commands (spec, go, check, ship, team, evolve)
- `skills/` — 8 auto skills + _dispatch engine
- `agents/` — 4 internal agents (builder, reviewer, auditor, planner)
- `hooks/` — Ring 0 automation + Ring 3 evolution loop
  - `hooks/bin/epic-harness` — Rust single binary (primary, ~4x faster)
  - `hooks/scripts/*.js` — Node.js fallback (compiled from `src/ts/`)
- `src/hooks/` — Rust source (common, guard, observe, polish, resume, snapshot, reflect)
- `src/ts/` — TypeScript source (Node.js fallback)
- `presets/` — Cold-start skill templates (embedded in Rust binary at compile time)
- `references/` — Checklists (security, performance, testing, team-patterns)
- `integrations/` — Per-tool integration files (6 tools):
  - `codex/` — hooks.json, config.toml, prompts/(6), skills/(7), agents/(4)
  - `gemini/` — settings.json, GEMINI.md, commands/(6), skills/(7), agents/(4)
  - `cursor/` — hooks.json, commands/(6), agents/(4)
  - `opencode/` — commands/(6), agents/(4), plugins/epic-harness.js
  - `cline/` — hooks/(5 scripts), rules/epic-harness.md
  - `aider/` — .aider.conf.yml, .aider/CONVENTIONS.md

## Architecture: 4-Ring Model

- **Ring 0 (Autopilot)**: Hooks auto-maintain quality, restore sessions, learn
- **Ring 1 (Commands)**: 6 user-invoked commands
- **Ring 2 (Auto Skills)**: Context-triggered skills fire automatically
- **Ring 3 (Evolve)**: Observe → Analyze → Evolve → Gate → Reload self-improvement loop

## Eval System (Ring 3 Core)

Fuses A-Evolve benchmark patterns into Claude Code context.

### Multi-Dimensional Scoring
Every tool call scored on 3 axes:
- `tool_success` (0/1): Did the tool succeed?
- `output_quality` (0.0-1.0): Output quality (per-tool criteria)
- `execution_cost` (0.0-1.0): Efficiency
- **Composite**: `SCORE_WEIGHTS.success×tool_success + SCORE_WEIGHTS.quality×quality + SCORE_WEIGHTS.cost×cost` (default 0.5/0.3/0.2)

All weights configurable via `SCORE_WEIGHTS` in `common.js`.

### Failure Classification (9 types)
type_error, syntax_error, test_fail, lint_fail, build_fail, permission_denied, timeout, not_found, runtime_error

### Pattern Detection (4 types)
All thresholds defined as constants in `common.js` for per-project tuning.
Function-name-level context included (extracted from stack traces, error messages).
Error message hash-based dedup for improved precision (`hashString` + `normalizeError`).
- `repeated_same_error`: Consecutive same error + same error hash (`REPEATED_ERROR_MIN`, default 3)
- `fix_then_break`: Edit success → Bash error cycle (`FTB_LOOKAHEAD`=3, `FTB_MIN_CYCLES`=2)
- `long_debug_loop`: Same file in consecutive operations (`DEBUG_LOOP_MIN`, default 5)
- `thrashing`: Edit↔Error alternating (`THRASH_MIN_EDITS`=3, `THRASH_MIN_ERRORS`=3)

### Skill Seeding Thresholds
- Weak tool: success rate < `WEAK_TOOL_RATE`(0.6), min `WEAK_TOOL_MIN_OBS`(5) observations
- Weak file type: success rate < `WEAK_EXT_RATE`(0.5), min `WEAK_EXT_MIN_OBS`(3) observations
- High-frequency error: `HIGH_FREQ_ERROR_MIN`(5)+ occurrences

### Stagnation Gating
- `STAGNATION_LIMIT`(3) sessions without improvement → auto-rollback evolved skills to best checkpoint
- `IMPROVEMENT_THRESHOLD`: 5%
- Trend tracking: improving / stable / declining

### Evolved Skill Validation
Auto-validated by `gate_skills()` in reflect:
- Must have `---` frontmatter delimiter
- Body (after frontmatter) must be ≥ 20 characters
- SKILL.md file must exist in skill directory
- Invalid skills silently removed; skill count capped at `MAX_EVOLVED_SKILLS`(10)

### Evolved Skill Priority
Static skills (tdd, debug, secure, etc.) always take priority over evolved skills. Evolved skills supplement only.

### Skill Structure
All static skills include 4 core sections:
- **Process**: Step-by-step execution procedure
- **Anti-Rationalization**: Excuse | Rebuttal | What to do instead (table)
- **Evidence Required**: Checklist of proof needed for completion claims
- **Red Flags**: Anti-pattern warnings

## Concurrent Session Safety

Obs files use `session_{date}_{pid}_{random}.jsonl` format for per-session isolation.
Reflect merges all same-day session files for analysis.

## Cold-Start Presets

On first session with no evolved skills, stack-appropriate preset skills auto-apply for detected stacks (Node.js/Go/Python/Rust).

## Guard Rule Extension

Add custom block/warn rules via `.harness/guard-rules.yaml`:
```yaml
blocked:
  - pattern: kubectl\s+delete  | msg: kubectl delete blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — check first
```

## Cross-Project Learning

Opt-in by creating `.harness/.cross-project-enabled`.
On session end, patterns export to `~/.harness-global/patterns.jsonl`.
On next session start, weak patterns from other projects shown as hints.

## Skill Attribution

`metrics.json` tracks per-evolved-skill A/B scores:
- `avg_score_with`: Average score in sessions where skill was active
- `avg_score_without`: Average score in sessions where skill was absent
- Positive delta = effective, negative delta = consider removing

## Polish → Observe Feedback

Polish hook (format/typecheck) results auto-record into observe pipeline.
Format failure = lint_fail, typecheck failure = build_fail — feeds into pattern detection.

## Dispatch Logging

Skill dispatches logged to `.harness/dispatch/dispatch_YYYYMMDD.jsonl`.
Analyze via `/evolve history`.

## Project Side Data

`.harness/` directory accumulates per-project memory, observations, evolved skills:
- `memory/` — Project patterns and rules
- `sessions/` — Session snapshots
- `obs/` — Tool usage observation logs (JSONL, 3-axis scores)
- `evolved/` — Auto-evolved skills (pattern/tool/filetype/error based)
- `evolved_backup/` — Best-state backup (for stagnation rollback)
- `team/` — /team outputs
- `dispatch/` — Skill dispatch logs (JSONL)
- `metrics.json` — Aggregate stats (score_history, trend, stagnation_count, skill_attribution)
- `evolution.jsonl` — Evolution history (SessionAnalysis + patterns)
- `guard-rules.yaml` — Custom guard rules (optional)
- `.cross-project-enabled` — Cross-project learning opt-in marker (optional)

`.harness/` auto-created on session start.
