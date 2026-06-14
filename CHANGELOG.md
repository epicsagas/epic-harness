# Changelog

All notable changes to epic-harness will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.4] — 2026-06-15

### Fixed
- **Observations lost project attribution**: `insert_observation_pool` dropped the `project` column, so ~30% of observations written since the per-project → global db consolidation had blank project. The project slug is now bound at the observe hook call site and threaded through the store layer.
- **`migrate` targeted the wrong database**: `open_migrate_conn` opened the per-project db instead of the global one, so `migrate` and `migrate --to-global` wrote to the wrong database. Now targets global `~/.harness/harness.db`. Removed the dead per-project `harness_db_path()`.
- **Sessions merge schema drift**: `merge_attached_db_async` referenced stale session columns (`snapshot_json`/`millis`); aligned with the current `pending_tasks`/`context_usage`/`created_at_millis` columns.
- **DDL divergence from production**: `DDL_SQLITE` was missing the `project` column on 7 tables (`sessions`, `evolution_records`, `metrics_state`, `score_history`, `orch_runs`, `orch_control`, `promotion_counters`); aligned so fresh dbs match the production global db schema.

## [0.6.3] — 2026-06-12

### Added
- **`epic slug` CLI subcommand**: prints the worktree-safe project slug; used in Cursor hooks.

### Fixed
- **Project slug in linked worktrees**: `project_slug()` now uses `git-common-dir` to resolve a stable slug in linked worktrees instead of CWD dirname.
- **Orchestration test flakiness**: added the `serial` attribute to env-mutating tests to prevent race conditions.
- **Episteme ingest**: disabled Episteme integration in reflect session metrics and removed the broken dependency.
- **src-tauri build**: prefixed unused project params to satisfy the compiler after signature changes.

### Changed
- sqlx requirement updated from 0.8 to 0.9.

## [0.6.2] — 2026-06-10

### Fixed
- **FTS5 search broken after rusqlite→sqlx migration**: `search` and `recall` commands returned empty results because FTS virtual table columns were inaccessible via `sqlx::AnyPool`
  - Two-step rowid query pattern: `SELECT rowid FROM nodes_fts WHERE MATCH ?` → `SELECT ... FROM nodes WHERE rowid IN (...)`
  - Schema DDL now includes `CREATE TRIGGER IF NOT EXISTS` for `nodes_ai/au/ad` to keep FTS index in sync automatically
  - `migrate_fts_schema()`: auto-detects old 3-column FTS tables and migrates to 6-column (id, title, body, tags, projects) with content=nodes
  - Removed manual `DELETE/INSERT INTO nodes_fts` from upsert/delete — triggers handle sync
- **src-tauri dashboard build broken after sqlx migration**: function signatures changed during migration but `src-tauri` (separate crate) wasn't updated
  - Added pool-only alias functions: `load_metrics_all_pool`, `list_recent_snapshots_all_pool`, `query_recent_records_all_pool`, `query_obs_stats_all_pool`, `list_all_pipelines_pool_limited`
  - Fixed call sites in `src-tauri/src/commands/harness.rs` to use new signatures
- **Duplicate exports in dashboard harness.ts**: removed duplicate `getOrchestratorRun` and `getOrchestratorAgentStatus` function blocks
- **`store::pool` visibility**: changed `pub(crate)` → `pub` for cross-crate access from `src-tauri`

## [0.6.1] — 2026-06-10

### Fixed
- **`cargo fmt` failure**: reformatted `detect_stack()` C# detection block and Java/Kotlin `pom.xml`/`build.gradle` condition in `src/eval/config.rs` to satisfy rustfmt

## [0.6.0] — 2026-06-09

