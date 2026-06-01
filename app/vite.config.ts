import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';
import { viteSingleFile } from 'vite-plugin-singlefile';
import path from 'path';
import { execSync, execFileSync } from 'child_process';
import fs from 'fs';
import type { Plugin } from 'vite';

const rootPkg = JSON.parse(fs.readFileSync(path.resolve(__dirname, '..', 'package.json'), 'utf8'));
const host = process.env.TAURI_DEV_HOST;

function getHarnessDir(): string {
  // epic-harness path resolves the slug from the git worktree's commondir (the
  // original repo), not from a linked worktree. Walk up from app/ to find the
  // directory whose .git is a file (worktree) or a real directory with a
  // "worktrees" subdirectory (main clone), then prefer the main clone path.
  try {
    // First try: resolve via git commondir so we always get the main clone root.
    const commonDir = execSync('git rev-parse --git-common-dir', {
      encoding: 'utf8',
      cwd: path.resolve(__dirname, '..'),
    }).trim();
    // commonDir is the .git of the main clone (e.g. /repo/.git)
    const mainCloneRoot = path.resolve(commonDir, '..');
    return execSync('epic-harness path', { encoding: 'utf8', cwd: mainCloneRoot }).trim();
  } catch {
    return '';
  }
}

function harnessApiPlugin(): Plugin {
  return {
    name: 'harness-api',
    apply: 'serve',
    configureServer(server) {
      const harnessDir = getHarnessDir();

      // SPA fallback: serve index.html for non-API, non-static GET requests
      server.middlewares.use((req, res, next) => {
        const url = req.url ?? '/';
        if (req.method === 'GET' && !url.startsWith('/api/') && !url.includes('.')) {
          req.url = '/index.html';
          // Let Vite's built-in middleware handle the rewritten URL
        }
        next();
      });

      // Orbit pipeline dismiss — DELETE /api/orbit/:id
      server.middlewares.use('/api/orbit', (req, res, next) => {
        if (req.method !== 'DELETE') { next(); return; }
        const match = (req.url ?? '').match(/^\/([^/]+)$/);
        if (!match) { res.statusCode = 400; res.end(JSON.stringify({ ok: false, error: 'missing id' })); return; }
        const id = decodeURIComponent(match[1]);
        const projectsRoot = path.resolve(harnessDir, '..');
        let deleted = false;
        if (fs.existsSync(projectsRoot)) {
          for (const proj of fs.readdirSync(projectsRoot)) {
            const orbitDir = path.join(projectsRoot, proj, 'orbit');
            if (!fs.existsSync(orbitDir)) continue;
            for (const f of fs.readdirSync(orbitDir)) {
              if (!f.startsWith('PIPELINE-') || !f.endsWith('.json') || !f.includes(id)) continue;
              fs.unlinkSync(path.join(orbitDir, f));
              deleted = true;
            }
          }
        }
        res.end(JSON.stringify({ ok: deleted, dismissed: id }));
      });

      // Agent tracking routes for the live Agents tab
      server.middlewares.use('/api/run', (_req, res) => {
        res.setHeader('Content-Type', 'application/json');
        if (!harnessDir) { res.end(JSON.stringify(null)); return; }
        const runPath = path.join(harnessDir, 'orchestrator', 'run.json');
        if (!fs.existsSync(runPath)) { res.end(JSON.stringify(null)); return; }
        try { res.end(fs.readFileSync(runPath, 'utf8')); }
        catch { res.end(JSON.stringify(null)); }
      });
      server.middlewares.use('/api/agents', (req, res) => {
        res.setHeader('Content-Type', 'application/json');
        if (!harnessDir) { res.end(JSON.stringify(null)); return; }
        const match = (req.url ?? '').match(/^\/([^/]+)\/status$/);
        if (!match) { res.end(JSON.stringify(null)); return; }
        const agentId = match[1];
        // Validate agentId: only alphanumeric, dash, underscore (path traversal defense)
        if (!/^[a-zA-Z0-9_-]+$/.test(agentId)) { res.end(JSON.stringify(null)); return; }
        const agentsBase = path.resolve(harnessDir, 'orchestrator', 'agents');
        const statusPath = path.resolve(agentsBase, agentId, 'status.json');
        if (!statusPath.startsWith(agentsBase + path.sep)) { res.end(JSON.stringify(null)); return; }
        if (!fs.existsSync(statusPath)) { res.end(JSON.stringify(null)); return; }
        try { res.end(fs.readFileSync(statusPath, 'utf8')); }
        catch { res.end(JSON.stringify(null)); }
      });

      // Agent dismiss — DELETE /api/agents/:id
      server.middlewares.use('/api/agents', (req, res, next) => {
        if (req.method !== 'DELETE') { next(); return; }
        const match = (req.url ?? '').match(/^\/([^/]+)$/);
        if (!match) { res.statusCode = 400; res.end(JSON.stringify({ ok: false, error: 'missing id' })); return; }
        const agentId = decodeURIComponent(match[1]);
        if (!/^[a-zA-Z0-9_-]+$/.test(agentId)) { res.statusCode = 400; res.end(JSON.stringify({ ok: false, error: 'invalid agent id' })); return; }
        if (!harnessDir) { res.end(JSON.stringify({ ok: false, error: 'HARNESS_DIR not found' })); return; }
        // Remove agent from run.json
        const runPath = path.join(harnessDir, 'orchestrator', 'run.json');
        let dismissed = false;
        if (fs.existsSync(runPath)) {
          try {
            const run = JSON.parse(fs.readFileSync(runPath, 'utf8'));
            const before = run.agents?.length ?? 0;
            if (run.agents) {
              run.agents = run.agents.filter((a: Record<string, unknown>) => a.id !== agentId);
            }
            if (run.dependency_graph) {
              delete run.dependency_graph[agentId];
            }
            if (run.agents?.length < before) {
              fs.writeFileSync(runPath, JSON.stringify(run, null, 2));
              dismissed = true;
            }
          } catch { /* ignore */ }
        }
        // Remove agent status directory
        const agentDir = path.resolve(harnessDir, 'orchestrator', 'agents', agentId);
        if (agentDir.startsWith(path.resolve(harnessDir, 'orchestrator', 'agents') + path.sep) && fs.existsSync(agentDir)) {
          fs.rmSync(agentDir, { recursive: true, force: true });
          dismissed = true;
        }
        res.setHeader('Content-Type', 'application/json');
        res.end(JSON.stringify({ ok: dismissed, dismissed: agentId }));
      });

      server.middlewares.use('/api/harness', (req, res) => {
        const cmd = new URL(req.url ?? '/', 'http://localhost').searchParams.get('cmd') ?? '';
        res.setHeader('Content-Type', 'application/json');
        if (!harnessDir) {
          res.end(JSON.stringify({ error: 'HARNESS_DIR not found' }));
          return;
        }
        try {
          let data: unknown = null;

          if (cmd === 'get_harness_metrics') {
            const p = path.join(harnessDir, 'metrics.json');
            data = fs.existsSync(p) ? JSON.parse(fs.readFileSync(p, 'utf8')) : null;

          } else if (cmd === 'get_evolved_skills') {
            const evolvedDir = path.join(harnessDir, 'evolved');
            const skills = fs.existsSync(evolvedDir)
              ? fs.readdirSync(evolvedDir)
                  .filter(n => fs.statSync(path.join(evolvedDir, n)).isDirectory())
                  .map(name => {
                    const skillMd = path.join(evolvedDir, name, 'SKILL.md');
                    return {
                      name,
                      skill_md: fs.existsSync(skillMd) ? fs.readFileSync(skillMd, 'utf8') : '',
                      created_at: null,
                    };
                  })
              : [];
            const evoLog = path.join(harnessDir, 'evolution.jsonl');
            const history = fs.existsSync(evoLog)
              ? fs.readFileSync(evoLog, 'utf8').trim().split('\n').filter(Boolean).slice(-50)
                  .map(l => { try { return JSON.parse(l); } catch { return null; } })
                  .filter(Boolean)
                  .reverse()
              : [];
            const metricsPath = path.join(harnessDir, 'metrics.json');
            const metrics = fs.existsSync(metricsPath)
              ? JSON.parse(fs.readFileSync(metricsPath, 'utf8'))
              : {};
            data = {
              evolved_skills: skills,
              evolution_history: history,
              total_sessions_analyzed: metrics.total_sessions ?? 0,
              patterns_detected: history.length,
            };

          } else if (cmd === 'get_obs_summary') {
            const obsDir = path.join(harnessDir, 'obs');
            data = { recent_sessions: [], tool_stats: [], total_tool_calls: 0, avg_score: 0, active_agents: [] };
            if (fs.existsSync(obsDir)) {
              // all files, sorted — session_{date}_{pid}.jsonl
              const files = fs.readdirSync(obsDir).filter(f => f.endsWith('.jsonl')).sort();
              const toolMap: Record<string, { calls: number; successes: number; score_sum: number }> = {};
              // session-level aggregation: filename → stats
              const sessionMap: Record<string, { tool_calls: number; score_sum: number; failures: number }> = {};

              for (const f of files) {
                const sessionKey = f.replace('.jsonl', '');
                // extract date from filename: session_YYYYMMDD_pid.jsonl
                const dateMatch = f.match(/session_(\d{8})/);
                const date = dateMatch ? dateMatch[1] : 'unknown';
                if (!sessionMap[sessionKey]) sessionMap[sessionKey] = { tool_calls: 0, score_sum: 0, failures: 0 };

                const lines = fs.readFileSync(path.join(obsDir, f), 'utf8')
                  .trim().split('\n').filter(Boolean).slice(-10000);
                for (const l of lines) {
                  try {
                    const e = JSON.parse(l) as Record<string, unknown>;
                    const t = (e.tool as string) ?? 'unknown';
                    // real schema: result="success"|"failure", score (float)
                    const isSuccess = e.result === 'success' || e.tool_success === true;
                    const score = (e.score as number) ?? (e.composite_score as number) ?? 0;

                    if (!toolMap[t]) toolMap[t] = { calls: 0, successes: 0, score_sum: 0 };
                    toolMap[t].calls++;
                    if (isSuccess) toolMap[t].successes++;
                    toolMap[t].score_sum += score;

                    sessionMap[sessionKey].tool_calls++;
                    sessionMap[sessionKey].score_sum += score;
                    if (!isSuccess) sessionMap[sessionKey].failures++;
                  } catch { /* skip malformed lines */ }
                }
                (sessionMap[sessionKey] as Record<string, unknown>)['date'] = date;
                (sessionMap[sessionKey] as Record<string, unknown>)['session_id'] = sessionKey;
              }

              const tool_stats = Object.entries(toolMap)
                .map(([tool, s]) => ({
                  tool,
                  calls: s.calls,
                  success_rate: s.calls ? Math.round((s.successes / s.calls) * 1000) / 1000 : 0,
                  avg_score: s.calls ? Math.round((s.score_sum / s.calls) * 1000) / 1000 : 0,
                }))
                .sort((a, b) => b.calls - a.calls);

              const recent_sessions = Object.values(sessionMap)
                .sort((a, b) => String((b as Record<string,unknown>)['session_id']).localeCompare(String((a as Record<string,unknown>)['session_id'])))
                .slice(0, 10)
                .map(s => {
                  const rec = s as Record<string, unknown>;
                  return {
                    session_id: rec['session_id'] as string,
                    date: rec['date'] as string,
                    tool_calls: rec['tool_calls'] as number,
                    avg_score: rec['tool_calls'] ? Math.round(((rec['score_sum'] as number) / (rec['tool_calls'] as number)) * 1000) / 1000 : 0,
                    failures: rec['failures'] as number,
                  };
                });

              const total = tool_stats.reduce((s, t) => s + t.calls, 0);
              const avg = total
                ? tool_stats.reduce((s, t) => s + t.avg_score * t.calls, 0) / total
                : 0;
              data = {
                recent_sessions,
                tool_stats,
                total_tool_calls: total,
                avg_score: Math.round(avg * 1000) / 1000,
                active_agents: [],
              };
            }

          } else if (cmd === 'get_orbit_pipelines') {
            // Scan ALL projects under ~/.harness/projects/ for pipeline files
            const projectsRoot = path.resolve(harnessDir, '..');
            const allPipelines: unknown[] = [];
            if (fs.existsSync(projectsRoot)) {
              for (const proj of fs.readdirSync(projectsRoot)) {
                const orbitDir = path.join(projectsRoot, proj, 'orbit');
                if (!fs.existsSync(orbitDir)) continue;
                for (const f of fs.readdirSync(orbitDir)) {
                  if (!f.startsWith('PIPELINE-') || !f.endsWith('.json')) continue;
                  try {
                    const p = JSON.parse(fs.readFileSync(path.join(orbitDir, f), 'utf8'));
                    // annotate with project slug for display
                    p._project = proj;
                    allPipelines.push(p);
                  } catch { /* skip corrupt files */ }
                }
              }
            }
            // sort by started_at descending
            allPipelines.sort((a, b) => {
              const ta = (a as Record<string,string>)['started_at'] ?? '';
              const tb = (b as Record<string,string>)['started_at'] ?? '';
              return tb.localeCompare(ta);
            });
            data = allPipelines;

          } else if (cmd === 'get_integration_status') {
            const home = process.env.HOME ?? '';
            data = [
              { name: 'Claude Code', installed: fs.existsSync(path.join(home, '.claude', 'settings.json')), config_path: '~/.claude/settings.json', version: null },
              { name: 'Codex', installed: false, config_path: null, version: null },
              { name: 'Gemini CLI', installed: fs.existsSync(path.join(home, '.gemini', 'settings.json')), config_path: null, version: null },
              { name: 'Cursor', installed: false, config_path: null, version: null },
              { name: 'Cline', installed: false, config_path: null, version: null },
              { name: 'Aider', installed: false, config_path: null, version: null },
            ];

          } else if (cmd === 'get_graph') {
            const dbPath = path.resolve(harnessDir, '..', '..', 'memory.db'); // ~/.harness/memory.db
            data = { nodes: [], edges: [] };
            if (fs.existsSync(dbPath)) {
              try {
                const nodesJson = execFileSync('sqlite3', ['-json', dbPath, 'SELECT id, type, title, tags, importance FROM nodes WHERE type != \'session\' ORDER BY importance DESC LIMIT 200'], { encoding: 'utf8' }).trim();
                const edgesJson = execFileSync('sqlite3', ['-json', dbPath, 'SELECT source, target, relation, weight FROM edges LIMIT 500'], { encoding: 'utf8' }).trim();
                const rawNodes = nodesJson ? (JSON.parse(nodesJson) as Array<Record<string, unknown>>) : [];
                const rawEdges = edgesJson ? (JSON.parse(edgesJson) as Array<Record<string, unknown>>) : [];
                data = {
                  nodes: rawNodes.map(n => ({
                    id: n['id'],
                    title: n['title'],
                    type: n['type'],
                    tags: String(n['tags'] ?? '').split(',').filter(Boolean),
                    importance: Number(n['importance'] ?? 0.5),
                  })),
                  edges: rawEdges.map(e => ({
                    source: e['source'],
                    target: e['target'],
                    relation: e['relation'],
                    weight: Number(e['weight'] ?? 1.0),
                  })),
                };
              } catch { /* sqlite3 not available or query failed */ }
            }
          }

          res.end(JSON.stringify(data));
        } catch (e) {
          res.statusCode = 500;
          res.end(JSON.stringify({ error: String(e) }));
        }
      });
    },
  };
}

export default defineConfig({
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
  define: {
    __APP_VERSION__: JSON.stringify(rootPkg.version),
  },
  plugins: [svelte(), viteSingleFile(), harnessApiPlugin()],
  appType: 'spa',
  base: '/',
  resolve: {
    alias: {
      '$lib': path.resolve('./src/lib'),
    },
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 5174,
        }
      : undefined,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
});
