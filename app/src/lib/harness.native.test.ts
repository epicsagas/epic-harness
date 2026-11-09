// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { getGraphStats, getMemoryNodes, getMemoryNode, searchMemory, getOrchestratorRun, getOrchestratorAgentStatus, getAgentEvents, getAgentInbox, dismissAgent, dismissOrbitPipeline } from './harness.js';
import { selectedProject } from './stores/project.js';
const native = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: native.invoke }));
beforeEach(() => {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
  selectedProject.set('fixture-project');
  vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('No HTTP server in native runtime')));
  native.invoke.mockImplementation(async (command: string) => ({ get_stats: { nodes: 1, edges: 0, types: { concept: 1 }, avg_importance: .7 }, get_nodes: [{ id: 'one', type: 'concept', title: 'fixture', tags: 'one,two', projects: 'fixture-project', updated: '' }], get_node: { id: 'one', tags: 'one,two', projects: 'fixture-project', body: 'detail' }, search_nodes: [{ id: 'one', title: 'found', type: 'concept', tags: [], projects: [] }], get_orchestrator_run: { id: 'run' }, get_orchestrator_agent_status: { agent_id: 'agent' }, get_agent_events: [{ event_type: 'started' }], get_agent_inbox: [{ message: 'fixture' }], dismiss_agent: { ok: true }, dismiss_orbit_pipeline: { ok: true } }[command]));
});
afterEach(() => { delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; vi.unstubAllGlobals(); vi.clearAllMocks(); });
it('routes native memory reads through IPC and normalizes native list metadata', async () => {
  expect((await getGraphStats())?.total_nodes).toBe(1);
  expect((await getMemoryNodes())[0].tags).toEqual(['one', 'two']);
  expect((await getMemoryNode('one'))?.body).toBe('detail');
  expect((await searchMemory('found', 'concept'))[0].title).toBe('found');
  expect(native.invoke).toHaveBeenCalledWith('get_node', { id: 'one' });
  expect(native.invoke).toHaveBeenCalledWith('search_nodes', { query: 'found', nodeType: 'concept' });
  expect(fetch).not.toHaveBeenCalled();
});
it('routes native orchestration reads and mutations through scoped IPC commands', async () => {
  expect((await getOrchestratorRun())?.id).toBe('run');
  expect((await getOrchestratorAgentStatus('agent'))?.agent_id).toBe('agent');
  expect(await getAgentEvents('agent')).toHaveLength(1);
  expect(await getAgentInbox('agent')).toHaveLength(1);
  expect(await dismissAgent('agent')).toEqual({ ok: true });
  expect(await dismissOrbitPipeline('pipeline')).toEqual({ ok: true });
  expect(native.invoke).toHaveBeenCalledWith('get_orchestrator_agent_status', { agentId: 'agent', project: 'fixture-project' });
  expect(native.invoke).toHaveBeenCalledWith('dismiss_orbit_pipeline', { id: 'pipeline', project: 'fixture-project' });
  expect(fetch).not.toHaveBeenCalled();
});
