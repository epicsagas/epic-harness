---
name: py-care
description: "Preset: Python files benefit from type-check + lint after edits."
---

# Python file care (preset)

## Process
1. Run `ruff check` or `flake8` after editing
2. Run `mypy` if type hints are used
3. Check import order and unused imports

## Red Flags
- Editing .py without lint check
- Ignoring type-checker warnings
