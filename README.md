# epic harness

**6 commands. Auto-trigger skills. Self-evolving.**

<p align="center">
<a href="README.md">English</a> | <a href="docs/ja/README.md">日本語</a> | <a href="docs/ko/README.md">한국어</a> | <a href="docs/de/README.md">Deutsch</a> | <a href="docs/fr/README.md">Français</a> | <a href="docs/zh-CN/README.md">简体中文</a> | <a href="docs/zh-TW/README.md">繁體中文</a> | <a href="docs/pt-BR/README.md">Português</a> | <a href="docs/es/README.md">Español</a> | <a href="docs/hi/README.md">हिन्दी</a>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-Apache_2.0-blue.svg" alt="License"></a>
  <img src="https://img.shields.io/badge/Version-0.1.0-brightgreen.svg" alt="Version">
  <img src="https://img.shields.io/badge/Claude_Code-Plugin-purple.svg" alt="Claude Code Plugin">
  <img src="https://img.shields.io/badge/Architecture-4_Ring-orange.svg" alt="4-Ring Architecture">
  <img src="https://img.shields.io/badge/Mode-Self_Evolving-green.svg" alt="Self Evolving">
  <a href="https://buymeacoffee.com/epicsaga"><img src="https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black" alt="Buy Me a Coffee"></a>
</p>

A Claude Code plugin that **replaces 30+ commands with 6**, **auto-triggers skills** based on what you're doing, and **evolves new skills** from your own failure patterns. Less surface area to memorize. More intelligence per keystroke.

<p align="center">
  <img src="./assets/features.jpg" alt="epic harness features" width="100%" />
</p>

## Architecture: 4-Ring Model

```
Ring 0 — Autopilot (hooks, invisible)
  Session restore, auto-format, guard rails, observation logging

Ring 1 — 6 Commands (you call these)
  /spec  /go  /check  /ship  /team  /evolve

Ring 2 — Auto Skills (context-triggered)
  tdd · debug · secure · perf · simplify · document · verify · context

Ring 3 — Evolve (self-improving)
  Observe tool usage → analyze failures → auto-generate skills → gate → reload
```

## Install

```bash
# Claude Code plugin CLI
claude plugins marketplace add epicsagas/epic-harness
claude plugins install epic@epicsagas

# Or manually
git clone https://github.com/epicsagas/epic-harness.git ~/.claude/plugins/epic
```

### Rust binary (optional, ~4x faster hooks)

```bash
# Homebrew (macOS)
brew install epicsagas/tap/epic-harness

# From crates.io
cargo install epic-harness

# Pre-built binary (faster, no compile)
cargo binstall epic-harness

# From source
cargo install --path .
```

The binary is automatically detected by hooks. If absent, hooks fall back to Node.js.

## Multi-Tool Support

epic-harness works with Claude Code and 6 additional AI coding tools. All tools share the same `~/.harness/projects/{slug}/` data directory.

| Tool | Ring 0 Hooks | Commands/Prompts | Skills | Agents |
|------|-------------|------------------|--------|--------|
| **Claude Code** | ✓ Full | ✓ 6 commands | ✓ 8 skills | ✓ 4 |
| **Codex CLI** | ✓ Full¹ | ✓ 6 prompts | ✓ 7 (`~/.agents/skills/`) | ✓ 4 |
| **Gemini CLI** | ✓ Partial² | ✓ 6 commands | ✓ 7 | ✓ 4 |
| **Cursor** | ✓ Full³ | ✓ 6 commands | ✓ via rules | ✓ 4 |
| **OpenCode** | ✓ Partial⁴ | ✓ 6 commands | — | ✓ 4 |
| **Cline** | ✓ Full⁵ | — | — | — |
| **Aider** | —⁶ | — | — | — |

