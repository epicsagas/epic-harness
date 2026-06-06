// Tauri invoke bridge with browser fallback for non-Tauri environments
import { get } from 'svelte/store';
import { selectedProject } from '$lib/stores/project.js';

type InvokeFn = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

let _invoke: InvokeFn | null = null;
let _invokePromise: Promise<void> | null = null;

function isTauriRuntime(): boolean {
  // __TAURI_INTERNALS__ is injected by the Tauri runtime into the webview window.
  // Plain browsers and Vite dev server never have it.
  return typeof window !== 'undefined' &&
    !!((window as unknown) as Record<string, unknown>)['__TAURI_INTERNALS__'];
}

async function resolveInvoke(): Promise<void> {
  if (!isTauriRuntime()) {
    _invoke = browserFallback;
    return;
  }
  try {
    const mod = await import('@tauri-apps/api/core');
    _invoke = (cmd, args) => mod.invoke(cmd, args);
  } catch {
    _invoke = browserFallback;
  }
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!_invoke) {
    if (!_invokePromise) _invokePromise = resolveInvoke();
    await _invokePromise;
  }
  return _invoke!(cmd, args) as Promise<T>;
}

// ── Types ─────────────────────────────────────────────────────────────────────

// Real metrics.json schema from epic-harness reflect hook
export interface ScoreHistoryEntry {
  timestamp: string;
  avg_score: number;
  success_rate: number;
  observations: number;
  dimension_averages?: Record<string, number>;
}

export interface SkillAttribution {
  skill_name: string;
  sessions_active: number;
  avg_score_with: number;
  avg_score_without: number;
  first_seen: string;
}

export interface HarnessMetrics {
  total_sessions: number;
  avg_success_rate: number;
  total_evolved_skills: number;
  last_session: string | null;
  best_score?: number;
  best_session?: string | null;
  last_error_context?: string | null;
  score_history: ScoreHistoryEntry[];
  stagnation_count?: number;
  trend?: string;
  skill_attribution?: Record<string, SkillAttribution>;
  // derived by getHarnessMetrics()
  session_count: number;
  avg_score: number;
}

export interface OrbitPipeline {
  id: string;
  mode: string | null;
  phase: string;
  status: 'running' | 'complete' | 'failed' | 'paused' | 'aborted' | 'timeout';
  goal_slug: string | null;
  branch: string | null;
  check_fail_count: number;
  started_at: string;
  updated_at: string;
  deadline: string | null;
  phase_history: Array<{ phase: string; status: string; completed_at: string }>;
  _project?: string;
}

export interface EvolvedSkill {
  name: string;
  origin: string;
  confidence: number;
  project: string;
  active: boolean;
  skill_md: string;
  created_at: string | null;
  updated_at: string;
}

export interface EvolutionData {
  evolved_skills: EvolvedSkill[];
  evolution_history: Record<string, unknown>[];
  total_sessions_analyzed: number;
  patterns_detected: number;
}

export interface SessionSummary {
  session_id: string;
  date: string;
  tool_calls: number;
  avg_score: number;
  failures: number;
}

export interface ToolStat {
  tool: string;
  calls: number;
  success_rate: number;
  avg_score: number;
}

export interface ActiveAgent {
  name: string;
  last_tool: string;
  last_action: string;
  score: number;
  timestamp: string;
}

export interface FailureCategory {
  category: string;
  count: number;
}

export interface ObsSummary {
  recent_sessions: SessionSummary[];
  tool_stats: ToolStat[];
  total_tool_calls: number;
  avg_score: number;
  failure_categories: FailureCategory[];
  active_agents: ActiveAgent[];
}

export interface OrchAgentDef {
  id: string;
  role: string;
  task: string;
  satisfies: string[];
  status: string;
  started_at: string | null;
  completed_at: string | null;
}

export interface OrchestrationRun {
  id: string;
  status: string;
  agents: OrchAgentDef[];
  dependency_graph: Record<string, string[]>;
  created_at: string;
  updated_at: string;
}

export interface OrchAgentStatus {
  agent_id: string;
  phase: string;
  progress: number;
  last_heartbeat: string;
  status: string;
}

export interface AgentEvent {
  timestamp: string;
  event_type: string;
  data: Record<string, unknown>;
}

