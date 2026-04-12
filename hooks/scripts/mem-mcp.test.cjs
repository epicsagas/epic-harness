'use strict';
/**
 * mem-mcp.test.cjs — Unit tests for MCP memory server
 * Run: node hooks/scripts/mem-mcp.test.cjs
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

// ── Setup temp HARNESS_ROOT ───────────────────────────────────────────────────
const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'harness-mcp-test-'));
process.env.HARNESS_ROOT = tmpRoot;

const { callTool, TOOLS } = require('./mem-mcp.cjs');

// ── Simple test harness ───────────────────────────────────────────────────────
let passed = 0;
let failed = 0;

function assert(condition, message) {
  if (!condition) throw new Error(`Assertion failed: ${message}`);
}

async function test(name, fn) {
  try {
    await fn();
    console.log(`  PASS  ${name}`);
    passed++;
  } catch (e) {
    console.log(`  FAIL  ${name}: ${e.message}`);
    failed++;
  }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

(async () => {
  console.log('\nmem-mcp.cjs — unit tests\n');

  // 1. TOOLS list has 5 entries
  await test('tools/list → 5 tools returned', () => {
    assert(Array.isArray(TOOLS), 'TOOLS must be an array');
    assert(TOOLS.length === 5, `Expected 5 tools, got ${TOOLS.length}`);
    const names = TOOLS.map((t) => t.name);
    for (const expected of ['mem_add', 'mem_query', 'mem_search', 'mem_related', 'mem_context']) {
      assert(names.includes(expected), `Missing tool: ${expected}`);
    }
  });

  // 2. mem_add creates a node and returns an id
  let createdId;
  await test('mem_add → node file created, id returned', () => {
    const result = callTool('mem_add', {
      title: 'Test Node',
      type: 'concept',
      body: '# Test\nThis is a test memory node.',
      tags: ['test', 'mcp'],
      project: 'epic-harness',
    });
    assert(result && result.content, 'result must have content');
    const text = result.content[0].text;
    const data = JSON.parse(text);
    assert(data.id, 'id must be present');
    createdId = data.id;

    // Verify file exists
    const nodesDir = path.join(tmpRoot, 'memory', 'nodes');
    const filePath = path.join(nodesDir, `${createdId}.md`);
    assert(fs.existsSync(filePath), `Node file not found at ${filePath}`);
  });

  // 3. mem_query returns the added node
  await test('mem_query → added node found by tag filter', () => {
    const result = callTool('mem_query', { tag: 'test', limit: 10 });
    const data = JSON.parse(result.content[0].text);
    assert(Array.isArray(data), 'result must be an array');
    const found = data.find((n) => n.id === createdId);
    assert(found, `Created node ${createdId} not found in query results`);
    assert(found.title === 'Test Node', 'title mismatch');
  });

  // 4. mem_search finds node by keyword
  await test('mem_search → keyword search returns matching node', () => {
    const result = callTool('mem_search', { query: 'Test Node' });
    const data = JSON.parse(result.content[0].text);
    assert(Array.isArray(data), 'result must be an array');
    assert(data.length > 0, 'Expected at least one search result');
    const found = data.find((n) => n.id === createdId);
    assert(found, `Created node ${createdId} not found in search results`);
  });

  // 5. mem_context returns project-filtered context
  await test('mem_context → project filter returns context', () => {
    const result = callTool('mem_context', { project: 'epic-harness', limit: 5 });
    const data = JSON.parse(result.content[0].text);
    assert(Array.isArray(data), 'result must be an array');
    assert(data.length > 0, 'Expected at least one context node');
    const found = data.find((n) => n.id === createdId);
    assert(found, `Created node ${createdId} not found in context`);
    assert(found.summary, 'context node must have summary field');
  });

  // 6. mem_related handles unknown id gracefully
  await test('mem_related → unknown id returns empty array without throwing', () => {
    const result = callTool('mem_related', { id: 'nonexistent-id', depth: 1 });
    const data = JSON.parse(result.content[0].text);
    assert(Array.isArray(data), 'result must be an array');
  });

  // 7. mem_add error → graceful content error (no exception)
  await test('mem_add → missing required field returns error in content', () => {
    const result = callTool('mem_add', { title: 'Bad Node' }); // missing type and body
    const data = JSON.parse(result.content[0].text);
    assert(data.error, 'Expected error field in result');
  });

  // ── Cleanup ───────────────────────────────────────────────────────────────
  fs.rmSync(tmpRoot, { recursive: true, force: true });

  console.log(`\n${passed + failed} tests: ${passed} passed, ${failed} failed\n`);
  process.exit(failed > 0 ? 1 : 0);
})();
