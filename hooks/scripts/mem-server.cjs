#!/usr/bin/env node
/**
 * mem-server.cjs — Cross-Agent Unified Memory HTTP Server (Node.js fallback)
 *
 * Usage: node hooks/scripts/mem-server.cjs --port 7700
 * Env:   HARNESS_ROOT (default: ~/.harness)
 *
 * REST API:
 *   GET    /                  → webview.html
 *   GET    /api/graph         → graph.json
 *   GET    /api/nodes         → index.json nodes[]
 *   GET    /api/nodes/:id     → nodes/{id}.md → JSON
 *   POST   /api/nodes         → create node
 *   PUT    /api/nodes/:id     → update node
 *   DELETE /api/nodes/:id     → delete node + edges
 *   POST   /api/edges         → append edge
 *   DELETE /api/edges/:id     → remove edge line
 *   GET    /api/search?q=...  → grep nodes/
 */
'use strict';

const http = require('http');
const fs = require('fs');
const path = require('path');
const os = require('os');
const { execFile } = require('child_process');
const { randomUUID } = require('crypto');

// ── Helpers ───────────────────────────────────────────

function parseSimpleYaml(text) {
  const result = {};
  for (const line of text.split('\n')) {
    const m = line.match(/^(\w+):\s*(.*)$/);
    if (!m) continue;
    const [, key, val] = m;
    if (val.startsWith('[')) {
      const inner = val.slice(1, -1).trim();
      result[key] = inner === '' ? [] : inner.split(',').map((s) => s.trim()).filter(Boolean);
    } else {
      result[key] = val.replace(/^["']|["']$/g, '');
    }
  }
  return result;
}

function parseNodeFile(content) {
  const match = content.match(/^---\n([\s\S]*?)\n---\n([\s\S]*)$/);
  if (!match) return null;
  const meta = parseSimpleYaml(match[1]);
  return { ...meta, body: match[2].trim() };
}

function atomicWrite(filePath, content) {
  const tmp = filePath + '.tmp';
  fs.writeFileSync(tmp, content, 'utf8');
  fs.renameSync(tmp, filePath);
}

// ── Memory root ───────────────────────────────────────

function getMemRoot() {
  const harnessRoot = process.env.HARNESS_ROOT || path.join(os.homedir(), '.harness');
  return path.join(harnessRoot, 'memory');
}

// ── index.json management ─────────────────────────────

function loadIndex(memRoot) {
  const indexPath = path.join(memRoot, 'index.json');
  if (!fs.existsSync(indexPath)) {
    return { nodes: [], by_tag: {}, by_type: {}, by_project: {} };
  }
  try {
    return JSON.parse(fs.readFileSync(indexPath, 'utf8'));
  } catch {
    return { nodes: [], by_tag: {}, by_type: {}, by_project: {} };
  }
}

function rebuildIndex(memRoot) {
  const nodesDir = path.join(memRoot, 'nodes');
  if (!fs.existsSync(nodesDir)) return;

  const index = { nodes: [], by_tag: {}, by_type: {}, by_project: {} };

  for (const file of fs.readdirSync(nodesDir)) {
    if (!file.endsWith('.md')) continue;
    const content = fs.readFileSync(path.join(nodesDir, file), 'utf8');
    const node = parseNodeFile(content);
    if (!node || !node.id) continue;

    index.nodes.push({
      id: node.id,
      title: node.title || '',
      type: node.type || '',
      tags: Array.isArray(node.tags) ? node.tags : [],
      projects: node.project ? [node.project] : [],
      updated: node.updated || '',
    });

    // by_tag
    const tags = Array.isArray(node.tags) ? node.tags : [];
    for (const tag of tags) {
      if (!index.by_tag[tag]) index.by_tag[tag] = [];
      index.by_tag[tag].push(node.id);
    }

    // by_type
    if (node.type) {
      if (!index.by_type[node.type]) index.by_type[node.type] = [];
      index.by_type[node.type].push(node.id);
    }

    // by_project
    if (node.project) {
      if (!index.by_project[node.project]) index.by_project[node.project] = [];
      index.by_project[node.project].push(node.id);
    }
  }

  atomicWrite(path.join(memRoot, 'index.json'), JSON.stringify(index, null, 2));
  return index;
}

function rebuildMaps(nodes) {
  const by_tag = {};
  const by_type = {};
  const by_project = {};
  for (const n of nodes) {
    for (const tag of (n.tags || [])) {
      if (!by_tag[tag]) by_tag[tag] = [];
      by_tag[tag].push(n.id);
    }
    if (n.type) {
      if (!by_type[n.type]) by_type[n.type] = [];
      by_type[n.type].push(n.id);
    }
    for (const proj of (n.projects || [])) {
      if (!by_project[proj]) by_project[proj] = [];
      by_project[proj].push(n.id);
    }
  }
  return { by_tag, by_type, by_project };
}

function upsertIndex(memRoot, nodeObj) {
  const index = loadIndex(memRoot);
  // Remove existing entry for this id
  index.nodes = index.nodes.filter((n) => n.id !== nodeObj.id);
  // Push new IndexNode
  index.nodes.push({
    id: nodeObj.id,
    title: nodeObj.title || '',
    type: nodeObj.type || '',
    tags: Array.isArray(nodeObj.tags) ? nodeObj.tags : [],
    projects: nodeObj.project ? [nodeObj.project] : (nodeObj.projects || []),
    updated: nodeObj.updated || '',
  });
  const maps = rebuildMaps(index.nodes);
  const updated = { nodes: index.nodes, ...maps };
  atomicWrite(path.join(memRoot, 'index.json'), JSON.stringify(updated, null, 2));
  return updated;
}

function removeFromIndex(memRoot, id) {
  const index = loadIndex(memRoot);
  index.nodes = index.nodes.filter((n) => n.id !== id);
  const maps = rebuildMaps(index.nodes);
  const updated = { nodes: index.nodes, ...maps };
  atomicWrite(path.join(memRoot, 'index.json'), JSON.stringify(updated, null, 2));
  return updated;
}

function execAsync(cmd, args) {
  return new Promise((resolve, reject) => {
    execFile(cmd, args, { encoding: 'utf8', timeout: 5000 }, (err, stdout) => {
      if (err && err.code !== 1) reject(err);
      else resolve(stdout || '');
    });
  });
}

function buildNodeMarkdown(fields) {
  const { body = '', ...meta } = fields;
  const lines = [];
  for (const [k, v] of Object.entries(meta)) {
    if (Array.isArray(v)) {
      lines.push(`${k}: [${v.join(', ')}]`);
    } else {
      lines.push(`${k}: ${v}`);
    }
  }
  return `---\n${lines.join('\n')}\n---\n${body}`;
}

// ── Request routing ───────────────────────────────────

function send(res, status, body, contentType = 'application/json') {
  const data = typeof body === 'string' ? body : JSON.stringify(body);
  res.writeHead(status, {
    'Content-Type': contentType,
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type',
  });
  res.end(data);
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    let data = '';
    req.on('data', (c) => (data += c));
    req.on('end', () => {
      try {
        resolve(data ? JSON.parse(data) : {});
      } catch (e) {
        reject(e);
      }
    });
    req.on('error', reject);
  });
}