export interface GraphNode {
  id: string;
  title: string;
  type: string;
  tags: string[];
  importance: number;
  projects: string[];
  accessed_at: string;
}

export interface GraphEdge {
  source: string;
  target: string;
  relation: string;
  weight: number;
}

export interface GraphData {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export interface IntegrationStatus {
  name: string;
  installed: boolean;
  config_path: string | null;
  version: string | null;
}

export interface InboxMessage {
  to: string;
  from: string;
  timestamp: string;
  message: string;
}

export interface SessionSnapshotData {
  timestamp: string;
  type: string;
  summary: string;
  pending_tasks: string[];
  context_usage: number | null;
  pipeline_state: Record<string, unknown> | null;
}

export interface GlobalPattern {
  timestamp: string;
  project: string;
  success_rate: number;
  avg_score: number;
  per_error_stats: Record<string, unknown>;
  failure_patterns: string[];
  weak_tools: string[];
}

export interface GraphStats {
  total_nodes: number;
  total_edges: number;
  avg_importance: number;
  by_type: Record<string, number>;
}

export interface MemoryNode {
  id: string;
  type: string;
  title: string;
  tags: string[];
  projects: string;
  updated: string;
  // detail fields
  agents?: string;
  created?: string;
  importance?: number;
  access_count?: number;
  body?: string;
}

// ── API calls ─────────────────────────────────────────────────────────────────

export async function getHarnessMetrics(): Promise<HarnessMetrics> {
  const raw = await invoke<HarnessMetrics>('get_harness_metrics', projectArgs());
  if (!raw) throw new Error('metrics.json not found');

  // Derive avg_score from score_history
  const history = raw.score_history ?? [];
  const avgScore = history.length > 0
    ? history.reduce((s, e) => s + (e.avg_score ?? 0), 0) / history.length
    : 0;

  // Derive trend from last 3 entries
  let trend: string = raw.trend ?? 'stable';
  if (!raw.trend && history.length >= 2) {
    const last = history[history.length - 1].avg_score;
    const prev = history[history.length - 2].avg_score;
    const delta = last - prev;
    trend = delta > 0.01 ? 'improving' : delta < -0.01 ? 'declining' : 'stable';
  }

  return {
    ...raw,
    session_count: raw.total_sessions ?? 0,
    avg_score: Math.round(avgScore * 1000) / 1000,
    stagnation_count: raw.stagnation_count ?? 0,
    trend,
  };
}
export const getOrbitPipelines = () => invoke<OrbitPipeline[]>('get_orbit_pipelines');
export async function dismissOrbitPipeline(id: string): Promise<{ ok: boolean }> {
  const res = await fetch(`/api/orbit/${encodeURIComponent(id)}`, { method: 'DELETE' });
  return res.json();
}
export const getEvolvedSkills = () => invoke<EvolutionData>('get_evolved_skills', projectArgs());
export const getObsSummary = () => invoke<ObsSummary>('get_obs_summary', projectArgs());
export const getGraph = () => invoke<GraphData>('get_graph');
export const getIntegrationStatus = () => invoke<IntegrationStatus[]>('get_integration_status');
export const getSessionSnapshots = () => invoke<SessionSnapshotData[]>('get_session_snapshots', projectArgs());
export const getGlobalPatterns = () => invoke<GlobalPattern[]>('get_global_patterns', projectArgs());

// ── Orchestration API ──────────────────────────────────────────────────────

export async function getOrchestratorRun(): Promise<OrchestrationRun | null> {
  try {
    const res = await fetch('/api/run');
    if (!res.ok) return null;
    const data = await res.json();
    return data && Object.keys(data).length > 0 ? data as OrchestrationRun : null;
  } catch {
    return null;
  }
}

export async function getOrchestratorAgentStatus(agentId: string): Promise<OrchAgentStatus | null> {
  try {
    const res = await fetch(`/api/agents/${encodeURIComponent(agentId)}/status`);
    if (!res.ok) return null;
    const data = await res.json();
    return data && Object.keys(data).length > 0 ? data as OrchAgentStatus : null;
  } catch {
    return null;
  }
}

export async function dismissAgent(agentId: string): Promise<{ ok: boolean }> {
  const res = await fetch(`/api/agents/${encodeURIComponent(agentId)}`, { method: 'DELETE' });
  return res.json();
}

export async function getAgentEvents(agentId: string): Promise<AgentEvent[]> {
  try {
    const res = await fetch(`/api/agents/${encodeURIComponent(agentId)}/events`);
    if (!res.ok) return [];
    return await res.json();
  } catch {
    return [];
  }
}

export async function getAgentInbox(agentId: string): Promise<InboxMessage[]> {
  try {
    const res = await fetch(`/api/agents/${encodeURIComponent(agentId)}/inbox`);
    if (!res.ok) return [];
    return await res.json();
  } catch {
    return [];
  }
}

export async function getGraphStats(): Promise<GraphStats | null> {
  try {
    const res = await fetch('/api/stats');
    if (!res.ok) return null;
    const data = await res.json();
    return data && Object.keys(data).length > 0 ? data as GraphStats : null;
  } catch {
    return null;
  }
}

export async function getMemoryNodes(): Promise<MemoryNode[]> {
  try {
    const res = await fetch('/api/nodes');
    if (!res.ok) return [];
    return await res.json();
  } catch {
    return [];
  }
}

export async function getMemoryNode(id: string): Promise<MemoryNode | null> {
  try {
    const res = await fetch(`/api/nodes/${encodeURIComponent(id)}`);
    if (!res.ok) return null;
    return await res.json();
  } catch {
    return null;
  }
}

export async function searchMemory(query: string, type?: string): Promise<MemoryNode[]> {
  try {
    let url = `/api/search?q=${encodeURIComponent(query)}`;
    if (type) url += `&type=${encodeURIComponent(type)}`;
    const res = await fetch(url);
    if (!res.ok) return [];
    return await res.json();
  } catch {
    return [];
  }
}

// ── Dev fallback: real data via Vite /api/harness middleware ─────────────────

/** Build project args from the store. `__all__` means aggregate → omit from request. */
function projectArgs(): Record<string, unknown> | undefined {
  const p = get(selectedProject);
  return p && p !== '__all__' ? { project: p } : undefined;
}

async function browserFallback(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  // In Vite dev, the harnessApiPlugin middleware serves real harness data at /api/harness.
  // Attempt it first; fall through to static mock only on network error or missing data.
  // The middleware is configured in vite.config.ts and only applies during `vite dev` (apply: 'serve').
  if (typeof window !== 'undefined') {
    try {
      const project = args?.project as string | undefined;
      let url = `/api/harness?cmd=${encodeURIComponent(cmd)}`;
      if (project) url += `&project=${encodeURIComponent(project)}`;
      const res = await fetch(url);
      if (res.ok) {
        const data: unknown = await res.json();
        if (data !== null && data !== undefined && !(data as Record<string, unknown>).error) return data;
      }
    } catch { /* fall through to static mock */ }
  }
  await new Promise(r => setTimeout(r, 200));
  switch (cmd) {
    case 'get_harness_metrics':
      return {
        total_sessions: 42,
        avg_success_rate: 0.91,
        total_evolved_skills: 2,
        last_session: new Date().toISOString(),
        score_history: [
          { timestamp: '2026-05-12T04:00:00Z', avg_score: 0.72, success_rate: 0.9, observations: 11 },
          { timestamp: '2026-05-13T04:00:00Z', avg_score: 0.75, success_rate: 0.92, observations: 14 },
          { timestamp: '2026-05-14T04:00:00Z', avg_score: 0.82, success_rate: 0.95, observations: 18 },
        ],
        stagnation_count: 0,
        trend: 'improving',
        session_count: 42,
        avg_score: 0.763,
      } satisfies HarnessMetrics;

    case 'get_orbit_pipelines':
      return [
        {
          id: '20260518192211',
          mode: 'direct',
          phase: 'go',
          status: 'running',
          goal_slug: 'dashboard-live-metrics',
          branch: 'worktree-orbit-dashboard-live-metrics',
          check_fail_count: 0,
          started_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
          deadline: new Date(Date.now() + 30 * 60000).toISOString(),
          phase_history: [{ phase: 'spec', status: 'complete', completed_at: new Date().toISOString() }],
        },
        {
          id: '20260517110000',
          mode: 'council',
          phase: 'complete',
          status: 'complete',
          goal_slug: 'epic-harness-dashboard',
          branch: 'feat/epic-harness-dashboard',
          check_fail_count: 0,
          started_at: new Date(Date.now() - 86400000).toISOString(),
          updated_at: new Date(Date.now() - 82800000).toISOString(),
          deadline: null,
          phase_history: [
            { phase: 'spec', status: 'complete', completed_at: '' },
            { phase: 'go', status: 'complete', completed_at: '' },
            { phase: 'check', status: 'pass', completed_at: '' },
            { phase: 'ship', status: 'complete', completed_at: '' },
          ],
        },
      ] satisfies OrbitPipeline[];

    case 'get_evolved_skills':
      return {
        evolved_skills: [
          { name: 'pattern-fix-then-break', skill_md: '# Fix-Then-Break Recovery\nDetected alternating edit/error cycle. Pause and re-read the file before editing.', created_at: null },
          { name: 'tool-bash-weak', skill_md: '# Bash Tool Guidance\nBash success rate below threshold. Prefer Read/Edit for file operations.', created_at: null },
        ],
        evolution_history: [
          { timestamp: new Date().toISOString(), patterns: ['fix_then_break'], skills_seeded: 1, trend: 'improving', avg_score: 0.82 },
        ],
        total_sessions_analyzed: 42,
        patterns_detected: 7,
      } satisfies EvolutionData;

    case 'get_obs_summary':
      return {
        recent_sessions: [
          { session_id: '20260518_67321', date: '20260518', tool_calls: 38, avg_score: 0.91, failures: 2 },
          { session_id: '20260518_67316', date: '20260518', tool_calls: 22, avg_score: 0.88, failures: 1 },
          { session_id: '20260517_66100', date: '20260517', tool_calls: 55, avg_score: 0.84, failures: 4 },
        ],
        tool_stats: [
          { tool: 'Read', calls: 45, success_rate: 1.0, avg_score: 0.95 },
          { tool: 'Edit', calls: 38, success_rate: 0.92, avg_score: 0.89 },
          { tool: 'Bash', calls: 30, success_rate: 0.87, avg_score: 0.81 },
          { tool: 'Write', calls: 12, success_rate: 1.0, avg_score: 0.95 },
          { tool: 'Agent', calls: 8, success_rate: 0.75, avg_score: 0.78 },
        ],
        total_tool_calls: 133,
        avg_score: 0.891,
        active_agents: [],
      } satisfies ObsSummary;

    case 'get_graph':
      return {
        nodes: [
          { id: '1', title: 'epic-harness', type: 'project', tags: ['harness'], importance: 0.9 },
          { id: '2', title: 'harness-mem', type: 'concept', tags: ['memory', 'sqlite'], importance: 0.8 },
          { id: '3', title: 'orbit pipeline', type: 'pattern', tags: ['automation'], importance: 0.85 },
          { id: '4', title: '4-Ring Architecture', type: 'concept', tags: ['architecture'], importance: 0.9 },
          { id: '5', title: 'eval system', type: 'concept', tags: ['scoring'], importance: 0.75 },
          { id: '6', title: 'Ring 0 Hooks', type: 'concept', tags: ['autopilot'], importance: 0.7 },
          { id: '7', title: '_dispatch', type: 'pattern', tags: ['skills'], importance: 0.8 },
        ],
        edges: [
          { source: '1', target: '2', relation: 'contains', weight: 0.9 },
          { source: '1', target: '3', relation: 'implements', weight: 0.85 },
          { source: '1', target: '4', relation: 'follows', weight: 0.9 },
          { source: '4', target: '6', relation: 'contains', weight: 0.8 },
          { source: '1', target: '5', relation: 'uses', weight: 0.75 },
          { source: '3', target: '5', relation: 'feeds', weight: 0.7 },
          { source: '1', target: '7', relation: 'uses', weight: 0.8 },
        ],
      } satisfies GraphData;

    case 'get_integration_status':
      return [
        { name: 'Claude Code', installed: true, config_path: '~/.claude/settings.json', version: null },
        { name: 'Codex', installed: false, config_path: null, version: null },
        { name: 'Gemini CLI', installed: false, config_path: null, version: null },
        { name: 'Cursor', installed: false, config_path: null, version: null },
        { name: 'Cline', installed: false, config_path: null, version: null },
        { name: 'Aider', installed: false, config_path: null, version: null },
      ] satisfies IntegrationStatus[];

    default:
      return null;
  }
}
