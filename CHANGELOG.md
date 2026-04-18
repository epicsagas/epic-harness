# Changelog

All notable changes to epic-harness will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
