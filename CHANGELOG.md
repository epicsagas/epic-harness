# Changelog

All notable changes to epic-harness will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Extract scoring weights to `SCORE_WEIGHTS` constant in `common.ts`
- Extract 11 pattern detection and skill seeding thresholds to named constants
- Evolved skills now defer to static skills on conflict (`_dispatch` policy)
- Add Acknowledgments section to README with reference project attributions

### Added

- Initial project setup
- Test suite with Node built-in `node:test` runner (zero deps): 25 tests covering
  failure classification, Ring 3 analysis (`analyzeSession`, `detectPatterns`,
  `computeTrend`, `checkStagnation`), guard rules, snapshot summary, and E2E
  hook invocation via subprocess
- GitHub Actions CI workflow (`.github/workflows/ci.yml`) — runs `npm test` on
  push and PR
- `npm test` script (builds then runs all tests)

### Changed

- Exported pure functions from `src/ts/reflect.ts`, `src/ts/guard.ts`,
  `src/ts/snapshot.ts` for testability
- Wrapped top-level `runHook`/`runGuardHook` calls with `isMain` guard so hook
  modules can be imported by tests without stdin hangs
- `getObsSummary` now accepts an optional `obsDir` parameter for test fixtures
