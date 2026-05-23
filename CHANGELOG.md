# Changelog

All notable changes to epic-harness will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- CLI: `mem delete` renamed to `mem remove` (old name works as deprecated alias)
- CLI: `mem query` renamed to `mem list` (old name works as deprecated alias)

## [0.4.2] — 2026-05-24

### Added
- Antigravity (Gemini CLI fork) integration: extension manifest, hooks, skills, commands
- `_dispatch` skill: core router that auto-invokes matching skills on context signals
- `agent-introspection` skill: failure recovery on 3+ consecutive errors
- `reflect` skill: 5-dimension evidence-based self-assessment
- `orchestrate` skill: multi-agent orchestration status and control
- `discover` skill: problem discovery via 5 Whys, JTBD, Socratic method
- `context` skill: session restoration from snapshots

### Changed
- Consolidated `commands/`, `skills/`, `agents/` into unified `registry/` structure (#26)
- Replaced gemini-cli integration with antigravity plugin
- Fixed 16 accuracy issues across skills, commands, presets, and integrations
- Optimized 12 skill/command description fields (~236 tokens saved)
- Fixed `_dispatch` and `reflect` referencing non-existent `mem_query` → `mem_list`
- Fixed `orchestrate` inbox path accuracy
- Fixed `orbit` evolve trigger logic for CI failures
- Fixed `evolve` cross-project path and `resume.ts` → `resume.rs`
- All preset skills now include required `## Red Flags` section
- Added 4 structural sections (Process, Anti-Rationalization, Evidence Required, Red Flags) to `_dispatch`

### Fixed
- Orchestrate test race condition with `serial_test::serial`
- ARM64 Linux CI build failure: lowered `rust-version` to 1.87.0
- ARM64 CI runner updated to `ubuntu-24.04-arm` via cargo-dist custom runners

## [0.3.3] — 2026-05-09

### Added
- Agent judgment known issues section in all READMEs
- Version bump checklist in AGENTS.md (Cargo.toml, package.json, plugin.json, git tag)

### Changed
- Reorganized project structure: `commands/`, `skills/`, `agents/`, `presets/` → `registry/`; docs → `docs/`
- Renamed syntagma references to episteme across docs and build
- `/orbit` isolates pipeline in git worktree to prevent session conflicts
- AGENTS.md synced with latest project content (replaced outdated template)
- CLAUDE.md consolidated to reference AGENTS.md
- Marketplace.json descriptions updated (6 → 8 commands)
- 9 i18n READMEs added (de, es, fr, hi, ja, ko, pt-BR, zh-CN, zh-TW)

## [0.2.1] — 2026-04-25 [YANKED]

> **YANKED**: This release wrote Claude hook commands containing `${CLAUDE_PLUGIN_ROOT}` into global `~/.claude/settings.json`.  
> That variable is only available in plugin-scoped hooks, causing runtime hook errors in global settings.

### Fixed
- **`epic install claude` now installs hooks**: Claude integration now syncs hook definitions into `~/.claude/settings.json` (from embedded `hooks/hooks.json`) instead of reporting zero installable files, so PreToolUse/PostToolUse/Session hooks are actually applied after install.

### Changed
- Install help/list output now includes `claude` in both install and uninstall integration lists.
- Added regression tests for Claude install behavior:
  - `settings.json` hook merge semantics are validated.
  - Claude canonical file generation remains explicitly empty (`generate_canonical_files("claude")`).

## [0.2.0] — 2026-04-19

### Added
- **Unified Memory system** (`harness mem`): cross-agent knowledge graph stored in `~/.harness/memory.db` (SQLite + FTS5), shared by all supported coding agents
  - 15 CLI subcommands: `add`, `edit`, `delete`, `query`, `search`, `related`, `link`, `graph`, `export`, `serve`, `validate`, `migrate`, `context`, `mcp`, `mcp-install`
  - **SQLite + FTS5 storage**: single `~/.harness/memory.db` — fast full-text search, ACID transactions, WAL concurrency; no rg/grep subprocess
  - **Auto-migration**: legacy `nodes/*.md` + `edges.jsonl` automatically imported into SQLite on first run
  - **`mem export`**: dumps all nodes to `~/.harness/exports/<id>.md` for Git-diffable plain-text backup; supports `--out <dir>` and `--dry-run`
  - Knowledge graph: typed nodes (concept/pattern/project/decision/error) + directed edges (uses/extends/conflicts/replaces/related/caused_by)
  - Web UI: `harness mem serve` → `http://localhost:7700` — D3.js force-directed graph, realtime search, CRUD, EN/KO language toggle, dark theme
  - **MCP server** (`epic-harness mem mcp`): native Rust stdio JSON-RPC 2.0 server, 5 tools — no Node.js required; register via `harness mem mcp-install [--force]`
  - Auto-recording: PostToolUse hook detects decisions/patterns → auto-stores (fire-and-forget, secret-masked)
  - Session context injection: relevant project memories injected at session start via `resume` hook
  - Migration: `harness mem migrate --all [--dry-run]` converts existing per-project memories to unified store
  - Security: 127.0.0.1 binding, UUID v4 strict path validation, secret masking, sensitive file path filtering
  - > ⚠️ **This feature is still in development.** Some capabilities described above may not yet be functional.
- **opencode integration**: JS plugin (`plugins/epic-harness.js`) for session/tool lifecycle hooks, 6 commands, 4 agents → `~/.config/opencode/`
- **cline integration**: 5 executable hook scripts (PreToolUse/PostToolUse/TaskStart/TaskResume/TaskCancel) → `~/Documents/Cline/Rules/Hooks/`
- **aider integration**: `.aider.conf.yml` + `.aider/CONVENTIONS.md` — no hook system, conventions auto-loaded via `read:` config
- **Interactive install menu**: `epic-harness install` (no args) shows numbered checklist; select by number (e.g. `1,3`) or `a` for all
- **Progress bar**: TTY shows animated `[====>   ] N/M filename`; non-TTY shows one-line summary per tool
- **`epic team` / `epic org`**: org-level agent team management — persistent team definitions that accumulate knowledge across projects
  - 9 CLI subcommands: `list`, `show`, `status`, `sync`, `link`, `unlink`, `delete`, `history`, `help`
  - Interactive team designer with auto stack detection (Rust/Node/Python/Go/Java)
  - Agent CRUD with mission, playbook (append-only), and history backup
  - Cross-tool sync: copies agents to `.claude/agents/{team}/` per project; `--global` for `~/.claude/agents/`
  - Project linking: `epic team link` binds team to current project; `status` shows linked teams
  - Security: path traversal prevention (`[a-zA-Z0-9_-]` allowlist), YAML injection defense (`yaml_quote`), Unicode prompt injection stripping (Plane-14, C0/C1 controls), HTML comment sanitization, ANSI escape filtering
  - Org browsing: `epic org` to browse team libraries across organizations

### Changed
- **`epic install claude` plugin cache sync**: now overwrites `~/.claude/plugins/cache/epicsagas/epic/*/` with the commands/skills/agents embedded in the binary — local changes take effect immediately without waiting for an npm publish
- **Codex skills path**: now installs to `~/.agents/skills/` (official Codex discovery path; was `~/.codex/skills/` which is not scanned)
- **Codex commands**: renamed `commands/` → `prompts/` → `~/.codex/prompts/`; invoke as `/prompts:check` etc.
- **Codex hooks**: require `features.codex_hooks = true` in `~/.codex/config.toml` (off by default); install now writes this config and warns if an existing config lacks the flag
- **Codex PostToolUse**: removed non-functional Edit/Write matchers (Codex only intercepts Bash); removed unsupported `async` flag
- Install output: replaced per-file `println!` with progress bar; errors still surfaced via `eprintln!`

### Removed
- `integrations/antigravity/` — Antigravity is an IDE-level setting; file-based install is not the right mechanism
- `install.sh` — fully superseded by `epic-harness install` Rust subcommand

## [0.1.3] — 2026-04-09

### Fixed

- Shell injection vulnerability in hook command dispatch
- Guard rule matching consistency across blocked/warned rule evaluation
- Reflect analysis correctness (session scoring, trend calculation edge cases)

## [0.1.2] — 2026-04-09

### Fixed

- Plugin install commands corrected from `/plugin` to `claude plugins` CLI syntax in all docs

## [0.1.1] — 2026-04-09

### Added

- Multi-language README for top 10 Claude Code countries (10 locales)
- npm publish step in release CI workflow
- Linux arm64 binary target in release builds
- `cargo install` and `cargo binstall` install methods documented
- Homebrew tap (`epicsagas/tap`) integration in CI and install docs

### Fixed

- Hook dispatch now checks `PATH` before falling back to Node.js scripts
- Homebrew tap path shortened to `epicsagas/tap` across all i18n READMEs
- Broken CI badge removed from all READMEs