¹ Requires `codex_hooks = true` in `~/.codex/config.toml`; PostToolUse intercepts Bash only  
² No `PreToolUse` equivalent — guard runs at `BeforeModel` level  
³ Requires Cursor 1.7+  
⁴ JS plugin: `session.created` / `tool.execute.before` / `tool.execute.after` / `session.idle`  
⁵ PreToolUse / PostToolUse / TaskStart / TaskResume / TaskCancel hook scripts  
⁶ No hook system — conventions injected via `.aider/CONVENTIONS.md` + `.aider.conf.yml`

### Install for other tools

```bash
# Interactive menu (select which tools to install)
epic-harness install

# Direct install
epic-harness install codex        # Codex CLI   → ~/.codex/ + ~/.agents/skills/
epic-harness install gemini       # Gemini CLI  → ~/.gemini/
epic-harness install cursor       # Cursor      → ~/.cursor/ (requires Cursor 1.7+)
epic-harness install opencode     # OpenCode    → ~/.config/opencode/
epic-harness install cline        # Cline       → ~/Documents/Cline/Rules/
epic-harness install aider        # Aider       → ~/.aider.conf.yml + ~/.aider/

# Project-local install
epic-harness install cursor --local

# Preview without changes
epic-harness install gemini --dry-run
```

Integration files in the tool directory (`hooks.json`, commands, agents, skills, rules, …) are **synced** from the binary: missing or outdated files are written. `GEMINI.md` and `AGENTS.md` are only created when absent.

## Commands

| Command | What it does |
|---------|-------------|
| `/spec` | Define what to build — clarify requirements, produce a spec |
| `/go` | Build it — auto-plan, TDD subagents, parallel execution |
| `/check` | Verify — parallel code review + security audit + performance |
| `/ship` | Ship — PR, CI, merge |
| `/team` | Design project-specific agent team |
| `/evolve` | Manual evolution trigger / status / rollback |

## Auto Skills (Ring 2)

Skills trigger automatically based on context. You don't need to invoke them.

| Skill | Triggers when |
|-------|--------------|
| **tdd** | New feature implementation |
| **debug** | Test failure or error |
| **secure** | Auth/DB/API/secrets code touched |
| **perf** | Loops, queries, rendering code |
| **simplify** | File > 200 lines or high complexity |
| **document** | Public API added or changed |
| **verify** | Before completing /go or /ship |
| **context** | Context window > 70% used |

## Hooks (Ring 0)

Run invisibly. No user action needed. Implemented as a **single Rust binary** (`epic-harness`) with subcommands, falling back to Node.js if the binary is not available.

```
epic-harness resume | guard | polish | observe | snapshot | reflect
```

| Hook | When | Does |
|------|------|------|
| **resume** | Session start | Restore context, load memory, detect stack |
| **guard** | Before Bash | Block force-push-to-main, rm -rf /, DROP prod |
| **polish** | After Edit | Auto-format (Biome/Prettier/ruff/gofmt) + typecheck |
| **observe** | Every tool use | Log to `~/.harness/projects/{slug}/obs/` for evolution |
| **snapshot** | Before compact | Save state to `~/.harness/projects/{slug}/sessions/` |
| **reflect** | Session end | Analyze failures, seed evolved skills, gate |

## Eval System (Ring 3 Core)

Fuses A-Evolve's benchmark patterns into Claude Code's hook system.

### Multi-Dimensional Scoring

Every tool call is scored on 3 axes. Weights are configurable via `SCORE_WEIGHTS` in `src/ts/common.ts` (or `src/hooks/common.rs`):

```
composite = SCORE_WEIGHTS.success × tool_success + SCORE_WEIGHTS.quality × output_quality + SCORE_WEIGHTS.cost × execution_cost
           (default: 0.5)                          (default: 0.3)                             (default: 0.2)
```

| Dimension | What it measures | Per-tool criteria |
|-----------|-----------------|-------------------|
| `tool_success` | Did it work? (0/1) | 9-category failure classification |
| `output_quality` | Output quality signals (0.0-1.0) | Bash: warnings, empty output. Edit: re-edit detection |
| `execution_cost` | Efficiency proxy (0.0-1.0) | Output size, silent-success command whitelist |

