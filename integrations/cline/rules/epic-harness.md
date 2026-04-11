# epic-harness

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

Project-level memory lives in `$(epic-harness path)/`:
- `obs/` — tool observation logs (scored by success, quality, cost)
- `evolved/` — auto-generated skills from your patterns
- `metrics.json` — session trends and skill effectiveness
- `guard-rules.yaml` — add custom block/warn shell patterns
