// Node built-in test runner — no external deps.
// Run after `npm run build` so hooks/scripts/common.js exists.
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  classifyFailure,
  classifyTool,
  extractFileExt,
  extractFunctionName,
  parseFrontmatter,
  validateEvolvedSkill,
  validateGuardPattern,
} from "../hooks/scripts/common.js";

test("classifyFailure: null on empty/clean output", () => {
  assert.equal(classifyFailure(""), null);
  assert.equal(classifyFailure("all good"), null);
  // must not match "ErrorBoundary" false-positive
  assert.equal(classifyFailure("imported ErrorBoundary from react"), null);
});

test("classifyFailure: canonical categories", () => {
  assert.equal(classifyFailure("TypeError: foo is undefined"), "type_error");
  assert.equal(classifyFailure("SyntaxError: Unexpected token"), "syntax_error");
  assert.equal(classifyFailure("AssertionError: expected 1"), "test_fail");
  assert.equal(classifyFailure("eslint found 3 errors"), "lint_fail");
  assert.equal(classifyFailure("error TS2322: build failed"), "build_fail");
  assert.equal(classifyFailure("EACCES: permission denied"), "permission_denied");
  assert.equal(classifyFailure("ETIMEDOUT waiting"), "timeout");
  assert.equal(classifyFailure("ENOENT: No such file or directory"), "not_found");
  assert.equal(classifyFailure("\nError: boom\n  at x (y.js:1)"), "runtime_error");
});

test("classifyTool: canonical tool names", () => {
  assert.equal(classifyTool("Bash"), "bash");
  assert.equal(classifyTool("Edit"), "edit");
  assert.equal(classifyTool("Write"), "write");
  assert.equal(classifyTool("Read"), "read");
  assert.equal(classifyTool("Glob"), "glob");
  assert.equal(classifyTool("Grep"), "grep");
  assert.equal(classifyTool("TaskCreate"), "other");
  assert.equal(classifyTool(""), "other");
});

test("extractFileExt: from file_path and bash command", () => {
  assert.equal(extractFileExt({ file_path: "src/a.ts" }), ".ts");
  assert.equal(extractFileExt({ command: "node runner.js --flag" }), ".js");
  assert.equal(extractFileExt(undefined), undefined);
  assert.equal(extractFileExt({ command: "ls -la" }), undefined);
});

// ── extractFunctionName ───────────────────────────────
test("extractFunctionName: extracts from various patterns", () => {
  assert.equal(extractFunctionName("function handleSubmit() {"), "handleSubmit");
  assert.equal(extractFunctionName("const processData = async ("), "processData");
  assert.equal(extractFunctionName("def calculate_total(items):"), "calculate_total");
  assert.equal(extractFunctionName("at validateInput (/src/util.js:42)"), "validateInput");
  assert.equal(extractFunctionName(".fetchUser is not a function"), "fetchUser");
  assert.equal(extractFunctionName(""), null);
  assert.equal(extractFunctionName("just plain text"), null);
});

// ── parseFrontmatter ──────────────────────────────────
test("parseFrontmatter: valid frontmatter", () => {
  const content = '---\nname: test-skill\ndescription: "A test skill"\n---\n# Test';
  const fm = parseFrontmatter(content);
  assert.equal(fm.name, "test-skill");
  assert.equal(fm.description, "A test skill");
});

test("parseFrontmatter: null on missing/invalid", () => {
  assert.equal(parseFrontmatter(""), null);
  assert.equal(parseFrontmatter("no frontmatter"), null);
  assert.equal(parseFrontmatter("---\nname: x\n---"), null); // no description
});

// ── validateEvolvedSkill ──────────────────────────────
test("validateEvolvedSkill: valid skill passes", () => {
  const content = [
    "---",
    "name: evo-test",
    'description: "Auto-evolved from 5x type_error failures."',
    "---",
    "",
    "# Fix type error",
    "",
    "Detected 5 occurrences of type_error.",
    "",
    "## Remediation",
    "Check variable types carefully.",
  ].join("\n");
  const result = validateEvolvedSkill(content);
  assert.equal(result.valid, true);
  assert.equal(result.errors.length, 0);
  assert.equal(result.frontmatter.name, "evo-test");
});

test("validateEvolvedSkill: rejects skill without actionable section", () => {
  const content = [
    "---",
    "name: bad-skill",
    'description: "Some description here."',
    "---",
    "",
    "# Title",
    "",
    "Just some text without remediation or process or red flags section.",
  ].join("\n");
  const result = validateEvolvedSkill(content);
  assert.equal(result.valid, false);
  assert.ok(result.errors.includes("missing_actionable_section"));
});

test("validateEvolvedSkill: rejects too-short description", () => {
  const content = '---\nname: x\ndescription: "short"\n---\n# T\nfoo';
  const result = validateEvolvedSkill(content);
  assert.equal(result.valid, false);
});

// ── validateGuardPattern ──────────────────────────────
test("validateGuardPattern: rejects nested quantifier (a+)+", () => {
  assert.equal(validateGuardPattern("(a+)+"), false);
});

test("validateGuardPattern: rejects nested quantifier (a+)+$", () => {
  assert.equal(validateGuardPattern("(a+)+$"), false);
});

test("validateGuardPattern: rejects nested quantifier (a*)*", () => {
  assert.equal(validateGuardPattern("(a*)*"), false);
});

test("validateGuardPattern: accepts alternation without inner quantifiers (docker|podman)", () => {
  // (x|y)+ is safe when alternatives contain no inner quantifiers
  assert.equal(validateGuardPattern("(docker|podman)\\s+run"), true);
});

test("validateGuardPattern: rejects deeply nested (x+y+)+", () => {
  assert.equal(validateGuardPattern("(x+y+)+"), false);
});

test("validateGuardPattern: accepts safe pattern kubectl\\s+delete", () => {
  assert.equal(validateGuardPattern("kubectl\\s+delete"), true);
});

test("validateGuardPattern: accepts safe pattern docker.*prune", () => {
  assert.equal(validateGuardPattern("docker.*prune"), true);
});

test("validateGuardPattern: accepts safe pattern git push --force", () => {
  assert.equal(validateGuardPattern("git push --force"), true);
});

test("validateGuardPattern: rejects invalid regex", () => {
  assert.equal(validateGuardPattern("(unclosed"), false);
});
