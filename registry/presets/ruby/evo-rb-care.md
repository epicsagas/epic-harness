---
name: rb-care
description: "Preset: Ruby files need lint and type-check after edits."
---

# Ruby file care (preset)

## Process
1. Run `ruby -c` to check syntax after editing `.rb` files
2. Run `bundle exec rubocop` for style and lint issues
3. If using Sorbet or Steep, run type-check (`srb tc` or `steep check`)

## Red Flags
- Skipping syntax check after editing Ruby files
- Ignoring rubocop warnings as "style only"
- Not running type-checker when Sorbet/Steep is configured