### Added
- **Security pipeline (Ring 2)**: three new skills ported from [defending-code](https://github.com/anthropics/defending-code-reference-harness)
  - **threat-model**: trust boundary analysis, threat actor enumeration, threat scenario generation → `THREAT_MODEL.md`
  - **vuln-scan**: 4-dimension systematic scanner (injection, auth, data exposure, dependencies) → `VULN-FINDINGS.json`
  - **triage**: adversarial validation with severity adjustment, chaining analysis, root-cause grouping → `TRIAGE.json`
  - Pipeline flow: `/threat-model` → `/vuln-scan` → `/triage`
- **Prompt auto-tuning for evolved skills**: underperforming skills receive targeted tuning guidance based on A/B score gaps
  - Tuning sections appended after `<!-- auto-tuned -->` delimiter — original content never modified
  - Auto-rollback after 3 consecutive declining sessions (`TUNING_DECLINE_LIMIT`)
  - History tracked in `SkillMeta.prompt_tuning_history` (capped at 10 entries)
  - New functions: `auto_tune_skills()`, `append_tuning_section()`, `strip_tuning_sections()`, `build_tuning_section()`
  - 10 new unit tests covering serialization, scoring, decline counting
- **Audit `--strict` mode**: trust boundary isolation for reviewer/auditor independence
  - Artifact-only delivery: audit modes receive only diff + spec, no builder context
  - Cross-check independence: code/security/test modes run blind until synthesis
  - Blind scoring: prevents anchoring bias between modes
  - No self-review: builder session excluded from audit agent selection
  - Activation: `--strict` flag or `mode: strict` in `.harness/engagement.md`
- **Engagement context**: optional `.harness/engagement.md` for security assessment scoping
  - Defines: Authorization, Scope (in/out), Constraints, Environment, Exclusions
  - `secure` skill checks for engagement context and loads scope if present
  - Reference template: `docs/references/engagement.md`
- **Tiered verification ladder** in `/ship`: T0 (build) → T1 (tests+lint+fmt) → T2 (AC verification) → T3 (security)
  - T1/T2 auto-retry ≤3 times
  - T3 conditional on engagement.md or security-scope diff
- **Semantic deduplication** in `/audit`: cross-mode finding dedup between parallel checks and synthesis
  - NEW/DUP_BETTER/DUP_SKIP classification
  - Severity reassessment across modes (highest severity wins)
- **SkillOpt-inspired evolution optimization**: three deep learning-inspired techniques adapted from [SkillOpt](https://arxiv.org/abs/2605.23904) for natural language skill evolution
  - **Negative Feedback Buffer**: persists rejected skill proposals with TTL-based expiry to prevent re-generating known-bad skills
  - **Minibatch Reflection**: decomposes observations into fixed-size batches for structural pattern extraction, catching micro-patterns hidden by session averages
  - **Slow/Meta Update**: epoch classification (Improving/Regressing/PersistentFailure/StableSuccess) with slow parameter tracking per evolved skill
- Ineffective skill auto-eviction: skills that demonstrably lower session scores are automatically removed after 3+ sessions of negative attribution
- New config options: `rejected_buffer_ttl` (default: 10) and `minibatch_size` (default: 8) in `[evolution]` section
- New types: `RejectedEntry`, `MinibatchInsight`, `EpochClass` in `src/shared/evolution.rs`
- New functions: `analyze_minibatches()`, `classify_epoch()`, `update_meta_field()`, rejected buffer CRUD
- **Eval skill + CLI**: project quality & regression evaluation with 4 dimensions (correctness, performance, quality, regression)
  - `epic eval --init` scaffolds eval.yaml with auto-detected stack (Rust/Node/Python/Go/Java)
  - `epic eval --json` outputs structured results for CI pipelines
  - `epic eval --baseline-update` saves current run as baseline for regression comparison
  - LLM-as-judge integration in SKILL.md for quality dimension (deferred to LLM session)
  - Orbit integration: Step 5.5 Eval phase inserted automatically when eval.yaml exists
  - New modules: `src/eval/{mod,config,runner,baseline,report}.rs`

- **`eval` benchmark auto-detect and in-repo baselines** (#73)
  - `Benchmark` struct gains `command` and `result_type` fields; benchmarks are now executed
  - `eval --init` scans for `benchmarks/eval_runner.py`, `Makefile eval`, `justfile eval` and pre-populates `benchmarks:` list
  - Baselines default to in-repo `benchmarks/baselines/latest.json`; results save to `benchmarks/results/`
  - `result_type: composite` parses `composite`/`score` float from JSON stdout (e.g. Episteme eval_runner.py output)
  - Warns at runtime when benchmark infrastructure exists but `benchmarks:` is empty
  - `correctness` and `quality` dimensions no longer inject stack-based defaults (`cargo test`, `cargo clippy`) — eval delegates build/test/lint to `verify`

- **`merge-project` subcommand**: consolidate duplicate project slugs into one
  - Three-layer merge: global `harness.db` (UPDATE project column), per-project `harness.db` (ATTACH + INSERT OR IGNORE), file-based data (`obs/`, `sessions/`, `evolved/`, `evolution.jsonl`, `orbit/`)
  - `--dry-run` previews row/file counts without writing
  - `--delete-source` removes the source directory after a successful merge
  - Handles composite-PK tables (`metrics_state`, `skill_attribution`, `promotion_counters`) with conflict-aware merge strategies

### Changed
- **Skill descriptions normalized**: removed `Trigger:` prefix from all 14 skill descriptions, applied consistent `[What it does]. [When to use]` pattern
- Skill count: 23 → 26 (9 pipeline + 17 quality gates)

## [0.5.0]

### Added
- **SQLite operational store**: all project operational data (observations, sessions, metrics, evolution, orbit pipelines, evolved skills) now stored in `harness.db` alongside the existing `memory.db` knowledge graph
- `epic-harness migrate` subcommand: import legacy JSONL/JSON data into SQLite (`--dry-run` to preview, `--reset` to retry interrupted migration)
- `store::observations::query_latest_observations_conn()`: query N most recent observation records

### Changed
- Dashboard commands (`get_harness_metrics`, `get_orbit_pipelines`, `get_evolved_skills`, `get_obs_summary`) now read from SQLite instead of file I/O
- `observe` hook writes to SQLite first, falls back to JSONL on write failure
- `reflect` hook reads/writes metrics and evolution data from SQLite
- `snapshot` hook syncs sessions and orbit pipelines to SQLite
- Web dashboard HTML response now includes `Cache-Control: no-cache` header to prevent stale UI

### Migration Guide

Existing users with JSONL/JSON data should run once after upgrading:

```bash
epic-harness migrate --dry-run   # preview what would be imported
epic-harness migrate             # perform the import
```

Original files are **not deleted** after import. New users are automatically on SQLite — no action needed.

## [0.4.9] — 2026-05-29

### Added
- Dashboard: auto-build Tauri app in CI, agent dismiss action, orbit pipeline dismiss
- README: static badge links fixed

## [0.4.8] — 2026-05-28

### Added
- Dashboard: orbit full-auto mode, pipeline dismiss, agents done-separation, Ring 1 skill rename

## [0.4.7] — 2026-05-27

### Fixed
- Dashboard: dynamic version injection, evolution history newest-first sort, council auto-proceed, security hardening

## [0.4.6] — 2026-05-26

### Added
- Install: brew + cargo-binstall cascade fallback when binary not found

### Changed
- Rename `check` skill to `audit` for consistency with pipeline naming
- Remove Antigravity integration section from docs

## [0.4.5] — 2026-05-26

### Added
- Antigravity (Gemini CLI) plugin package with install docs and i18n translations
- Cursor integration: sessionStart + preCompact hooks, `.cursor/skills/` generation
- Plugin manifests: declare hooks and mcpServers in Claude/Codex plugin.json

### Changed
- Cursor: migrate commands to skills format
- Antigravity: switch from install.rs to plugin-only distribution
- i18n: sync installer URLs, remove stale Codex install command
- README: rewrite top description for multi-tool positioning
- Remove deprecated Codex and Antigravity file-based plugin files

### Fixed
- Dashboard: i18n double-call in Agents page

## [0.4.4] — 2026-05-25

### Changed
- Unified all commands into skills, removing the separate command layer

## [0.4.3] — 2026-05-25

### Fixed
- Hooks: use event-specific stdout for codex/antigravity protocol compatibility

### Changed
- README updated for v0.4.2 release
- Codex removed from install wizard

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
