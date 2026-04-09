---
name: fix-build-fail
description: "Preset: Common Node.js build failures."
---

# Fix build failures (preset)

## Remediation
1. Check tsconfig.json paths and module resolution
2. Verify all imports exist (`tsc --noEmit`)
3. Check package.json type field (module vs commonjs)

## Red Flags
- Ignoring TypeScript strict mode errors
- Missing type declarations for dependencies
