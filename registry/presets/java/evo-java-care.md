---
name: java-care
description: "Preset: Java files need compilation and lint after edits."
---

# Java file care (preset)

## Process
1. Run `javac` or `mvn compile` after editing `.java` files
2. Run `mvn checkstyle:check` or equivalent linter
3. Check for unused imports and missing null checks

## Red Flags
- Skipping compilation after editing Java files
- Ignoring checkstyle warnings as "style only"
- Not running static analysis (SpotBugs, PMD) when available