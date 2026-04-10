# Changelog

All notable changes to epic-harness will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Multi-tool support**: epic-harness now works with Codex CLI, Gemini CLI, Cursor, and Google Antigravity
  — full Ring 0 hooks + commands + skills + agents ported for each tool
- `integrations/codex/` — hooks.json (SessionStart/PreToolUse/PostToolUse/Stop), 6 commands, 7 skills, 4 agents
- `integrations/gemini/` — settings.json (BeforeAgent/AfterAgent/BeforeModel/AfterModel), GEMINI.md snippet, 6 commands, 7 skills, 4 agents
- `integrations/cursor/` — hooks.json (PreToolUse/PostToolUse/SessionEnd), .mdc rules, 6 commands, 4 agents (requires Cursor 1.7+)
- `integrations/antigravity/` — AGENTS.md config, .agents/skills/, .agents/workflows/ (6 commands), 4 agent personas; Manager view used for parallel agent execution
- `install.sh` — unified installer: `./install.sh --tool=<codex|gemini|cursor|antigravity> [--global] [--dry-run]`
- `project_slug()`: stable `{dirname}-{6-char hex}` identifier to namespace per-project data
- Auto-migration: on first session, existing `cwd()/.harness/` is copied to the new global path

### Changed

- **Cursor `hooks.json`**: aligned with [Cursor hooks schema](https://cursor.com/docs/hooks) — `"version": 1`, camelCase events (`preToolUse`, `postToolUse`, `sessionEnd`), and `Shell` matcher for guard (Claude Code–style `PreToolUse` / `Bash` was not recognized, so hooks could appear unloaded)
- Cursor slash commands: added required YAML `name` field (kebab-case) alongside `description` so the Cursor slash menu shows summaries instead of only `(user)` — matches [Cursor command frontmatter](https://cursor.com/docs/reference/plugins)
- Cursor agents: added `model: inherit` in frontmatter per [Cursor subagents](https://cursor.com/docs/subagents)
- `epic-harness install`: non-root integration files are **synced** (written when missing or when content differs from the embedded copy), fixing skipped installs when a placeholder file already existed (e.g. empty `hooks.json`). `GEMINI.md` / `AGENTS.md` remain create-if-missing only
- Harness data directory moved from `cwd()/.harness/` to `~/.harness/projects/{slug}/`
  — prevents git pollution and survives project deletion
- `guard-rules.yaml` stays at `cwd()/.harness/guard-rules.yaml` for team git-sharing
- `cross_project_file` opt-in marker moved to `~/.harness/global/` (was per-project)
- `global_harness_dir` renamed from `~/.harness-global/` to `~/.harness/global/`

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
