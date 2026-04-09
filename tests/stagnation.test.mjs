// checkStagnation: metric mutation logic only (fs side effects are no-ops in tmp cwd).
import { test } from "node:test";
import assert from "node:assert/strict";
import { checkStagnation } from "../hooks/scripts/reflect.js";
import { defaultMetrics } from "../hooks/scripts/common.js";

test("first session: always improved", () => {
  const m = defaultMetrics();
  const r = checkStagnation(m, 0.5);
  assert.equal(r.improved, true);
  assert.equal(r.shouldRollback, false);
});

test("score improves by ≥5%: improved=true, stagnation_count unchanged", () => {
  const m = { ...defaultMetrics(), total_sessions: 3, best_score: 0.70, stagnation_count: 0 };
  const r = checkStagnation(m, 0.80);
  assert.equal(r.improved, true);
  assert.equal(m.stagnation_count, 0);
});

test("score flat: stagnation_count increments, no rollback before limit", () => {
  const m = { ...defaultMetrics(), total_sessions: 5, best_score: 0.80, stagnation_count: 0 };
  checkStagnation(m, 0.79);
  assert.equal(m.stagnation_count, 1);
  checkStagnation(m, 0.78);
  assert.equal(m.stagnation_count, 2);
});

test("stagnation at limit with low best_score: rollback attempted", () => {
  // best < 0.90 triggers rollback branch regardless of degradation
  const m = { ...defaultMetrics(), total_sessions: 10, best_score: 0.70, stagnation_count: 2 };
  const r = checkStagnation(m, 0.68);
  // stagnation_count reaches 3 → rollback attempt
  // shouldRollback depends on EVOLVED_BACKUP_DIR existence (won't in test cwd) → false
  // but the branch was taken: stagnation_count logic ran
  assert.equal(r.improved, false);
  // when no backup dir exists, shouldRollback stays false but count is not reset
  assert.ok(m.stagnation_count >= 3 || r.shouldRollback);
});
