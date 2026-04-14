# epic-harness

**CRITICAL**: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

This project uses the **epic-harness** automation layer. The hooks in
`.clinerules/hooks/` (or `~/Documents/Cline/Rules/Hooks/`) run automatically
around every tool call. Here is what they do:

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
| `/spec` | Define requirements before coding |
| `/go` | Build with auto-plan + TDD sub-agents |
| `/check` | Parallel review + security + tests |
| `/ship` | Create PR, verify CI, merge |
| `/evolve` | Inspect or trigger skill evolution |
| `/team` | Generate project-specific agent team |

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
