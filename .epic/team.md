# team

- CI fails on `cargo clippy -- -D warnings` and `cargo fmt --all -- --check`. Run both, plus `cargo test`, before reporting any Rust change as done.
- After editing any `skills/*/SKILL.md`, run `make lint-skills`.
- User-facing changes get a `CHANGELOG.md` entry in the same commit.
- Never bypass hooks. `--no-verify` is not an option; fix what the hook flagged.
- file: CONTRIBUTING.md

## spec
- Every spec names the ring (0 hooks, 1 pipeline, 2 quality gates, 3 evolve) and lists the `skills/` or `src/` paths it changes.

## ship
- PR titles follow `type(scope): description`, lowercase, no emoji.
