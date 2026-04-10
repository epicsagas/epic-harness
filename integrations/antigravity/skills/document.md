---
name: document
description: "Auto-documentation. Use when a public API, function, or module is added or changed. Generate/update JSDoc, docstrings, or comments."
trigger: when a public function, class, or API endpoint is added or changed
---

# Document — Auto-Documentation

## When to Trigger

- New public function, class, or API endpoint
- Function signature changed (params added/removed)
- Module purpose unclear from code alone
- User explicitly asks for documentation

## Process

### 1. Detect what changed

- New exports? → Add JSDoc/docstring
- Changed params? → Update existing docs
- New file? → Add module-level doc comment

### 2. Write docs

Follow the project's existing doc style. If none exists:

**TypeScript/JavaScript:**
```typescript
/**
 * Brief description of what this does.
 *
 * @param name - Description of parameter
 * @returns Description of return value
 * @throws ErrorType - When this happens
 *
 * @example
 * const result = myFunction("input");
 */
```

**Python:**
```python
def my_function(name: str) -> str:
    """Brief description.

    Args:
        name: Description of parameter.

    Returns:
        Description of return value.

    Raises:
        ValueError: When this happens.
    """
```

### 3. Don't over-document

- Skip obvious getters/setters
- Skip internal/private helpers unless complex
- Code should be self-documenting first, comments second

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|-------------------|
| "The code is self-documenting" | Function signatures don't explain _why_ or edge cases. | Document the why, the constraints, and the non-obvious. |
| "Docs get outdated" | Undocumented code is immediately outdated — 100% wrong by omission. | Put docs near code (JSDoc/docstring). They update with the code. |
| "I'll document it when the API stabilizes" | By then you'll forget the design rationale. | Document now. Update is cheaper than reconstructing intent. |

## Evidence Required

Before claiming documentation is done, show ALL applicable:

- [ ] Every new public function/class has a doc comment (show one example)
- [ ] Changed signatures have updated docs (show the diff)
- [ ] `@param` / `@returns` / `@throws` present for non-trivial functions
- [ ] At least one `@example` for complex APIs

**"I added docs" without showing them = not documented.**

## Red Flags

- Comments that restate the code: `// increment i` → `i++`
- Outdated comments that contradict the code
- Missing docs on public API that others will call
