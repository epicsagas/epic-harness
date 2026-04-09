---
name: go-care
description: "Preset: Go files need vet + build check after edits."
---

# Go file care (preset)

## Process
1. Run `go vet ./...` after editing
2. Run `go build ./...` to catch compile errors early
3. Check for unused imports (will fail build)

## Red Flags
- Editing Go files without running vet
- Leaving unused imports (Go compiler will reject)
