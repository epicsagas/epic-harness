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
// ── HarnessX evolution-engine surfaces ──
export const getSeesawRegistry = () => invoke<SolvedTaskRegistry>('get_seesaw_registry', projectArgs());
export const getVariantPool = () => invoke<VariantPool>('get_variant_pool', projectArgs());
export const getHarnessSnapshot = () => invoke<HarnessSnapshotData>('get_harness_snapshot', projectArgs());
export const getAdaptationLandscape = () => invoke<AdaptationLandscape>('get_adaptation_landscape', projectArgs());
export const getManifests = () => invoke<EditManifestEntry[]>('get_manifests', projectArgs());

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
  const query = new URLSearchParams({ cmd });
  const project = args?.project as string | undefined;
  if (project) query.set('project', project);
  const response = await fetch('/api/harness?' + query);
  if (!response.ok) throw new Error('Harness request failed (' + response.status + ')');
  const data: unknown = await response.json();
  if (data && typeof data === 'object' && 'error' in data) {
    throw new Error(String((data as Record<string, unknown>).error));
  }
  return data;
}
