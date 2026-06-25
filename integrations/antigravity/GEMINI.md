# epic-harness

Self-evolving agent harness with 4-ring automation.

**CRITICAL**: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

## 4-Ring Model

- **Ring 0 (Autopilot)**: Hooks auto-maintain quality, restore sessions, learn
- **Ring 1 (Commands)**: 3 slash commands via `/harness:orbit`, `/harness:evolve`, `/harness:team`
- **Ring 2 (Auto Skills)**: Context-triggered skills fire automatically
- **Ring 3 (Evolve)**: Observe → Analyze → Evolve → Gate → Reload self-improvement loop

## Commands

All commands are namespaced under `/harness:` (e.g., `/harness:orbit`).

| Command | Purpose |
|---------|---------|
| `/harness:evolve` | Analyze session observations, auto-evolve skills |
| `/harness:team` | Agent team design and management |
| `/harness:orbit` | Full autonomous pipeline (spec → go → audit → ship) |

## Auto Skills

These skills activate automatically when relevant context is detected:

- **spec**: Generate spec with Requirements + Acceptance Criteria
- **go**: Build phase — plan tasks, execute with TDD
- **check**: Parallel review + audit + test verification
- **ship**: Integration test → PR → CI watch
- **tdd**: New feature or bug fix — Red → Green → Refactor
- **debug**: Test failure, runtime error, unexpected behavior
- **secure**: Auth, DB, API, or secrets code touched
- **verify**: Before marking done or shipping
- **document**: Public API/function/module added or changed
- **perf**: Loops, DB queries, rendering, or batch ops
- **simplify**: File >200 lines, high complexity, or duplication
- **council**: Architecture decisions with significant trade-offs
- **commit**: Conventional Commits generation
- **context**: Session restoration from snapshots
- **reflect**: AI usage review and scoring
- **discover**: Problem discovery — 5 Whys, JTBD, Socratic
- **orchestrate**: Multi-agent orchestration status and control
- **agent-introspection**: Failure recovery on 3+ consecutive errors

## Hooks

Lifecycle hooks run automatically:

- **SessionStart**: Restores previous session context + loads project memory
- **PreToolUse**: Blocks dangerous commands (force push, prod DB drop, rm -rf /)
- **PostToolUse**: Records results for evolution loop + auto-format/typecheck
- **Stop**: Analyzes session for evolution patterns

## Memory (harness-mem)

Shared knowledge graph via MCP tools: `mem_recall`, `mem_add`, `mem_search`, `mem_list`, `mem_context`, `mem_related`.

Use `mem_context` at session start. Use `mem_add` for decisions (importance=0.9).
