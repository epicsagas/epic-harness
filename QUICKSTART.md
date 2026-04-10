# epic-harness — Quick Start

5 minutes from zero to your first self-evolving Claude Code session.

## Prerequisites

- [Claude Code](https://docs.claude.com/en/docs/claude-code) installed
- Git

## Install

```bash
# Via Claude Code plugin marketplace
/plugin marketplace add epicsagas/epic-harness
/plugin install harness@epic
```

Or manually:

```bash
git clone https://github.com/epicsagas/epic-harness.git ~/.claude/plugins/epic
```

The Rust binary handles all hooks. If you also want to install for other tools (Codex, Gemini, Cursor, OpenCode, Cline, Aider), run `epic-harness install` for an interactive menu.

## First Session

1. **Open any project** in Claude Code. epic-harness auto-detects the stack (Node, Go, Python, Rust, …) and initializes your data directory in `~/.harness/projects/{slug}/` on the first session.

2. **Try a command:**

   ```
   /spec   # describe what you want to build
   /go     # let it build
   /check  # parallel review + security + perf audit
   /ship   # PR + CI + merge
   ```

3. **Skills trigger themselves.** When you touch auth code, the `secure` skill activates. When tests fail, `debug` kicks in. You don't call them.

## Verify

After your first session ends, check evolution data (it's in your home directory, not the project root):

```bash
ls ~/.harness/projects/
# The directory name matches your project directory name (slugified)

/evolve status   # see your scores, trends, evolved skills
```

If `metrics.json` exists and `obs/session_*.jsonl` is non-empty, observation is working.

## What Happens Next

- **Session 1–2**: epic-harness watches and learns. No evolved skills yet.
- **Session 3+**: Failure patterns are detected. New skills are seeded into `~/.harness/projects/{slug}/evolved/` and gated.
- **After stagnation**: If 3 sessions show no improvement, evolved skills auto-rollback to the last best checkpoint.

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| Hooks not running | Verify the `epic-harness` binary is in PATH (`which epic-harness`); Node.js fallback used if absent |
| `~/.harness/projects/` not created | Restart Claude Code session (resume hook initializes it) |
| `/evolve status` empty | Need at least 1 completed session first |

## Next Steps

- Create `.harness/guard-rules.yaml` in your project root to share safety rules with your team.
- Read [README.md](README.md) for the full architecture
- See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup
- Report issues: [GitHub Issues](https://github.com/epicsagas/epic-harness/issues)
