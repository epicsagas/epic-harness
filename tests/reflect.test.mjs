// Integration tests for Ring 3 analysis — pure functions only.
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  analyzeSession,
  detectPatterns,
  computeTrend,
  gateSkills,
} from "../hooks/scripts/reflect.js";
import { EVOLVED_DIR } from "../hooks/scripts/common.js";
import { mkdirSync, writeFileSync, existsSync, rmSync } from "node:fs";
import { join } from "node:path";

// ── Fixture builders ────────────────────────────────
const obs = (over = {}) => ({
  timestamp: "2026-04-09T00:00:00Z",
  tool: "Bash",
  tool_category: "bash",
  action: "echo hi",
  result: "success",
  score: 1.0,
  dimensions: { tool_success: 1, output_quality: 1, execution_cost: 1 },
  failure_category: null,
  file_ext: undefined,
  sequence_id: 0,
  ...over,
});

const err = (file, category, tool = "Edit", cat = "edit") => obs({
  tool, tool_category: cat,
  action: file,
  result: "error",
  score: 0.3,
  dimensions: { tool_success: 0, output_quality: 0.3, execution_cost: 1 },
  failure_category: category,
});

// ── analyzeSession ──────────────────────────────────
test("analyzeSession: aggregates success rate and tool stats", () => {
  const records = [
    obs({ sequence_id: 1 }),
    obs({ sequence_id: 2 }),
    err("/repo/src/a.ts", "type_error", "Edit", "edit"),
  ];
  const a = analyzeSession(records);
  assert.equal(a.total_observations, 3);
  assert.equal(a.success_rate, Math.round((2 / 3) * 1000) / 1000);
  assert.ok(a.avg_score > 0 && a.avg_score < 1);
  assert.equal(a.per_tool_stats.bash.successes, 2);
  assert.equal(a.per_tool_stats.edit.errors, 1);
  assert.equal(a.per_error_stats.type_error, 1);
  assert.ok(a.score_distribution["0.2-0.4"] >= 1);
});

test("analyzeSession: empty input returns neutral stats", () => {
  const a = analyzeSession([]);
  assert.equal(a.total_observations, 0);
  assert.equal(a.success_rate, 1);
  assert.equal(a.avg_score, 0);
});

// ── detectPatterns ──────────────────────────────────
test("detectPatterns: repeated_same_error fires at streak ≥ 3", () => {
  const records = [
    err("/repo/src/a.ts", "type_error"),
    err("/repo/src/a.ts", "type_error"),
    err("/repo/src/a.ts", "type_error"),
    obs({ sequence_id: 99 }), // break streak to flush
  ];
  const p = detectPatterns(records);
  const repeated = p.find(x => x.pattern_type === "repeated_same_error");
  assert.ok(repeated, "expected repeated_same_error pattern");
  assert.ok(repeated.count >= 3);
  assert.ok(repeated.involved_files.includes("/repo/src/a.ts"));
});

test("detectPatterns: no false positive on single errors", () => {
  const records = [
    err("/repo/src/a.ts", "type_error"),
    obs(),
    err("src/b.ts", "syntax_error"),
  ];
  const p = detectPatterns(records);
  assert.equal(p.find(x => x.pattern_type === "repeated_same_error"), undefined);
});

// ── computeTrend ────────────────────────────────────
test("computeTrend: improving/stable/declining", () => {
  const mk = (scores) => scores.map((s, i) => ({
    session_id: `s${i}`,
    avg_score: s,
    timestamp: `2026-04-0${i + 1}`,
  }));
  assert.equal(computeTrend(mk([0.5, 0.6, 0.7, 0.8])), "improving");
  assert.equal(computeTrend(mk([0.8, 0.7, 0.6, 0.5])), "declining");
  assert.equal(computeTrend(mk([0.7, 0.7, 0.7, 0.7])), "stable");
  assert.equal(computeTrend([]), "stable"); // insufficient data
});

// ── gateSkills ─────────────────────────────────────────
test("gateSkills: removes invalid skills, keeps valid ones", () => {
  // Setup temp evolved dir
  const testDir = join(process.cwd(), ".harness", "evolved");
  mkdirSync(join(testDir, "evo-good"), { recursive: true });
  mkdirSync(join(testDir, "evo-bad"), { recursive: true });
  mkdirSync(join(testDir, "evo-no-file"), { recursive: true });

  // Good skill
  writeFileSync(join(testDir, "evo-good", "SKILL.md"), [
    "---",
    "name: evo-good",
    'description: "Auto-evolved from 5x type_error failures."',
    "---",
    "",
    "# Fix type error",
    "",
    "Detected 5 occurrences.",
    "",
    "## Remediation",
    "Check variable types carefully.",
  ].join("\n"));

  // Bad skill — no actionable section
  writeFileSync(join(testDir, "evo-bad", "SKILL.md"), "just text no frontmatter");

  // No SKILL.md — evo-no-file has dir but no file

  const result = gateSkills();
  assert.ok(result.kept.some(s => s.name === "evo-good"), "should keep valid skill");
  assert.ok(result.removed.some(s => s.name === "evo-bad"), "should remove invalid skill");
  assert.ok(result.removed.some(s => s.name === "evo-no-file"), "should remove missing file");

  // Cleanup
  rmSync(testDir, { recursive: true, force: true });
});
