# Testing Checklist

## Test Types

### Unit Tests
- [ ] Pure functions tested with multiple inputs (happy + edge + error)
- [ ] One behavior per test
- [ ] No test interdependencies (each test runs in isolation)
- [ ] Mocks/stubs for external dependencies
- [ ] Test names describe the behavior, not the implementation

### Integration Tests
- [ ] API endpoints tested end-to-end
- [ ] Database queries tested against real (test) DB
- [ ] Auth flow tested (login → access → logout)
- [ ] Error responses tested (400, 401, 403, 404, 500)

### Edge Cases
- [ ] Empty inputs
- [ ] Null/undefined values
- [ ] Boundary values (0, -1, MAX_INT)
- [ ] Unicode and special characters
- [ ] Concurrent access
- [ ] Network failures

## Test Quality

### Coverage
- [ ] New code covered by tests
- [ ] Critical paths have 100% coverage
- [ ] Coverage measured and reported

### Reliability
- [ ] No flaky tests (random failures)
- [ ] Tests run in < 30 seconds (unit) / < 5 minutes (integration)
- [ ] Tests work offline (no external service deps)
- [ ] Tests clean up after themselves

### Maintainability
- [ ] Test helpers/fixtures extracted for reuse
- [ ] No hardcoded test data that becomes stale
- [ ] Test structure mirrors source structure
- [ ] Setup/teardown shared via beforeEach/afterEach

## Anti-Patterns
- Testing implementation details (private methods, internal state)
- Snapshot tests for non-visual code
- Tests that always pass (assert nothing meaningful)
- Tests disabled with `.skip` or `@Ignore` and forgotten
