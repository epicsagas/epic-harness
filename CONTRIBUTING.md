# Contributing to epic-harness

Thanks for your interest in contributing!

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/<you>/epic-harness.git`
3. Install: see [QUICKSTART.md](QUICKSTART.md)
4. Create a feature branch: `git checkout -b feat/your-feature`

## Development

```bash
npm install
npm run build
```

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
- Ensure `npm run build` passes
- Reference related issues in the description

## Reporting Issues

Use [GitHub Issues](https://github.com/epicsagas/epic-harness/issues). Include:
- epic-harness version
- Reproduction steps
- Expected vs actual behavior

## License

By contributing, you agree your contributions are licensed under the Apache License 2.0.
