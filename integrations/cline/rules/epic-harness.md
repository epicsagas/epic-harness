<!-- Canonical source: ~/.harness/HARNESS.md (managed by epic-harness). Keep in sync when updating harness instructions. -->
# epic-harness

**CRITICAL**: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

This project uses the **epic-harness** automation layer. The hooks in
`~/Documents/Cline/Hooks/` run automatically around every tool call.
Here is what they do:

## Automatic Behaviours

| Hook | When | Action |
|------|------|--------|
| `TaskStart` / `TaskResume` | Task begins or resumes | Restores evolved skills and prior session context |
| `PreToolUse` | Before shell commands | `guard` checks for dangerous patterns; blocks if exit 2 |
| `PostToolUse` | After every tool call | Records observation (tool success, quality score) |
| `TaskCancel` | Task cancelled | Triggers `reflect` to evolve skills in background |

## Slash Commands

Use these in your Cline chat:

| Command | Purpose |
|---------|---------|
| `/evolve` | Inspect or trigger skill evolution |
| `/team` | Generate project-specific agent team |
| `/orbit` | Autonomous spec→ship pipeline with council or interactive mode |

## Auto Skills

These skills activate automatically based on context signals:

| Skill | Purpose |
|-------|---------|
| `spec` | Define requirements before coding |
| `go` | Build with auto-plan + TDD sub-agents |
| `check` | Parallel review + security + tests |
| `ship` | Create PR, verify CI, merge |
| `tdd` | New feature or bug fix — Red → Green → Refactor |
| `debug` | Test failure, runtime error, unexpected behavior |
| `secure` | Auth, DB, API, or secrets code touched |
| `verify` | Before marking done or shipping |
| `document` | Public API/function/module added or changed |
| `perf` | Loops, DB queries, rendering, or batch ops |
| `simplify` | File >200 lines, high complexity, or duplication |
| `council` | Architecture decisions with significant trade-offs |
| `commit` | Conventional Commits generation |
| `context` | Session restoration from snapshots |
| `discover` | Problem discovery — 5 Whys, JTBD, Socratic |
| `orchestrate` | Multi-agent orchestration status and control |
| `agent-introspection` | Failure recovery on 3+ consecutive errors |
| `reflect` | AI usage review and scoring |

## ~/.harness/projects/{slug}/ Directory

Project-level memory lives in `$HARNESS_DIR/`:
- `obs/` — tool observation logs (scored by success, quality, cost)
- `evolved/` — auto-generated skills from your patterns
- `metrics.json` — session trends and skill effectiveness
- `guard-rules.yaml` — add custom block/warn shell patterns

## Commit — Conventional Commits Generator

**Always generate Conventional Commits format. Guard blocks non-CC messages.**

Format: `type(scope): description`
- Lowercase type, optional scope, imperative mood, no period, under 72 chars
- Breaking changes: append `!` before `:`

Valid types: `feat`, `fix`, `refactor`, `docs`, `test`, `build`, `chore`, `ci`, `style`, `perf`

Process:
1. `git status` + `git diff HEAD` + `git log --oneline -5` (parallel)
2. Determine type from the diff — the changes make the type obvious
3. Stage specific files (`git add <files>`, not `git add -A`)
4. `git commit -m "type(scope): description"` — execute automatically, no confirmation

Anti-patterns to reject:
- Vague messages: "update code", "fix stuff", "changes"
- Wrong type: `feat` for a bug fix, `fix` for a new feature
- Staging unrelated files
- Using `--no-verify` to bypass hooks

## Orbit — Autonomous Pipeline

Chains spec → go → check → ship in a single session.

**Two modes:**
- **Interactive**: User describes the problem, then spec/go/check/ship skills fire automatically
- **Council auto-spec**: 4-voice council (Architect, Skeptic, Pragmatist, Critic) analyzes the request and generates a spec. User approves or rejects.

**After spec approved**, runs autonomously:
1. Go: plan tasks, execute with TDD, integrate
2. Check: code review + security audit + test suite + spec coverage
3. On FAIL: auto-fix and re-check, max 3 cycles. Pauses for human input if all 3 fail.
4. Ship: isolated integration test, git hygiene, create PR, watch CI

**State tracking**: `$HARNESS_DIR/orbit/PIPELINE-{timestamp}.json`

**Human checkpoints:**
- Spec must be explicitly approved before autonomous execution begins
- 3 failed check cycles → pause for user decision (continue or abort)
