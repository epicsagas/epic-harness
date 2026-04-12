'use strict';
/**
 * mem-mcp.cjs — Cross-Agent Unified Memory MCP Server (stdio transport)
 *
 * Implements JSON-RPC 2.0 over stdin/stdout per MCP protocol spec.
 * Node.js built-in modules only — no external npm packages.
 *
 * Tools: mem_add, mem_query, mem_search, mem_related, mem_context
 *
 * Usage: node hooks/scripts/mem-mcp.cjs
 * Env:   HARNESS_ROOT (default: ~/.harness)
 */

const fs = require('fs');
const path = require('path');
const os = require('os');
const { execFileSync } = require('child_process');
const { randomUUID } = require('crypto');

// ── Paths (resolved lazily so HARNESS_ROOT env override works in tests) ───────

function memoryDir() {
  const harnessRoot = process.env.HARNESS_ROOT || path.join(os.homedir(), '.harness');
  return path.join(harnessRoot, 'memory');
}

function nodesDir() {
  return path.join(memoryDir(), 'nodes');
}

function indexPath() {
  return path.join(memoryDir(), 'index.json');
}

function edgesPath() {
  return path.join(memoryDir(), 'edges.jsonl');
}

// ── Helpers (inlined from mem-server.cjs) ─────────────────────────────────────

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

function readIndex() {
  const ip = indexPath();
  if (!fs.existsSync(ip)) return { nodes: [], by_tag: {}, by_type: {}, by_project: {} };
  try {
    return JSON.parse(fs.readFileSync(ip, 'utf8'));
  } catch {
    return { nodes: [], by_tag: {}, by_type: {}, by_project: {} };
  }
}

function rebuildIndex() {
  const nd = nodesDir();
  if (!fs.existsSync(nd)) return { nodes: [], by_tag: {}, by_type: {}, by_project: {} };

  const index = { nodes: [], by_tag: {}, by_type: {}, by_project: {} };
  for (const file of fs.readdirSync(nd)) {
    if (!file.endsWith('.md')) continue;
    let node;
    try {
      const content = fs.readFileSync(path.join(nd, file), 'utf8');
      node = parseNodeFile(content);
    } catch {
      continue;
    }
    if (!node || !node.id) continue;
    const meta = node;
    index.nodes.push({
      id: meta.id,
      title: meta.title || '',
      type: meta.type || 'concept',
      tags: meta.tags || [],
      projects: meta.projects || [],
      updated: meta.updated || new Date().toISOString(),
    });
    const tags = Array.isArray(node.tags) ? node.tags : [];
    for (const tag of tags) {
      if (!index.by_tag[tag]) index.by_tag[tag] = [];
      index.by_tag[tag].push(node.id);
    }
    if (node.type) {
      if (!index.by_type[node.type]) index.by_type[node.type] = [];
      index.by_type[node.type].push(node.id);
    }
    if (node.project) {
      if (!index.by_project[node.project]) index.by_project[node.project] = [];
      index.by_project[node.project].push(node.id);
    }
  }
  atomicWrite(indexPath(), JSON.stringify(index, null, 2));
  return index;
}

function readEdges() {
  const ep = edgesPath();
  if (!fs.existsSync(ep)) return [];
  const lines = fs.readFileSync(ep, 'utf8').split('\n').filter(Boolean);
  const edges = [];
  for (const line of lines) {
    try { edges.push(JSON.parse(line)); } catch { /* skip */ }
  }
  return edges;
}

function loadNodeById(id) {
  const filePath = path.join(nodesDir(), `${id}.md`);
  if (!fs.existsSync(filePath)) return null;
  try {
    const content = fs.readFileSync(filePath, 'utf8');
    return parseNodeFile(content);
  } catch {
    return null;
  }
}

// ── MCP Tool definitions ──────────────────────────────────────────────────────

