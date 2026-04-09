// getObsSummary: tmpdir fixture test.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { getObsSummary } from "../hooks/scripts/snapshot.js";

const today = new Date().toISOString().slice(0, 10).replace(/-/g, "");

function fixture(records) {
  const dir = mkdtempSync(join(tmpdir(), "harness-snap-"));
  const file = join(dir, `session_${today}.jsonl`);
  writeFileSync(file, records.map(r => JSON.stringify(r)).join("\n") + "\n");
  return { dir, cleanup: () => rmSync(dir, { recursive: true, force: true }) };
}

test("getObsSummary: null on missing dir", () => {
  assert.equal(getObsSummary("/nonexistent/path/xyz"), null);
});

test("getObsSummary: aggregates success rate + error categories", () => {
  const { dir, cleanup } = fixture([
    { tool: "Bash", score: 1.0, result: "success" },
    { tool: "Bash", score: 1.0, result: "success" },
    { tool: "Edit", score: 0.3, result: "error", failure_category: "type_error" },
    { tool: "Edit", score: 0.3, result: "error", failure_category: "type_error" },
  ]);
  try {
    const s = getObsSummary(dir);
    assert.ok(s);
    assert.match(s, /4 obs/);
    assert.match(s, /50% success/);
    assert.match(s, /type_error:2/);
  } finally { cleanup(); }
});

test("getObsSummary: all-success → no error block", () => {
  const { dir, cleanup } = fixture([
    { tool: "Bash", score: 1.0, result: "success" },
  ]);
  try {
    const s = getObsSummary(dir);
    assert.ok(s);
    assert.match(s, /100% success/);
    assert.doesNotMatch(s, /errors=/);
  } finally { cleanup(); }
});
