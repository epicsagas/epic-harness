# Project Overrides (`.epic/`)

epic-harness ships one set of skill defaults. A team layers its own rules on top by committing a small markdown file beside the code, so every engineer and every host (Claude Code, Codex, agy, hermes) inherits the same conventions on pull. State never lives here; state lives in `$HARNESS_DIR`.

## Files

| Priority | File | Who owns it | Git |
|---|---|---|---|
| 1 (wins) | `.epic/team.local.md` | one engineer | ignored |
| 2 | `.epic/team.md` | the team | committed |
| 3 | shipped skill defaults | epic-harness | n/a |

`.epic/` starts absent. Create it only when there is a rule to record.

## Format

Plain markdown. Top-level bullets are standing facts for every skill. A heading equal to a skill name scopes the bullets under it to that skill. A bullet `file: {path}` loads another document you already maintain.

```markdown
# team

- Commit messages follow `type(scope): description`, lowercase, no emoji.
- Never bypass pre-commit hooks. `--no-verify` is not an option.
- file: docs/CONVENTIONS.md

## spec
- Every spec names the owning crate and the public API surface it changes.

## ship
- PRs target `main`. Release tags come from `cargo dist`, never by hand.
```

`_dispatch` reads these files at session start and before each pipeline skill, so a rule is present when the decision is made rather than caught in review.

## Writing a good `team.md`

The test for every line: would removing it change agent behavior? If not, delete it. Prefer what the code cannot express (frozen paths, policy, compliance), conventions that differ from ecosystem defaults, and pitfalls that were actually observed. A prohibition always names the permitted alternative. The same test applies to `AGENTS.md`: a 500-line AGENTS.md is planning context in the wrong place. Rationale and rejected approaches belong in the project docs vault, not in the repo file loaded every session.