const TOOLS = [
  {
    name: 'mem_add',
    description:
      'Add a new memory node to the unified knowledge graph. Use for architectural decisions, patterns, recurring errors, or project-specific knowledge.',
    inputSchema: {
      type: 'object',
      properties: {
        title: { type: 'string', description: 'Short descriptive title' },
        type: {
          type: 'string',
          enum: ['concept', 'pattern', 'project', 'decision', 'error'],
          description: 'Node type',
        },
        body: { type: 'string', description: 'Markdown content (the actual knowledge)' },
        tags: { type: 'array', items: { type: 'string' }, description: 'Tags for filtering' },
        project: { type: 'string', description: 'Project slug (optional)' },
      },
      required: ['title', 'type', 'body'],
    },
  },
  {
    name: 'mem_query',
    description:
      'Query memory nodes by filter. Returns relevant memories for the current context.',
    inputSchema: {
      type: 'object',
      properties: {
        tag: { type: 'string' },
        type: {
          type: 'string',
          enum: ['concept', 'pattern', 'project', 'decision', 'error'],
        },
        project: { type: 'string' },
        limit: { type: 'number', default: 10 },
      },
    },
  },
  {
    name: 'mem_search',
    description:
      'Full-text search across all memory nodes. Use when you need to find specific knowledge by keyword.',
    inputSchema: {
      type: 'object',
      properties: {
        query: { type: 'string', description: 'Search keyword or phrase' },
      },
      required: ['query'],
    },
  },
  {
    name: 'mem_related',
    description:
      'Find nodes related to a given node via the knowledge graph edges.',
    inputSchema: {
      type: 'object',
      properties: {
        id: { type: 'string', description: 'Node ID' },
        depth: { type: 'number', default: 2, description: 'Graph traversal depth' },
      },
      required: ['id'],
    },
  },
  {
    name: 'mem_context',
    description:
      'Get relevant memory context for a project. Call at session start to load project-specific knowledge.',
    inputSchema: {
      type: 'object',
      properties: {
        project: { type: 'string', description: 'Project slug' },
        limit: { type: 'number', default: 5 },
      },
    },
  },
];

// ── Tool implementations ──────────────────────────────────────────────────────

function toolMemAdd(args) {
  const { title, type, body, tags, project } = args;
  if (!title || !type || !body) {
    return { error: 'mem_add requires title, type, and body' };
  }

  const nd = nodesDir();
  if (!fs.existsSync(nd)) fs.mkdirSync(nd, { recursive: true });

  const id = randomUUID();
  const now = new Date().toISOString();
  const fields = { id, title, type, created: now, updated: now };
  if (tags && tags.length) fields.tags = tags;
  if (project) fields.project = project;
  fields.body = body;

  const markdown = buildNodeMarkdown(fields);
  atomicWrite(path.join(nd, `${id}.md`), markdown);
  rebuildIndex();
  return { id, created: now };
}

function toolMemQuery(args) {
  const { tag, type, project, limit = 10 } = args;
  const index = readIndex();
  const nd = nodesDir();

  // Determine candidate ids from index filters
  let candidates = null;

  if (tag && index.by_tag[tag]) {
    candidates = new Set(index.by_tag[tag]);
  }
  if (type && index.by_type[type]) {
    const typeSet = new Set(index.by_type[type]);
    candidates = candidates
      ? new Set([...candidates].filter((id) => typeSet.has(id)))
      : typeSet;
  }
  if (project && index.by_project[project]) {
    const projSet = new Set(index.by_project[project]);
    candidates = candidates
      ? new Set([...candidates].filter((id) => projSet.has(id)))
      : projSet;
  }

  // If no filters supplied, use all nodes (may be id strings or IndexNode objects)
  const allIds = candidates
    ? [...candidates]
    : index.nodes.map((n) => (typeof n === 'object' ? n.id : n));

  const ids = allIds;

  const results = [];
  for (const id of ids) {
    if (results.length >= limit) break;
    if (!fs.existsSync(nd)) continue;
    const node = loadNodeById(id);
    if (!node) continue;
    results.push({
      id: node.id,
      title: node.title,
      type: node.type,
      tags: node.tags || [],
      updated: node.updated,
      project: node.project,
      body: (node.body || '').slice(0, 200),
    });
  }

  return results;
}

function toolMemSearch(args) {
  const { query } = args;
  if (!query) return { error: 'mem_search requires query' };

  const nd = nodesDir();
  if (!fs.existsSync(nd)) return [];

  let output = '';

  try {
    output = execFileSync('rg', ['-l', '--', query, nd], {
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'pipe'],
    });
  } catch {
    try {
      output = execFileSync('grep', ['-rl', '--', query, nd], {
        encoding: 'utf8',
        stdio: ['pipe', 'pipe', 'pipe'],
      });
    } catch (grepErr) {
      // exit code 1 means no matches; stdout is a string due to encoding: 'utf8'
      output = (typeof grepErr.stdout === 'string' ? grepErr.stdout : '') ;
    }
  }

  const files = output.split('\n').filter(Boolean);
  const results = [];
  for (const file of files.slice(0, 10)) {
    try {
      const content = fs.readFileSync(file, 'utf8');
      const node = parseNodeFile(content);
      if (!node) continue;

      // Find snippet containing the query (case-insensitive)
      const lower = content.toLowerCase();
      const idx = lower.indexOf(query.toLowerCase());
      const snippet = idx >= 0
        ? content.slice(Math.max(0, idx - 40), idx + 120).replace(/\n/g, ' ')
        : '';

      results.push({ id: node.id, title: node.title, type: node.type, snippet });
    } catch {
      // skip unreadable files
    }
  }
  return results;
}

