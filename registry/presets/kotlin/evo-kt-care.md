---
name: kt-care
description: "Preset: Kotlin files need compilation and lint after edits."
---

# Kotlin file care (preset)

## Process
1. Run `kotlinc` or `./gradlew compileKotlin` after editing `.kt` files
2. Run `./gradlew detekt` or `ktlint` for idiomatic patterns
3. Check for unnecessary safe calls and smart cast opportunities

## Red Flags
- Skipping compilation after editing Kotlin files
- Ignoring detekt or ktlint warnings
- Using !! (non-null assertion) instead of safe alternatives