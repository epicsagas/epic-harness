---
name: simplify
description: "Code simplification. Use when a file exceeds 200 lines, has high complexity, or contains duplication. Extract, rename, reduce."
trigger: when a file exceeds 200 lines or has deeply nested logic
---

# Simplify — Code Simplification

## When to Trigger

- File exceeds 200 lines
- Function exceeds 40 lines
- Deeply nested code (3+ levels)
- Copy-pasted blocks detected
- "This is getting hard to follow" feeling

## Process

### 1. Measure

- Line count, function count, nesting depth
- Identify the longest/most complex function

### 2. Extract

- **Extract function**: Turn a code block into a named function
- **Extract constant**: Replace magic numbers/strings
- **Extract module**: Split large files by responsibility

### 3. Rename

- Variables: describe what it holds, not how it's computed
- Functions: describe what it does, not how
- Files: match the primary export/class

### 4. Reduce

- Remove dead code (unused imports, unreachable branches)
- Replace imperative loops with declarative (map, filter, reduce)
- Merge duplicate logic into shared utility

### 5. Verify

- All tests still pass after simplification
- No behavior changes — only structural improvements

## Constraints

- One simplification at a time — verify between each
- Never simplify and add features in the same change

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|-------------------|
| "It works, don't touch it" | Working and maintainable are different. Tech debt compounds. | Simplify with tests as safety net. If tests pass, ship it. |
| "It's not that complex" | If you need to re-read it twice, it's too complex. | Apply the 30-second rule: can a new dev understand this in 30s? |
| "Refactoring is risky" | Not refactoring is riskier — bugs hide in complexity. | One small simplification at a time, verified by tests. |
| "We'll clean it up in the rewrite" | Rewrites get cancelled. Improve incrementally. | Simplify one function per session. Compound improvements. |

## Evidence Required

Before claiming simplification is done, show ALL of these:

- [ ] Before/after metrics: line count, function count, or nesting depth reduced
- [ ] All tests pass after change (show output)
- [ ] No behavior change: same inputs produce same outputs
- [ ] Each extraction is independently justified (not "because it felt cleaner")

**"I cleaned it up" without metrics = opinion, not simplification.**

## Red Flags

- Simplifying code you don't understand
- Over-abstracting (3 files for a 10-line utility)
- Mixing simplification with feature additions
