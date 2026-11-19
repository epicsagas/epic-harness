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

export function isDesktop(): boolean {
  return isTauriRuntime();
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
  epoch_class?: string | null;
  reward_hacking_suspected?: boolean;
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
  audit_fail_count: number;
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
  active_agents: ActiveAgent[]; // kept for backward compat; currently empty from backend
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

// ── HarnessX evolution-engine types ──
export interface SolvedTaskRegistry {
  solved: Record<string, number>;
  total_solved: number;
}

export interface SkillVariant {
  id: string;
  domain_tags: string[];
  skills: string[];
  avg_score: number;
  task_routing: string[];
}

export interface VariantPool {
  variants: SkillVariant[];
  routing_stats: Record<string, [number, number]>;
}

export interface HarnessSnapshotData {
  version: string;
  project_slug: string;
  timestamp: string;
  config_summary: {
    hook_profile: string;
    scoring_weights: number[];
    max_skills: number;
    stagnation_limit: number;
  };
  active_skills: string[];
  evolved_skills: string[];
  guard_rules: string[];
  metrics_summary: {
    total_sessions: number;
    best_score: number | null;
    trend: string;
    total_evolved: number;
    stagnation_count: number;
  };
  hash: string;
}

export interface PersistentFailure {
  failure_category: string;
  first_seen: string;
  sessions_seen: number;
  attempted_fixes: string[];
  resolved: boolean;
}

export interface AdaptationLandscape {
  persistent_failures: PersistentFailure[];
  attempted_edits: { edit_type: string; target: string; timestamp: string; success: boolean }[];
  edit_type_coverage: Record<string, number>;
  untried_edit_types: string[];
  component_failure_heatmap: Record<string, number>;
}

export interface EditManifestEntry {
  edit_type: string;
  target: string;
  intended_effect: string;
  predicted_impact: string;
}

export interface GraphStats {
  nodes: number;
  edges: number;
  avg_importance: number;
  types: Record<string, number>;
}

export interface MemoryNode {
  id: string;
  type: string;
  title: string;
  tags: string[];
  projects: string[];
  updated: string;
  // detail fields
  agents?: string;
  created?: string;
  importance?: number;
  access_count?: number;
  body?: string;
}

// ── API calls ─────────────────────────────────────────────────────────────────

export async function getHarnessMetrics(project?: string): Promise<HarnessMetrics> {
  const raw = await invoke<HarnessMetrics>('get_harness_metrics', projectArgs(project));
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
export async function getOrbitPipelines(): Promise<OrbitPipeline[]> {
  const rows = await invoke<Array<OrbitPipeline & { check_fail_count?: number }>>('get_orbit_pipelines', projectArgs());
  return rows.map(p => ({ ...p, audit_fail_count: p.audit_fail_count ?? p.check_fail_count ?? 0 }));
}
export async function dismissOrbitPipeline(id: string): Promise<{ ok: boolean }> {
  const res = await fetch(`/api/orbit/${encodeURIComponent(id)}`, { method: 'DELETE' });
  let body: { ok?: boolean } | null = null;
  try { body = await res.json(); } catch { body = null; }
  if (!res.ok || body?.ok !== true) throw new Error(`dismiss failed (${res.status})`);
  return { ok: true };
}
export const getEvolvedSkills = (project?: string) => invoke<EvolutionData>('get_evolved_skills', projectArgs(project));
export const getObsSummary = () => invoke<ObsSummary>('get_obs_summary', projectArgs());
export const getGraph = () => invoke<GraphData>('get_graph');
export const getIntegrationStatus = () => invoke<IntegrationStatus[]>('get_integration_status');
export const getSessionSnapshots = () => invoke<SessionSnapshotData[]>('get_session_snapshots', projectArgs());
export const getGlobalPatterns = () => invoke<GlobalPattern[]>('get_global_patterns', projectArgs());
// ── HarnessX evolution-engine surfaces ──
export const getSeesawRegistry = (project?: string) => invoke<SolvedTaskRegistry>('get_seesaw_registry', projectArgs(project));
export const getVariantPool = (project?: string) => invoke<VariantPool>('get_variant_pool', projectArgs(project));
export const getHarnessSnapshot = () => invoke<HarnessSnapshotData>('get_harness_snapshot', projectArgs());
export const getAdaptationLandscape = (project?: string) => invoke<AdaptationLandscape>('get_adaptation_landscape', projectArgs(project));
export const getManifests = () => invoke<EditManifestEntry[]>('get_manifests', projectArgs());

// ── Orchestration API ──────────────────────────────────────────────────────

// Desktop reads go over read-only native commands; the browser build calls the
// served routes with the same project scope.
async function readOrchestrator(command: string, path: string, project: string | undefined, extra?: Record<string, unknown>): Promise<Record<string, unknown> | null> {
  const args = projectArgs(project);
  if (isTauriRuntime()) {
    return invoke<Record<string, unknown> | null>(command, extra ? { ...args, ...extra } : args);
  }
  const res = await fetch(args ? `${path}?project=${encodeURIComponent(args.project as string)}` : path);
  return res.ok ? res.json() : null;
}

export async function getOrchestratorRun(project: string | undefined = get(selectedProject)): Promise<OrchestrationRun | null> {
  try {
    const data = await readOrchestrator('get_orchestrator_run', '/api/run', project);
    return data && Object.keys(data).length > 0 ? data as unknown as OrchestrationRun : null;
  } catch {
    return null;
  }
}

export async function getOrchestratorAgentStatus(agentId: string, project: string | undefined = get(selectedProject)): Promise<OrchAgentStatus | null> {
  try {
    const data = await readOrchestrator('get_orchestrator_agent_status', `/api/agents/${encodeURIComponent(agentId)}/status`, project, { agentId });
    return data && Object.keys(data).length > 0 ? data as unknown as OrchAgentStatus : null;
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
    const data = isTauriRuntime()
      ? await invoke<GraphStats>('get_stats')
      : await fetchJson<GraphStats>('/api/stats');
    return data && Object.keys(data).length > 0 ? data : null;
  } catch {
    return null;
  }
}

export async function getMemoryNodes(): Promise<MemoryNode[]> {
  try {
    return isTauriRuntime()
      ? await invoke<MemoryNode[]>('get_nodes')
      : await fetchJson<MemoryNode[]>('/api/nodes');
  } catch {
    return [];
  }
}

export async function getMemoryNode(id: string): Promise<MemoryNode | null> {
  try {
    return isTauriRuntime()
      ? await invoke<MemoryNode>('get_node', { id })
      : await fetchJson<MemoryNode>(`/api/nodes/${encodeURIComponent(id)}`);
  } catch {
    return null;
  }
}

export async function searchMemory(query: string): Promise<MemoryNode[]> {
  try {
    const rows = isTauriRuntime()
      ? await invoke<MemoryNode[]>('search_nodes', { query })
      : await fetchJson<MemoryNode[]>(`/api/search?q=${encodeURIComponent(query)}`);
    return rows.map(r => ({ ...r, projects: r.projects ?? [] }));
  } catch {
    return [];
  }
}

async function fetchJson<T>(url: string): Promise<T> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url} failed: HTTP ${res.status}`);
  return res.json();
}

// ── Dev fallback: real data via Vite /api/harness middleware ─────────────────

/** Build project args from the store. `__all__` means aggregate → omit from request. */
function projectArgs(p: string | undefined = get(selectedProject)): Record<string, unknown> | undefined {
  return p && p !== '__all__' ? { project: p } : undefined;
}

async function browserFallback(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  // Served by the epic-harness binary (/api/harness) or the Vite dev middleware.
  // Failures reject so pages show their error state instead of invented data.
  const project = args?.project as string | undefined;
  let url = `/api/harness?cmd=${encodeURIComponent(cmd)}`;
  if (project) url += `&project=${encodeURIComponent(project)}`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${cmd} failed: HTTP ${res.status}`);
  const data: unknown = await res.json();
  const err = (data as Record<string, unknown> | null)?.error;
  if (err) throw new Error(`${cmd} failed: ${String(err)}`);
  return data;
}
