#!/usr/bin/env node
/**
 * Tests for mem-server.cjs
 * Run: node hooks/scripts/mem-server.test.cjs
 */
'use strict';

const http = require('http');
const fs = require('fs');
const path = require('path');
const os = require('os');
const { randomUUID } = require('crypto');

// ── Test harness ──────────────────────────────────────
let passed = 0;
let failed = 0;

function assert(condition, msg) {
  if (condition) {
    console.log(`  PASS: ${msg}`);
    passed++;
  } else {
    console.error(`  FAIL: ${msg}`);
    failed++;
  }
}

function assertEqual(a, b, msg) {
  const ok = JSON.stringify(a) === JSON.stringify(b);
  if (ok) {
    console.log(`  PASS: ${msg}`);
    passed++;
  } else {
    console.error(`  FAIL: ${msg} (got ${JSON.stringify(a)}, expected ${JSON.stringify(b)})`);
    failed++;
  }
}

// ── Load helpers from mem-server.cjs ─────────────────
let helpers;
try {
  helpers = require('./mem-server.cjs');
} catch (e) {
  console.error('Failed to load mem-server.cjs:', e.message);
  process.exit(1);
}

const { parseSimpleYaml, parseNodeFile, atomicWrite, startServer } = helpers;

// ── Unit tests: parseSimpleYaml ───────────────────────
console.log('\n[parseSimpleYaml]');
{
  const r = parseSimpleYaml('id: abc123\ntitle: Hello World\ntags: [a, b, c]');
  assertEqual(r.id, 'abc123', 'parses simple string value');
  assertEqual(r.title, 'Hello World', 'parses string with spaces');
  assertEqual(r.tags, ['a', 'b', 'c'], 'parses array value');
}
{
  const r = parseSimpleYaml('key: "quoted"');
  assertEqual(r.key, 'quoted', 'strips double quotes');
}
{
  const r = parseSimpleYaml("key: 'single'");
  assertEqual(r.key, 'single', 'strips single quotes');
}
{
  const r = parseSimpleYaml('tags: []');
  assertEqual(r.tags, [], 'parses empty array');
}

// ── Unit tests: parseNodeFile ─────────────────────────
console.log('\n[parseNodeFile]');
{
  const content = `---\nid: node1\ntitle: My Node\ntags: [foo, bar]\n---\n# Hello\n\nWorld`;
  const r = parseNodeFile(content);
  assert(r !== null, 'parses valid frontmatter');
  assertEqual(r.id, 'node1', 'extracts id');
  assertEqual(r.title, 'My Node', 'extracts title');
  assertEqual(r.tags, ['foo', 'bar'], 'extracts tags array');
  assertEqual(r.body, '# Hello\n\nWorld', 'extracts body');
}
{
  const r = parseNodeFile('no frontmatter here');
  assertEqual(r, null, 'returns null for missing frontmatter');
}

// ── Unit tests: atomicWrite ───────────────────────────
console.log('\n[atomicWrite]');
{
  const tmpFile = path.join(os.tmpdir(), `mem-server-test-${randomUUID()}.json`);
  atomicWrite(tmpFile, '{"ok":true}');
  const content = fs.readFileSync(tmpFile, 'utf8');
  assertEqual(content, '{"ok":true}', 'writes content atomically');
  assert(!fs.existsSync(tmpFile + '.tmp'), 'tmp file cleaned up');
  fs.unlinkSync(tmpFile);
}

// ── Integration tests: HTTP server ───────────────────
console.log('\n[HTTP integration]');

const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'mem-server-test-'));
const memDir = path.join(tmpRoot, 'memory');
const nodesDir = path.join(memDir, 'nodes');
fs.mkdirSync(nodesDir, { recursive: true });

const emptyIndex = { nodes: [], by_tag: {}, by_type: {}, by_project: {} };
fs.writeFileSync(path.join(memDir, 'index.json'), JSON.stringify(emptyIndex));
fs.writeFileSync(path.join(memDir, 'graph.json'), JSON.stringify({ nodes: [], edges: [] }));
fs.writeFileSync(path.join(memDir, 'edges.jsonl'), '');

const PORT = 17702;
process.env.HARNESS_ROOT = tmpRoot;

const server = startServer(PORT);

function req(method, urlPath, body) {
  return new Promise((resolve, reject) => {
    const options = {
      hostname: 'localhost',
      port: PORT,
      path: urlPath,
      method,
      headers: { 'Content-Type': 'application/json' },
    };
    const r = http.request(options, (res) => {
      let data = '';
      res.on('data', (c) => (data += c));
      res.on('end', () => resolve({ status: res.statusCode, headers: res.headers, body: data }));
    });
    r.on('error', reject);
    if (body) r.write(JSON.stringify(body));
    r.end();
  });
}

