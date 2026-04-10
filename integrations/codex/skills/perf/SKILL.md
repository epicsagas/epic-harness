---
name: perf
description: "Performance review. Use when writing loops, database queries, rendering logic, or data processing. Detect N+1, leaks, unnecessary computation."
---

# Perf — Performance Review

## When to Trigger
- Database queries inside loops
- Large data set processing
- Rendering or UI update logic
- API endpoint that could receive high traffic
- File I/O in hot paths

## Checklist

### Database
- [ ] No N+1 queries (use JOIN, eager loading, or batch)
- [ ] Indexes exist for frequently queried columns
- [ ] Pagination for large result sets
- [ ] Connection pooling configured

### Memory
- [ ] No unbounded arrays or caches growing indefinitely
- [ ] Event listeners properly removed / unsubscribed
- [ ] Large objects released after use
- [ ] Streams used for large file processing (not loading entire file)

### Computation
- [ ] Expensive calculations memoized where appropriate
- [ ] No redundant re-renders (React: useMemo, useCallback)
- [ ] Async operations don't block the main thread
- [ ] Debounce/throttle on frequent events (scroll, input)

### Network
- [ ] Responses compressed (gzip/brotli)
- [ ] Static assets cached with proper headers
- [ ] No unnecessary API calls (cache, dedupe)

See `references/performance.md` for the full checklist.

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|-------------------|
| "Premature optimization" | Knuth meant micro-opts, not O(n²) queries. N+1 is never premature. | Fix algorithmic issues now. Micro-opt later with profiling data. |
| "We don't have enough traffic yet" | When traffic arrives, you won't have time to fix. | Build for 10x current scale. It's cheaper now than under load. |
| "The ORM handles it" | ORMs generate N+1 by default. Check the query log. | Always inspect generated SQL. Use eager loading explicitly. |
| "We'll cache it later" | Caching hides the problem and adds invalidation complexity. | Fix the query first. Cache only what's still slow after optimization. |

## Evidence Required

Before claiming performance review is complete, show ALL applicable:

- [ ] No N+1 queries: show the query plan or eager-loading code
- [ ] Pagination present: show limit/offset or cursor implementation
- [ ] No unbounded collections: show size caps or streaming for large data
- [ ] Async I/O in request paths: show `await` or stream usage (no sync fs/net)

**"Looks fine" is not a review. Show the query or the code path.**

## Red Flags
- Loading all records when only count is needed
- Synchronous file I/O in request handlers
- Missing pagination on list endpoints