async function handleRequest(req, res) {
  const memRoot = getMemRoot();
  const url = new URL(req.url, `http://localhost`);
  const pathname = url.pathname;
  const method = req.method.toUpperCase();

  // CORS preflight
  if (method === 'OPTIONS') {
    return send(res, 204, '');
  }

  // GET /
  if (method === 'GET' && pathname === '/') {
    const webviewPath = path.join(__dirname, '..', '..', 'src', 'hooks', 'mem', 'webview.html');
    if (fs.existsSync(webviewPath)) {
      const html = fs.readFileSync(webviewPath, 'utf8');
      return send(res, 200, html, 'text/html');
    }
    return send(
      res,
      200,
      `<!DOCTYPE html><html><body><h1>Memory UI loading...</h1></body></html>`,
      'text/html'
    );
  }

  // GET /api/graph
  if (method === 'GET' && pathname === '/api/graph') {
    const graphPath = path.join(memRoot, 'graph.json');
    if (!fs.existsSync(graphPath)) return send(res, 200, { nodes: [], edges: [] });
    try {
      const data = JSON.parse(fs.readFileSync(graphPath, 'utf8'));
      return send(res, 200, data);
    } catch (e) {
      process.stderr.write(`graph.json parse error: ${e.message}\n`);
      return send(res, 400, { error: 'Failed to parse graph.json' });
    }
  }

  // GET /api/nodes
  if (method === 'GET' && pathname === '/api/nodes') {
    const index = loadIndex(memRoot);
    return send(res, 200, index.nodes);
  }

  // GET /api/nodes/:id
  const nodeMatch = pathname.match(/^\/api\/nodes\/([^/]+)$/);
  if (nodeMatch) {
    const id = nodeMatch[1];
    const nodesDir = path.join(memRoot, 'nodes');
    const filePath = path.join(nodesDir, `${id}.md`);

    if (method === 'GET') {
      if (!fs.existsSync(filePath)) return send(res, 404, { error: 'Node not found' });
      try {
        const content = fs.readFileSync(filePath, 'utf8');
        const node = parseNodeFile(content);
        if (!node) return send(res, 400, { error: 'Failed to parse node file' });
        return send(res, 200, node);
      } catch (e) {
        process.stderr.write(`GET /api/nodes/${id}: ${e.message}\n`);
        return send(res, 500, { error: e.message });
      }
    }

    if (method === 'PUT') {
      try {
        const fields = await readBody(req);
        if (!fs.existsSync(filePath)) return send(res, 404, { error: 'Node not found' });
        const updated = new Date().toISOString();
        const nodeData = { ...fields, id, updated };
        const markdown = buildNodeMarkdown(nodeData);
        if (!fs.existsSync(nodesDir)) fs.mkdirSync(nodesDir, { recursive: true });
        atomicWrite(filePath, markdown);
        upsertIndex(memRoot, nodeData);
        return send(res, 200, { id, updated });
      } catch (e) {
        process.stderr.write(`PUT /api/nodes/${id}: ${e.message}\n`);
        return send(res, 400, { error: e.message });
      }
    }

    if (method === 'DELETE') {
      if (!fs.existsSync(filePath)) return send(res, 404, { error: 'Node not found' });
      try {
        fs.unlinkSync(filePath);

        // Clean edges.jsonl
        const edgesPath = path.join(memRoot, 'edges.jsonl');
        if (fs.existsSync(edgesPath)) {
          const lines = fs.readFileSync(edgesPath, 'utf8').split('\n');
          const filtered = lines.filter((l) => {
            if (!l.trim()) return false;
            try {
              const e = JSON.parse(l);
              return e.source !== id && e.target !== id;
            } catch {
              return true;
            }
          });
          atomicWrite(edgesPath, filtered.join('\n') + (filtered.length ? '\n' : ''));
        }

        removeFromIndex(memRoot, id);
        return send(res, 200, { id, deleted: true });
      } catch (e) {
        process.stderr.write(`DELETE /api/nodes/${id}: ${e.message}\n`);
        return send(res, 500, { error: e.message });
      }
    }
  }

  // POST /api/nodes
  if (method === 'POST' && pathname === '/api/nodes') {
    try {
      const fields = await readBody(req);
      const id = randomUUID();
      const created = new Date().toISOString();
      const nodesDir = path.join(memRoot, 'nodes');
      if (!fs.existsSync(nodesDir)) fs.mkdirSync(nodesDir, { recursive: true });
      const nodeData = { ...fields, id, created, updated: created };
      const markdown = buildNodeMarkdown(nodeData);
      atomicWrite(path.join(nodesDir, `${id}.md`), markdown);
      upsertIndex(memRoot, nodeData);
      return send(res, 201, { id, created });
    } catch (e) {
      process.stderr.write(`POST /api/nodes: ${e.message}\n`);
      return send(res, 400, { error: e.message });
    }
  }

  // POST /api/edges
  if (method === 'POST' && pathname === '/api/edges') {
    readBody(req)
      .then((fields) => {
        const id = randomUUID();
        const created = new Date().toISOString();
        const edge = { id, ...fields, created };
        const edgesPath = path.join(memRoot, 'edges.jsonl');
        fs.appendFileSync(edgesPath, JSON.stringify(edge) + '\n', 'utf8');
        return send(res, 201, edge);
      })
      .catch((e) => {
        process.stderr.write(`POST /api/edges: ${e.message}\n`);
        send(res, 400, { error: e.message });
      });
    return;
  }

  // DELETE /api/edges/:id
  const edgeMatch = pathname.match(/^\/api\/edges\/([^/]+)$/);
  if (edgeMatch && method === 'DELETE') {
    const id = edgeMatch[1];
    const edgesPath = path.join(memRoot, 'edges.jsonl');
    try {
      if (!fs.existsSync(edgesPath)) return send(res, 404, { error: 'edges.jsonl not found' });
      const lines = fs.readFileSync(edgesPath, 'utf8').split('\n');
      let found = false;
      const filtered = lines.filter((l) => {
        if (!l.trim()) return false;
        try {
          const e = JSON.parse(l);
          if (e.id === id) { found = true; return false; }
          return true;
        } catch {
          return true;
        }
      });
      if (!found) return send(res, 404, { error: 'Edge not found' });
      atomicWrite(edgesPath, filtered.join('\n') + (filtered.length ? '\n' : ''));
      return send(res, 200, { id, deleted: true });
    } catch (e) {
      process.stderr.write(`DELETE /api/edges/${id}: ${e.message}\n`);
      return send(res, 500, { error: e.message });
    }
  }

  // GET /api/search?q=...
  if (method === 'GET' && pathname === '/api/search') {
    const q = url.searchParams.get('q') || '';
    if (!q) return send(res, 200, []);
    const nodesDir = path.join(memRoot, 'nodes');
    if (!fs.existsSync(nodesDir)) return send(res, 200, []);

    try {
      let output = '';
      try {
        output = await execAsync('rg', ['-l', '--', q, nodesDir]);
      } catch {
        try {
          output = await execAsync('grep', ['-rl', '--include=*.md', '--', q, nodesDir]);
        } catch (grepErr) {
          output = '';
        }
      }

      const files = output.split('\n').filter(Boolean);
      const results = [];
      for (const file of files) {
        try {
          const content = fs.readFileSync(file, 'utf8');
          const node = parseNodeFile(content);
          if (node) results.push(node);
        } catch {
          // skip unreadable files
        }
      }
      return send(res, 200, results);
    } catch (e) {
      process.stderr.write(`GET /api/search: ${e.message}\n`);
      return send(res, 500, { error: e.message });
    }
  }

  // 404 fallback
  send(res, 404, { error: 'Not found' });
}

// ── Server factory ────────────────────────────────────

function startServer(port) {
  const server = http.createServer(handleRequest);
  server.listen(port, '127.0.0.1');
  return server;
}

// ── CLI entry point ───────────────────────────────────

if (require.main === module) {
  const args = process.argv.slice(2);
  let port = 7700;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--port' && args[i + 1]) {
      port = parseInt(args[i + 1], 10);
    }
  }

  const memRoot = getMemRoot();
  const server = startServer(port);
  server.on('listening', () => {
    process.stdout.write(`Listening on http://localhost:${port}\n`);
    process.stdout.write(`Memory root: ${memRoot}/\n`);
  });
}

module.exports = { parseSimpleYaml, parseNodeFile, atomicWrite, startServer };

/*
 * 실행: node hooks/scripts/mem-server.cjs --port 7700
 * 테스트:
 *   curl http://localhost:7700/api/nodes
 *   curl -X POST http://localhost:7700/api/nodes \
 *     -H 'Content-Type: application/json' \
 *     -d '{"title":"Hello","type":"note","tags":["test"],"body":"# Hello"}'
 *   curl http://localhost:7700/api/search?q=Hello
 */