async function runIntegration() {
  // GET /api/graph
  {
    const r = await req('GET', '/api/graph');
    assertEqual(r.status, 200, 'GET /api/graph returns 200');
    const parsed = JSON.parse(r.body);
    assert('nodes' in parsed, 'graph has nodes key');
    assert(r.headers['access-control-allow-origin'] === '*', 'CORS header present');
  }

  // GET /api/nodes (empty)
  {
    const r = await req('GET', '/api/nodes');
    assertEqual(r.status, 200, 'GET /api/nodes returns 200');
    assertEqual(JSON.parse(r.body), [], 'empty nodes list');
  }

  // POST /api/nodes
  let createdId;
  {
    const r = await req('POST', '/api/nodes', {
      title: 'Test Node',
      type: 'note',
      tags: ['test'],
      project: 'epic-harness',
      body: '# Test\nHello world',
    });
    assertEqual(r.status, 201, 'POST /api/nodes returns 201');
    const parsed = JSON.parse(r.body);
    assert(typeof parsed.id === 'string', 'returned id is string');
    createdId = parsed.id;
  }

  // GET /api/nodes/:id
  {
    const r = await req('GET', `/api/nodes/${createdId}`);
    assertEqual(r.status, 200, 'GET /api/nodes/:id returns 200');
    const parsed = JSON.parse(r.body);
    assertEqual(parsed.title, 'Test Node', 'node title matches');
    assertEqual(parsed.body, '# Test\nHello world', 'node body matches');
  }

  // GET /api/nodes (now has one)
  {
    const r = await req('GET', '/api/nodes');
    assertEqual(JSON.parse(r.body).length, 1, 'nodes list has one entry');
  }

  // PUT /api/nodes/:id
  {
    const r = await req('PUT', `/api/nodes/${createdId}`, {
      title: 'Updated Node',
      type: 'note',
      tags: ['test', 'updated'],
      project: 'epic-harness',
      body: '# Updated\nNew content',
    });
    assertEqual(r.status, 200, 'PUT /api/nodes/:id returns 200');
  }

  // Verify update
  {
    const r = await req('GET', `/api/nodes/${createdId}`);
    assertEqual(JSON.parse(r.body).title, 'Updated Node', 'title updated');
  }

  // POST /api/edges
  let edgeId;
  {
    const r = await req('POST', '/api/edges', {
      source: createdId,
      target: 'other-node',
      label: 'relates_to',
    });
    assertEqual(r.status, 201, 'POST /api/edges returns 201');
    const parsed = JSON.parse(r.body);
    assert(typeof parsed.id === 'string', 'edge has id');
    edgeId = parsed.id;
  }

  // DELETE /api/edges/:id
  {
    const r = await req('DELETE', `/api/edges/${edgeId}`);
    assertEqual(r.status, 200, 'DELETE /api/edges/:id returns 200');
  }

  // GET /api/search?q=Updated
  {
    const r = await req('GET', '/api/search?q=Updated');
    assertEqual(r.status, 200, 'GET /api/search returns 200');
    const parsed = JSON.parse(r.body);
    assert(Array.isArray(parsed), 'search returns array');
    assert(parsed.length >= 1, 'search finds result');
  }

  // GET /api/nodes/:id not found
  {
    const r = await req('GET', '/api/nodes/nonexistent-id');
    assertEqual(r.status, 404, 'GET /api/nodes/:id 404 for missing node');
  }

  // DELETE /api/nodes/:id
  {
    const r = await req('DELETE', `/api/nodes/${createdId}`);
    assertEqual(r.status, 200, 'DELETE /api/nodes/:id returns 200');
  }

  // Verify deletion
  {
    const r = await req('GET', `/api/nodes/${createdId}`);
    assertEqual(r.status, 404, 'deleted node returns 404');
  }

  // GET / (webview)
  {
    const r = await req('GET', '/');
    assertEqual(r.status, 200, 'GET / returns 200');
    assert(r.body.includes('Memory'), 'GET / returns page with "Memory"');
  }
}

runIntegration()
  .then(() => {
    server.close();
    fs.rmSync(tmpRoot, { recursive: true, force: true });
    console.log(`\nResults: ${passed} passed, ${failed} failed`);
    process.exit(failed > 0 ? 1 : 0);
  })
  .catch((err) => {
    console.error('Integration test error:', err);
    server.close();
    fs.rmSync(tmpRoot, { recursive: true, force: true });
    process.exit(1);
  });
