---
name: ts-care
description: "Preset: TypeScript files need type-check after every edit."
---

# TypeScript file care (preset)

## Process
1. Run `tsc --noEmit` before and after editing .ts/.tsx files
2. Check for strict null violations
3. Validate import paths resolve correctly

## Red Flags
- Editing .ts files without verifying types afterward
- Ignoring `any` type assertions