### Failure Classification (9 categories)

`type_error` · `syntax_error` · `test_fail` · `lint_fail` · `build_fail` · `permission_denied` · `timeout` · `not_found` · `runtime_error`

### Pattern Detection (4 types)

All thresholds are configurable constants in `src/ts/common.ts` (or `src/hooks/common.rs`):

| Pattern | Detects | Constant | Default |
|---------|---------|----------|---------|
| `repeated_same_error` | Same error N+ times in a row | `REPEATED_ERROR_MIN` | 3 |
| `fix_then_break` | Edit succeeds → build/test fails | `FTB_LOOKAHEAD` / `FTB_MIN_CYCLES` | 3 / 2 |
| `long_debug_loop` | Stuck on same file N+ operations | `DEBUG_LOOP_MIN` | 5 |
| `thrashing` | Edit↔Error alternating on same file | `THRASH_MIN_EDITS` / `THRASH_MIN_ERRORS` | 3 / 3 |

### Skill Seeding Thresholds

| Trigger | Constant | Default |
|---------|----------|---------|
| Weak tool (low success rate) | `WEAK_TOOL_RATE` / `WEAK_TOOL_MIN_OBS` | 0.6 / 5 |
| Weak file type | `WEAK_EXT_RATE` / `WEAK_EXT_MIN_OBS` | 0.5 / 3 |
| High-frequency error | `HIGH_FREQ_ERROR_MIN` | 5 |

### Stagnation Gating

- `STAGNATION_LIMIT` (default: 3) sessions without improvement → auto-rollback evolved skills to best checkpoint
- `IMPROVEMENT_THRESHOLD` (default: 5%)
- Trend tracking: `improving` / `stable` / `declining` via linear regression
- Static skills always take priority over evolved skills on conflict

### Evolution Flow

```
Observe (PostToolUse — 3-axis scoring)
    ↓ ~/.harness/projects/{slug}/obs/session_{id}.jsonl
Analyze (SessionEnd)
    ↓ SessionAnalysis: per-tool, per-ext, score distribution
    ↓ Patterns: repeated_same_error, fix_then_break, long_debug_loop, thrashing
Seed (4 paths: pattern / weak tool / weak file type / high-freq error)
    ↓ ~/.harness/projects/{slug}/evolved/{skill}/SKILL.md
Gate (format check, dedup, cap of 10, stagnation check)
    ↓ ~/.harness/projects/{slug}/evolved_backup/ (best checkpoint)
Reload (next session — resume.ts reports metrics + loads evolved skills)
```

```bash
/evolve              # Run evolution now
/evolve status       # Dashboard: scores, trends, patterns, skills
/evolve history      # Long-term analysis: full history, skill effectiveness, dispatch stats
/evolve cross-project # Cross-project pattern analysis
/evolve rollback     # Restore previous best
/evolve reset        # Clear all evolution data
```

## Cold-Start Presets

No need to wait 5 sessions for useful evolved skills. On first session, epic harness detects your stack and applies preset skills automatically:

| Stack | Preset Skills |
|-------|--------------|
| Node.js/TypeScript | `evo-ts-care`, `evo-fix-build-fail` |
| Go | `evo-go-care` |
| Python | `evo-py-care` |
| Rust | `evo-rs-care` |

Presets are supplements — they get replaced by real evolved skills as data accumulates.

## Concurrent Session Safety

Each session writes to its own observation file (`session_{date}_{pid}_{random}.jsonl`). Multiple Claude Code sessions on the same project won't corrupt each other's data. The reflect hook merges all same-day files for analysis.

## Custom Guard Rules

Add project-specific safety rules via `.harness/guard-rules.yaml` in your project root:

```yaml
blocked:
  - pattern: kubectl\s+delete\s+namespace | msg: Namespace deletion blocked
  - pattern: terraform\s+destroy | msg: Terraform destroy blocked
warned:
  - pattern: docker\s+system\s+prune | msg: Docker prune — verify first
```

