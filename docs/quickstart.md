# epic-harness — Quick Start

5 minutes from zero to your first self-evolving Claude Code session.

## Prerequisites

- [Claude Code](https://docs.claude.com/en/docs/claude-code) installed
- Git
- [Rust toolchain](https://rustup.rs) (for source/binary install — plugin marketplace doesn't need this)

## Install

epic-harness ships as a **plugin** — skills, hooks, and the `harness-mem` MCP server load directly from the plugin layout. There is no `install` step; the plugin self-seeds `~/.harness/config.toml` and `HARNESS.md` on the first session.

### Claude Code (recommended)

```
/plugin marketplace add epicsagas/plugins
/plugin install epic@epicsagas
```

The binary is auto-installed and all hooks register in one step.

### agy (Antigravity CLI)

```bash
agy plugin install .   # from a clone, or via marketplace
```

### Codex CLI

```bash
codex plugin marketplace add epicsagas/plugins
```

### Binary-only (no plugin host)

```bash
brew install epicsagas/tap/epic-harness      # macOS / Linux
cargo binstall epic-harness                  # or build from source
```

> **Telemetry**: anonymous usage reporting is on by default (opt-out). Disable with `epic-harness telemetry off` — see the README [Telemetry](../../README.md#telemetry) section for what is collected.

## First Session

1. **Open any project** in Claude Code. epic-harness auto-detects the stack (Node, Go, Python, Rust, …) and initializes your data directory in `~/.harness/projects/{slug}/` on the first session.

2. **Try a command:**

   **Manual pipeline** — step through each phase yourself:
   ```
   /discover   # explore and define the problem (optional — for vague or unfocused requests)
   /spec       # describe what you want to build
               # → if spec has 3+ requirements and no team linked, suggests /team
   /go         # let it build (uses worktree isolation for parallel conflicting tasks)
   /audit      # parallel review + security + perf audit
   /ship       # isolated pre-flight test → PR + CI + merge
               # → on completion, suggests /evolve to improve skills for the next cycle
   ```

   **Or use `/orbit`** — runs spec → go → audit → ship autonomously in one command:
   ```
   /orbit
   # Choose mode:
   #   1. Interactive  — you run /discover + /spec, then say "orbit go"
   #   2. Council auto-spec — 4-voice council generates spec, you approve
   # After spec approval: fully autonomous until PR is merged
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
| Hooks not running | Verify the `epic` binary is in PATH (`which epic`); reinstall the plugin (`/plugin install epic@epicsagas`) and restart Claude Code |
| `~/.harness/projects/` not created | Restart Claude Code session (resume hook initializes it) |
| `/evolve status` empty | Need at least 1 completed session first |

## Next Steps

- Create `.harness/guard-rules.yaml` in your project root to share safety rules with your team.
- Read [README.md](README.md) for the full architecture
- See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup
- Report issues: [GitHub Issues](https://github.com/epicsagas/epic-harness/issues)
