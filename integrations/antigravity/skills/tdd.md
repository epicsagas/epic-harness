---
name: tdd
description: "Test-Driven Development. Use when implementing any new feature or fixing a bug. Write test first, then implement, then refactor."
trigger: when implementing new features or fixing bugs
---

# TDD — Test-Driven Development

## Process

### Red → Green → Refactor

1. **Red**: Write a failing test that describes the desired behavior
   - Test name should read like a spec: `it("returns 401 when token is expired")`
   - Run the test — confirm it fails for the right reason

2. **Green**: Write the minimum code to make the test pass
   - Do not write more than needed
   - Do not optimize yet
   - Run the test — confirm it passes

3. **Refactor**: Clean up without changing behavior
   - Extract duplicates, rename for clarity, simplify
   - Run tests again — must still pass

### Cycle

Repeat for each behavior. One test, one behavior, one cycle.

## When to Trigger

- New function or method being written
- Bug fix (write regression test first)
- Any /go agent task

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|-------------------|
| "I'll add tests later" | You won't. 95% of "later" tests never get written. | Write the failing test NOW before any implementation. |
| "This is too simple to test" | Simple code breaks too — especially after refactoring. | If it has a return value or side effect, it's testable. Write one. |
| "Tests slow me down" | Debugging without tests costs 10x more time. | Time the cycle: test-first is faster by the second iteration. |
| "I'll just test manually" | Manual tests don't catch regressions. | Automate it once, save hours forever. |
| "The types guarantee correctness" | Types check shape, not logic. `add(a,b)` can still return `a-b`. | Types + tests together. Neither alone suffices. |
| "I need to see the API shape first" | Spike freely, then delete and rebuild test-first. | Write a throwaway spike, extract the interface, TDD the real impl. |

## Evidence Required

Before claiming TDD is done, show ALL of these:

- [ ] Failing test output (Red phase): the test name and failure message
- [ ] Passing test output (Green phase): ✓ with the same test name
- [ ] Test covers behavior, not implementation (no mocking internals)
- [ ] Refactor step completed OR explicitly noted as unnecessary with reason

**No evidence = not done.** "I wrote tests" without showing output is not TDD.

## Red Flags

- Writing implementation before any test exists
- Test that tests implementation details instead of behavior
- Skipping the refactor step
- Multiple behaviors in one test
