# Coding Conventions (epic-harness)

## Core Rules

- Write the test first, then the implementation (red → green → refactor).
- Every change must pass build + lint + tests before being considered done.
- Minimal, surgical edits only — no speculative refactoring or feature creep.
- No deprecated APIs. Match existing codebase style exactly.

## Security

- Never commit secrets, tokens, or credentials. Replace with `<REDACTED>`.
- Validate all external input at system boundaries only (user input, API responses).
- No SQL/command injection: use parameterised queries and safe subprocess APIs.

## Quality

- Functions > 40 lines or files > 200 lines signal a need to extract and simplify.
- Prefer explicit error handling over silent catches or empty `except` blocks.
- Delete dead code; do not comment it out.

## Workflow

- Before claiming a task done: build passes, tests pass, lint passes.
- After editing a file: check for type errors and run relevant tests immediately.
- When debugging a loop: stop after 3 attempts and re-read the error message.