Rules merge with built-in guards (force-push-to-main, rm -rf /, DROP prod). Keeping this file in git allows sharing safety rules with your team.

## Cross-Project Learning

Opt-in to share failure patterns across projects:

```bash
touch ~/.harness/projects/{slug}/.cross-project-enabled  # opt-in
```

When enabled:
- Session end exports anonymized patterns to `~/.harness/global_patterns.jsonl`
- Session start shows hints from other projects' weak areas
- Use `/evolve cross-project` to see aggregate patterns

## Skill Effectiveness Tracking

Every evolved skill is tracked with A/B attribution scores:

```
/evolve history → Skill Effectiveness section

| Skill              | Sessions | Score With | Score Without | Delta  |
|--------------------|----------|------------|---------------|--------|
| evo-ts-care        | 8        | 0.87       | 0.72          | +15%   |
| evo-bash-discipline| 3        | 0.65       | 0.68          | -3%    |
```

Positive delta = skill helps. Negative delta = consider removing via `/evolve rollback`.

## Polish → Observe Feedback

The polish hook (auto-format + typecheck) feeds results back into the observation pipeline:

- Format failure → recorded as `lint_fail`
- TypeScript error → recorded as `build_fail`
- Successes → recorded with full scores

This means "edit → type error → edit → type error" thrashing patterns get detected even when the errors come from the polish hook, not manual commands.

## Project Data (`~/.harness/projects/{slug}/`)

Project-specific data lives in your home directory. This survives project deletion and doesn't pollute your git history.

```
~/.harness/projects/{slug}/
├── memory/           # Project patterns and rules (persistent)
├── sessions/         # Session snapshots (for resume)
├── obs/              # Tool usage observation logs (JSONL, per-session)
├── evolved/          # Auto-evolved skills
├── evolved_backup/   # Best checkpoint (for stagnation rollback)
├── dispatch/         # Skill dispatch logs (JSONL)
├── team/             # /team generated agents and skills
├── evolution.jsonl   # Full evolution history
└── metrics.json      # Aggregate stats + skill attribution
```

You can still use `.harness/guard-rules.yaml` in the project root if you want to share safety rules with your team.

## Development

### Rust (primary — ~4x faster)

```bash
cargo install --path .          # Build + install to ~/.cargo/bin/
cp ~/.cargo/bin/epic-harness hooks/bin/epic-harness  # Update plugin binary
```

### Node.js (fallback)

```bash
npm install
npm run build    # TypeScript (src/ts/) → hooks/scripts/*.js
```

### How hooks are dispatched

Each hook in `hooks.json` looks for the Rust binary in three places, then falls back to Node.js:

```
1. Plugin local: hooks/bin/epic-harness
2. PATH:         ~/.cargo/bin/epic-harness (via cargo install)
3. Fallback:     node hooks/scripts/<hook>.js
```

### Tests

```bash
cargo test       # 98 Rust unit tests
npm test         # Node.js unit + e2e tests
```

## Acknowledgments

epic harness was inspired by and built upon ideas from the following projects:

- [a-evolve](https://github.com/A-EVO-Lab/a-evolve) — Automated evolution and benchmark patterns
- [agent-skills](https://github.com/addyosmani/agent-skills) — Claude Code agent skill system
- [everything-claude-code](https://github.com/affaan-m/everything-claude-code) — Comprehensive Claude Code patterns
- [gstack](https://github.com/garrytan/gstack) — Plugin architecture reference
- [harness](https://github.com/revfactory/harness) — Hook and harness infrastructure patterns
- [serena](https://github.com/oraios/serena) — Autonomous agent design
- [SuperClaude Framework](https://github.com/SuperClaude-Org/SuperClaude_Framework) — Multi-command framework architecture
- [superpowers](https://github.com/obra/superpowers) — Claude Code extension patterns

## License

[Apache 2.0](LICENSE)
