// E2E: spawn compiled hook scripts with tmpdir cwd, verify side effects.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, existsSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";

const OBSERVE = resolve("hooks/scripts/observe.js");
const SNAPSHOT = resolve("hooks/scripts/snapshot.js");
const GUARD = resolve("hooks/scripts/guard.js");
const RESUME = resolve("hooks/scripts/resume.js");

function spawnHook(scriptPath, cwd, stdinJson) {
  return spawnSync("node", [scriptPath], {
    cwd,
    env: { ...process.env, HOME: cwd },
    input: JSON.stringify(stdinJson),
    encoding: "utf8",
    timeout: 5000,
  });
}

function setupHarness() {
  const dir = mkdtempSync(join(tmpdir(), "harness-e2e-"));
  // Use resume to initialize the harness directory (slug-aware)
  spawnHook(RESUME, dir, {});
  return { dir, cleanup: () => rmSync(dir, { recursive: true, force: true }) };
}

test("observe: appends scored record to session JSONL", () => {
  const { dir, cleanup } = setupHarness();
  try {
    const result = spawnHook(OBSERVE, dir, {
      tool_name: "Bash",
      tool_input: { command: "echo hello" },
      tool_output: { output: "hello\n", stderr: "" },
    });
    assert.equal(result.status, 0, `observe failed: ${result.stderr}`);

    // Find the slug directory under .harness/projects
    const projectsDir = join(dir, ".harness/projects");
    const projectSlugDir = readdirSync(projectsDir)[0];
    const obsDir = join(projectsDir, projectSlugDir, "obs");
    const obsFiles = readdirSync(obsDir).filter(f => f.startsWith("session_") && f.endsWith(".jsonl"));
    assert.ok(obsFiles.length >= 1, "session file created");

    const line = readFileSync(join(obsDir, obsFiles[0]), "utf8").trim();
    const record = JSON.parse(line);
    assert.equal(record.tool, "Bash");
    assert.equal(record.tool_category, "bash");
    assert.equal(record.result, "success");
    assert.equal(record.failure_category, null);
    assert.ok(record.score > 0.9);
  } finally { cleanup(); }
});

test("observe: classifies type_error from stderr", () => {
  const { dir, cleanup } = setupHarness();
  try {
    spawnHook(OBSERVE, dir, {
      tool_name: "Bash",
      tool_input: { command: "node -e 'x.y'" },
      tool_output: { output: "", stderr: "TypeError: Cannot read property y" },
    });
    // Find the slug directory under .harness/projects
    const projectsDir = join(dir, ".harness/projects");
    const projectSlugDir = readdirSync(projectsDir)[0];
    const obsDir2 = join(projectsDir, projectSlugDir, "obs");
    const obsFiles2 = readdirSync(obsDir2).filter(f => f.startsWith("session_") && f.endsWith(".jsonl"));
    const line = readFileSync(join(obsDir2, obsFiles2[0]), "utf8").trim();
    const record = JSON.parse(line);
    assert.equal(record.result, "error");
    assert.equal(record.failure_category, "type_error");
  } finally { cleanup(); }
});

test("snapshot: writes snapshot file with obs summary", () => {
  const { dir, cleanup } = setupHarness();
  try {
    // Pre-populate an observation
    spawnHook(OBSERVE, dir, {
      tool_name: "Bash",
      tool_input: { command: "ls" },
      tool_output: { output: "file.txt\n", stderr: "" },
    });
    // Trigger snapshot
    const result = spawnHook(SNAPSHOT, dir, {
      conversation_summary: "testing snapshot",
      pending_tasks: ["task1"],
    });
    assert.equal(result.status, 0);

    const projectsDir = join(dir, ".harness/projects");
    const projectSlugDir = readdirSync(projectsDir)[0];
    const sessionsDir = join(projectsDir, projectSlugDir, "sessions");
    const files = readdirSync(sessionsDir).filter(f => f.startsWith("snapshot_") && f.endsWith(".json"));
    assert.ok(files.length >= 1, "snapshot file written");
    const snap = JSON.parse(readFileSync(join(sessionsDir, files[0]), "utf8"));
    assert.equal(snap.type, "pre-compact");
    assert.deepEqual(snap.pending_tasks, ["task1"]);
    assert.match(snap.summary, /testing snapshot/);
    assert.match(snap.summary, /Eval:/); // obs summary injected
  } finally { cleanup(); }
});

test("guard: blocks rm -rf / with exit code 2", () => {
  const { dir, cleanup } = setupHarness();
  try {
    const result = spawnHook(GUARD, dir, {
      tool_input: { command: "rm -rf /" },
    });
    assert.equal(result.status, 2, "expected blocked exit code 2");
  } finally { cleanup(); }
});

test("guard: allows safe commands", () => {
  const { dir, cleanup } = setupHarness();
  try {
    const result = spawnHook(GUARD, dir, {
      tool_input: { command: "git status" },
    });
    assert.equal(result.status, 0);
  } finally { cleanup(); }
});
