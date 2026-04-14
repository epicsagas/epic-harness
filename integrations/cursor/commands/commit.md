---
name: commit
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
   | Type | When |
   |------|------|
   | `feat` | New feature or capability |
   | `fix` | Bug fix |
   | `build` | Build system or dependencies |
   | `chore` | Maintenance, no production code change |
   | `ci` | CI/CD configuration |
   | `docs` | Documentation only |
   | `style` | Formatting, whitespace |
   | `refactor` | Code change that neither fixes nor adds |
   | `perf` | Performance improvement |
   | `test` | Adding or correcting tests |

3. **Generate 3 candidates** in `type(scope): description` format, pick the best for clarity, compliance, and consistency with existing commit style.

4. **Body (optional)** — only when the diff needs context not obvious from the subject. Max 3 bullet points, each under 60 characters.

5. **Auto-select and execute**:
   - `git add <files>` (prefer specific files over `git add -A`)
   - `git commit -m "message"`
   - Execute automatically — no user confirmation needed

## Rules

- Format: `type(scope): description` — lowercase type, optional scope, imperative mood, no period, under 72 chars
- Breaking changes: use `!` before `:` or `BREAKING CHANGE:` footer
- No emoji, no `Co-Authored-By` footers
- Single commit only — suggest splitting if changes span multiple concerns
- If no changes exist, inform the user instead of creating an empty commit
- Never use `--no-verify` flag
