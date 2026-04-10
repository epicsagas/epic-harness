# Changelog

All notable changes to epic-harness will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **opencode integration**: JS plugin (`plugins/epic-harness.js`) for session/tool lifecycle hooks, 6 commands, 4 agents → `~/.config/opencode/`
- **cline integration**: 5 executable hook scripts (PreToolUse/PostToolUse/TaskStart/TaskResume/TaskCancel) → `~/Documents/Cline/Rules/Hooks/`
- **aider integration**: `.aider.conf.yml` + `.aider/CONVENTIONS.md` — no hook system, conventions auto-loaded via `read:` config
- **Interactive install menu**: `epic-harness install` (no args) shows numbered checklist; select by number (e.g. `1,3`) or `a` for all
- **Progress bar**: TTY shows animated `[====>   ] N/M filename`; non-TTY shows one-line summary per tool

### Changed
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
