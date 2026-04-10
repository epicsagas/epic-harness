# Changelog

All notable changes to epic-harness will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Harness data directory moved from `cwd()/.harness/` to `~/.harness/projects/{slug}/`
  — prevents git pollution and survives project deletion
- `guard-rules.yaml` stays at `cwd()/.harness/guard-rules.yaml` for team git-sharing
- `cross_project_file` opt-in marker moved to `~/.harness/global/` (was per-project)
- `global_harness_dir` renamed from `~/.harness-global/` to `~/.harness/global/`

### Added

- `project_slug()`: stable `{dirname}-{4-char hex}` identifier to namespace per-project data
- Auto-migration: on first session, existing `cwd()/.harness/` is copied to the new global path

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