function toolMemRelated(args) {
  const { id, depth = 2 } = args;
  if (!id) return { error: 'mem_related requires id' };

  const edges = readEdges();
  if (!edges.length) return [];

  // BFS from id up to depth
  const visited = new Set([id]);
  const queue = [{ id, d: 0 }];
  const related = [];

  while (queue.length) {
    const { id: current, d } = queue.shift();
    if (d >= depth) continue;

    for (const edge of edges) {
      let neighbor = null;
      if (edge.source === current && !visited.has(edge.target)) neighbor = edge.target;
      if (edge.target === current && !visited.has(edge.source)) neighbor = edge.source;
      if (!neighbor) continue;

      visited.add(neighbor);
      queue.push({ id: neighbor, d: d + 1 });

      const node = loadNodeById(neighbor);
      related.push({
        id: neighbor,
        title: node ? node.title : null,
        type: node ? node.type : null,
        relation: edge.relation || 'related',
        depth: d + 1,
      });
    }
  }

  return related;
}

function toolMemContext(args) {
  const { project, limit = 5 } = args;
  const index = readIndex();

  let ids;
  if (project && index.by_project[project]) {
    ids = index.by_project[project];
  } else {
    ids = index.nodes.map((n) => (typeof n === 'object' ? n.id : n));
  }

  // Sort by updated (most recent first) — load to compare
  const nodes = [];
  for (const id of ids) {
    const node = loadNodeById(id);
    if (node) nodes.push(node);
  }
  nodes.sort((a, b) => {
    const ta = a.updated || a.created || '';
    const tb = b.updated || b.created || '';
    return tb.localeCompare(ta);
  });

  return nodes.slice(0, limit).map((n) => ({
    id: n.id,
    title: n.title,
    type: n.type,
    tags: n.tags || [],
    updated: n.updated,
    summary: (n.body || '').slice(0, 300),
  }));
}

// ── callTool dispatcher ────────────────────────────────────────────────────────

function callTool(name, args) {
  let result;
  try {
    switch (name) {
      case 'mem_add':
        result = toolMemAdd(args);
        break;
      case 'mem_query':
        result = toolMemQuery(args);
        break;
      case 'mem_search':
        result = toolMemSearch(args);
        break;
      case 'mem_related':
        result = toolMemRelated(args);
        break;
      case 'mem_context':
        result = toolMemContext(args);
        break;
      default:
        result = { error: `Unknown tool: ${name}` };
    }
  } catch (e) {
    result = { error: e.message };
  }
  return { content: [{ type: 'text', text: JSON.stringify(result, null, 2) }] };
}

// ── JSON-RPC handler ──────────────────────────────────────────────────────────

function send(obj) {
  process.stdout.write(JSON.stringify(obj) + '\n');
}

function handleMessage(msg) {
  if (!msg || typeof msg !== 'object') return;

  if (msg.method === 'initialize') {
    send({
      jsonrpc: '2.0',
      id: msg.id,
      result: {
        protocolVersion: '2024-11-05',
        capabilities: { tools: {} },
        serverInfo: { name: 'harness-mem', version: '1.0.0' },
      },
    });
  } else if (msg.method === 'notifications/initialized') {
    // no-op — client notification, no response needed
  } else if (msg.method === 'tools/list') {
    send({ jsonrpc: '2.0', id: msg.id, result: { tools: TOOLS } });
  } else if (msg.method === 'tools/call') {
    const toolName = msg.params && msg.params.name;
    const toolArgs = (msg.params && msg.params.arguments) || {};
    const result = callTool(toolName, toolArgs);
    send({ jsonrpc: '2.0', id: msg.id, result });
  } else {
    if (msg.id !== undefined) {
      send({
        jsonrpc: '2.0',
        id: msg.id,
        error: { code: -32601, message: 'Method not found' },
      });
    }
  }
}

// ── Entry point (guarded for test import) ─────────────────────────────────────

if (require.main === module) {
  process.stdin.setEncoding('utf8');
  let buf = '';
  process.stdin.on('data', (chunk) => {
    buf += chunk;
    const lines = buf.split('\n');
    buf = lines.pop(); // hold back incomplete line
    for (const line of lines) {
      if (!line.trim()) continue;
      try {
        const msg = JSON.parse(line);
        handleMessage(msg);
      } catch {
        // ignore parse errors
      }
    }
  });
  process.stdin.on('end', () => {
    if (buf.trim()) {
      try {
        handleMessage(JSON.parse(buf));
      } catch { /* ignore */ }
    }
  });
}

module.exports = { callTool, TOOLS, parseSimpleYaml, parseNodeFile };
