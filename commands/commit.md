---
description: "Generate and execute git commits following Conventional Commits 1.0.0"
---

# /commit — Conventional Commits Generator

Generate and execute a git commit following [Conventional Commits 1.0.0](https://www.conventionalcommits.org/).

## Process

1. **Gather context** — run all four in parallel:
   - `git status`
   - `git diff HEAD`
   - `git branch --show-current`
   - `git log --oneline -5`

2. **Analyze changes** — determine the commit type:
   | Type | SemVer | When |
   |------|--------|------|
   | `feat` | MINOR | New feature or capability |
   | `fix` | PATCH | Bug fix |
   | `build` | — | Build system or dependencies |
   | `chore` | — | Maintenance, no production code change |
   | `ci` | — | CI/CD configuration |
   | `docs` | — | Documentation only |
   | `style` | — | Formatting, whitespace, semicolons |
   | `refactor` | — | Code change that neither fixes nor adds |
   | `perf` | — | Performance improvement |
   | `test` | — | Adding or correcting tests |
   | Breaking | MAJOR | Append `!` before `:` or add `BREAKING CHANGE:` footer |

3. **Generate 3 candidates** in `type(scope): description` format, evaluate each for:
   - Clarity — does it explain the *why*?
   - Compliance — valid Conventional Commits format?
   - Consistency — matches the repo's existing commit style?

4. **Body (optional)** — only when the diff needs context not obvious from the subject:
   - Max 3 bullet points, each under 60 characters
   - Use `git commit -m "subject" -m "- point 1\n- point 2"` to attach

5. **Auto-select and execute** — pick the best candidate and run:
   - `git add <files>` if needed (prefer specific files over `git add -A`)
   - `git commit -m "message"`
   - **Execute automatically** — no user confirmation needed

## Rules

- Format: `type(scope): description` — lowercase type, optional scope in parentheses
- Subject line: imperative mood, no period at end, under 72 characters
- Breaking changes: use `!` before `:` or `BREAKING CHANGE:` footer
- No emoji in commit messages
- No `Co-Authored-By` footers
- Single commit only — if changes span multiple concerns, suggest splitting first
- If no changes exist (clean working tree), inform the user instead of creating an empty commit
- Never use `--no-verify` flag

## Red Flags
- Committing unrelated changes in a single commit
- Vague messages like "update code" or "fix stuff"
- Using `git add -A` when only specific files changed
- Skipping the type prefix
