# Performance Checklist

## Database
- [ ] No N+1 queries — use JOIN, eager loading, or batch
- [ ] Indexes on frequently filtered/sorted columns
- [ ] Pagination for list endpoints
- [ ] Connection pooling configured
- [ ] No `SELECT *` when only specific columns needed
- [ ] Slow query logging enabled

## Memory
- [ ] No unbounded caches or arrays
- [ ] Event listeners removed on cleanup/unmount
- [ ] Large objects released after use
- [ ] Streams for large file operations
- [ ] WeakRef/WeakMap for caches where appropriate

## Network
- [ ] Response compression (gzip/brotli)
- [ ] Static asset caching (Cache-Control headers)
- [ ] API response pagination
- [ ] No redundant API calls (dedup, cache)
- [ ] Connection keep-alive enabled

## Frontend
- [ ] Images optimized (WebP, lazy loading, srcset)
- [ ] Bundle split / code splitting
- [ ] No unnecessary re-renders (React.memo, useMemo)
- [ ] Debounce on frequent events (scroll, resize, input)
- [ ] Virtual scrolling for long lists

## Computation
- [ ] Expensive calculations memoized
- [ ] Heavy work offloaded to workers/background jobs
- [ ] No blocking I/O on main thread
- [ ] Batch operations where possible (bulk insert > loop insert)

## Monitoring
- [ ] p50/p95/p99 latency tracked
- [ ] Error rate monitored
- [ ] Resource usage (CPU, memory) dashboarded
- [ ] Alerts on threshold breaches
