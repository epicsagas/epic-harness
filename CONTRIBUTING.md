# Contributing to epic-harness

Thanks for your interest in contributing!

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/<you>/epic-harness.git`
3. Install: see [QUICKSTART.md](QUICKSTART.md)
4. Create a feature branch: `git checkout -b feat/your-feature`

## Development

```bash
cargo build                              # Build the binary
cargo test                               # Run all tests (314+ unit + 18 integration)
cargo clippy                             # Lint
node scripts/lint-skills.mjs              # Validate published skill frontmatter
node --test scripts/lint-skills.test.mjs  # Test the validator itself
cp target/debug/epic-harness hooks/bin/  # Test as plugin
```

`make lint-skills`, `make gen-skills`, and `make gen-skills-dry` use the same
cross-platform validator for the published `skills/` directory. Missing or empty
skill roots, missing skill files, invalid required frontmatter, and name/directory
mismatches fail validation. Description wording warnings are advisory and inspect
only the description field. Optional YAML metadata is not schema-validated.

## Commit Convention

This project follows [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` new feature
- `fix:` bug fix
- `docs:` documentation only
- `refactor:` code change without behavior change
- `chore:` build/config changes

Example: `feat(evolve): add stagnation gating`

## Pull Requests

- Keep PRs focused — one logical change per PR
- Update `CHANGELOG.md` for user-facing changes
- Ensure `cargo test` and `cargo clippy` pass
- Reference related issues in the description

## Reporting Issues

Use [GitHub Issues](https://github.com/epicsagas/epic-harness/issues). Include:
- epic-harness version
- Reproduction steps
- Expected vs actual behavior

## License

By contributing, you agree your contributions are licensed under the Apache License 2.0.
