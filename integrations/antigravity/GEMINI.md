# epic-harness

Self-evolving agent harness with 4-ring automation.

**CRITICAL**: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

## 4-Ring Model

- **Ring 0 (Autopilot)**: Hooks auto-maintain quality, restore sessions, learn
- **Ring 1 (Skills)**: 22 skills — manually invokable and context-triggered
- **Ring 2 (Auto Skills)**: Context-triggered skills fire automatically
- **Ring 3 (Evolve)**: Observe → Analyze → Evolve → Gate → Reload self-improvement loop

## Skills

All skills are namespaced under `/harness:` (e.g., `/harness:orbit`).

| Skill | Auto-trigger | Purpose |
|-------|-------------|---------|
| orbit | No | Full autonomous pipeline (spec → go → check → ship) |
| evolve | No | Analyze session observations, auto-evolve skills |
| team | No | Agent team design and management |
| spec | Yes | Generate spec with Requirements + Acceptance Criteria |
| go | Yes | Build phase — plan tasks, execute with TDD |
| check | Yes | Parallel review + audit + test verification |
| ship | Yes | Integration test → PR → CI watch |
| tdd | Yes | New feature or bug fix — Red → Green → Refactor |
| debug | Yes | Test failure, runtime error, unexpected behavior |
| secure | Yes | Auth, DB, API, or secrets code touched |
| verify | Yes | Before marking done or shipping |
| document | Yes | Public API/function/module added or changed |
| perf | Yes | Loops, DB queries, rendering, or batch ops |
| simplify | Yes | File >200 lines, high complexity, or duplication |
| council | Yes | Architecture decisions with significant trade-offs |
| commit | Yes | Conventional Commits generation |
| context | Yes | Session restoration from snapshots |
| reflect | Yes | AI usage review and scoring |
| discover | Yes | Problem discovery — 5 Whys, JTBD, Socratic |
| orchestrate | Yes | Multi-agent orchestration status and control |
| agent-introspection | Yes | Failure recovery on 3+ consecutive errors |

## Hooks

Lifecycle hooks run automatically:

- **SessionStart**: Restores previous session context + loads project memory
- **PreToolUse**: Blocks dangerous commands (force push, prod DB drop, rm -rf /)
- **PostToolUse**: Records results for evolution loop + auto-format/typecheck
- **Stop**: Analyzes session for evolution patterns

## Memory (harness-mem)

Shared knowledge graph via MCP tools: `mem_recall`, `mem_add`, `mem_search`, `mem_list`, `mem_context`, `mem_related`.

Use `mem_context` at session start. Use `mem_add` for decisions (importance=0.9).
