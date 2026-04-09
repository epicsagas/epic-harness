// Ring 0: guard rule tests.
import { test } from "node:test";
import assert from "node:assert/strict";
import { BLOCKED, WARNED } from "../hooks/scripts/guard.js";

const matchAny = (rules, cmd) => rules.some(r => r.pattern.test(cmd));

test("BLOCKED: force push to main/master", () => {
  assert.ok(matchAny(BLOCKED, "git push --force origin main"));
  assert.ok(matchAny(BLOCKED, "git push origin --force master"));
  assert.ok(!matchAny(BLOCKED, "git push origin main"));
  assert.ok(!matchAny(BLOCKED, "git push --force origin feature/x"));
});

test("BLOCKED: rm -rf /", () => {
  assert.ok(matchAny(BLOCKED, "rm -rf /"));
  assert.ok(!matchAny(BLOCKED, "rm -rf /tmp/foo"));
  assert.ok(!matchAny(BLOCKED, "rm -rf ./dist"));
});

test("BLOCKED: DROP on prod db", () => {
  assert.ok(matchAny(BLOCKED, "DROP DATABASE prod_users"));
  assert.ok(matchAny(BLOCKED, "drop table prod.users"));
  assert.ok(!matchAny(BLOCKED, "DROP TABLE staging_users"));
});

test("WARNED: force push (any target) + hard reset + rm -rf", () => {
  assert.ok(matchAny(WARNED, "git push --force origin feature/x"));
  assert.ok(matchAny(WARNED, "git reset --hard HEAD~3"));
  assert.ok(matchAny(WARNED, "rm -rf node_modules"));
  assert.ok(!matchAny(WARNED, "git push origin main"));
});
