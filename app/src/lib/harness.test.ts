// @vitest-environment jsdom
// The binary-served dashboard has no Tauri runtime, so every call goes through
// browserFallback. A backend failure must reject (pages render their error
// state), never resolve with invented numbers.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { dismissOrbitPipeline, getEvolvedSkills, getGraphStats, getHarnessMetrics, getObsSummary, getOrbitPipelines, getOrchestratorAgentStatus, getOrchestratorRun, isDesktop, searchMemory } from './harness.js';
import { selectedProject } from './stores/project.js';

function stubFetch(impl: () => Promise<Partial<Response>>) {
  const fn = vi.fn(impl);
  vi.stubGlobal('fetch', fn);
  return fn;
}

const json = (body: unknown, ok = true, status = 200) =>
  Promise.resolve({ ok, status, json: () => Promise.resolve(body) });

afterEach(() => vi.unstubAllGlobals());

describe('browserFallback error handling', () => {
  it('rejects on non-OK HTTP instead of returning mock data', async () => {
    stubFetch(() => json({ error: 'boom' }, false, 500));
    await expect(getHarnessMetrics()).rejects.toThrow(/500/);
    await expect(getObsSummary()).rejects.toThrow(/500/);
    await expect(getOrbitPipelines()).rejects.toThrow(/500/);
  });

  it('rejects on network failure', async () => {
    stubFetch(() => Promise.reject(new TypeError('Failed to fetch')));
    await expect(getEvolvedSkills()).rejects.toThrow('Failed to fetch');
  });

  it('rejects when the body carries an error', async () => {
    stubFetch(() => json({ error: 'db unavailable' }));
    await expect(getObsSummary()).rejects.toThrow('db unavailable');
  });

  it('keeps genuine empty results empty', async () => {
    stubFetch(() => json([]));
    await expect(getOrbitPipelines()).resolves.toEqual([]);
  });

  it('passes real data through untouched', async () => {
    const fetchMock = stubFetch(() =>
      json({ total_sessions: 3, score_history: [{ avg_score: 0.5 }], trend: 'stable' }),
    );
    const m = await getHarnessMetrics();
    expect(m.total_sessions).toBe(3);
    expect(m.avg_score).toBe(0.5);
    expect(fetchMock).toHaveBeenCalledWith('/api/harness?cmd=get_harness_metrics');
  });
});

describe('project selection', () => {
  afterEach(() => selectedProject.set('__all__'));

  it('an explicit project wins over the store, so a load keeps the project it started with', async () => {
    selectedProject.set('proj-new');
    const fetchMock = stubFetch(() => json({ evolved_skills: [] }));
    await getEvolvedSkills('proj-old');
    expect(fetchMock).toHaveBeenCalledWith('/api/harness?cmd=get_evolved_skills&project=proj-old');
  });

  it('orbit pipelines follow the selected project', async () => {
    selectedProject.set('proj-a');
    const fetchMock = stubFetch(() => json([]));
    await getOrbitPipelines();
    expect(fetchMock).toHaveBeenCalledWith('/api/harness?cmd=get_orbit_pipelines&project=proj-a');
  });
});

describe('dismissOrbitPipeline', () => {
  it('resolves on 200 {ok:true} and sends no query string', async () => {
    const fetchMock = stubFetch(() => json({ ok: true }));
    await expect(dismissOrbitPipeline('pid-1')).resolves.toEqual({ ok: true });
    expect(fetchMock).toHaveBeenCalledWith('/api/orbit/pid-1', { method: 'DELETE' });
  });

  it('throws on 404 {ok:false}', async () => {
    stubFetch(() => json({ ok: false }, false, 404));
    await expect(dismissOrbitPipeline('pid-1')).rejects.toThrow('dismiss failed (404)');
  });

  it('throws on 400', async () => {
    stubFetch(() => json({ error: 'bad id' }, false, 400));
    await expect(dismissOrbitPipeline('pid-1')).rejects.toThrow('dismiss failed (400)');
  });

  it('throws on 404 with a text body', async () => {
    stubFetch(() => Promise.resolve({ ok: false, status: 404, json: () => Promise.reject(new SyntaxError('not json')) }));
    await expect(dismissOrbitPipeline('pid-1')).rejects.toThrow('dismiss failed (404)');
  });
});

describe('memory reads over fetch', () => {
  it('searchMemory fetches /api/search?q=x', async () => {
    const fetchMock = stubFetch(() => json([]));
    await searchMemory('x');
    expect(fetchMock).toHaveBeenCalledWith('/api/search?q=x');
  });

  it('getGraphStats returns the stats object', async () => {
    stubFetch(() => json({ nodes: 3, edges: 2, avg_importance: 0.5, types: { concept: 3 } }));
    await expect(getGraphStats()).resolves.toEqual({ nodes: 3, edges: 2, avg_importance: 0.5, types: { concept: 3 } });
  });

  it('getGraphStats returns null for the empty-object fallback', async () => {
    stubFetch(() => json({}));
    await expect(getGraphStats()).resolves.toBeNull();
  });
});

describe('getOrbitPipelines fail count mapping', () => {
  it('maps audit_fail_count and the legacy check_fail_count', async () => {
    stubFetch(() => json([{ id: 'a', audit_fail_count: 2 }, { id: 'b', check_fail_count: 1 }, { id: 'c' }]));
    const rows = await getOrbitPipelines();
    expect(rows.map(r => r.audit_fail_count)).toEqual([2, 1, 0]);
  });
});

describe('Tauri runtime memory transport', () => {
  const invokeMock = vi.fn();

  async function load() {
    vi.resetModules();
    vi.doMock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    return import('./harness.js');
  }

  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    vi.doUnmock('@tauri-apps/api/core');
    invokeMock.mockReset();
  });

  it('calls the four native commands with their args', async () => {
    const h = await load();
    invokeMock.mockResolvedValueOnce({ nodes: 1, edges: 0, avg_importance: 0.1, types: {} });
    await expect(h.getGraphStats()).resolves.toMatchObject({ nodes: 1 });
    expect(invokeMock).toHaveBeenLastCalledWith('get_stats', undefined);

    invokeMock.mockResolvedValueOnce([]);
    await h.getMemoryNodes();
    expect(invokeMock).toHaveBeenLastCalledWith('get_nodes', undefined);

    invokeMock.mockResolvedValueOnce({ id: 'n1' });
    await h.getMemoryNode('n1');
    expect(invokeMock).toHaveBeenLastCalledWith('get_node', { id: 'n1' });

    invokeMock.mockResolvedValueOnce([{ id: 'n1', title: 't' }]);
    const rows = await h.searchMemory('q');
    expect(invokeMock).toHaveBeenLastCalledWith('search_nodes', { query: 'q' });
    expect(rows[0].projects).toEqual([]);
  });

  it('getMemoryNode returns null when invoke rejects', async () => {
    const h = await load();
    invokeMock.mockRejectedValueOnce(new Error('invalid uuid'));
    await expect(h.getMemoryNode('not-a-uuid')).resolves.toBeNull();
  });

  it('reads the run and agent status over read-only native commands', async () => {
    const h = await load();
    expect(h.isDesktop()).toBe(true);

    invokeMock.mockResolvedValueOnce({ id: 'run-1', agents: [] });
    await expect(h.getOrchestratorRun('proj-a')).resolves.toMatchObject({ id: 'run-1' });
    expect(invokeMock).toHaveBeenLastCalledWith('get_orchestrator_run', { project: 'proj-a' });

    invokeMock.mockResolvedValueOnce(null);
    await expect(h.getOrchestratorRun('__all__')).resolves.toBeNull();
    expect(invokeMock).toHaveBeenLastCalledWith('get_orchestrator_run', undefined);

    invokeMock.mockResolvedValueOnce({ agent_id: 'ag-1', status: 'running' });
    await h.getOrchestratorAgentStatus('ag-1', 'proj-a');
    expect(invokeMock).toHaveBeenLastCalledWith('get_orchestrator_agent_status', { project: 'proj-a', agentId: 'ag-1' });

    invokeMock.mockResolvedValueOnce({});
    await expect(h.getOrchestratorAgentStatus('ag-1', '__all__')).resolves.toBeNull();
    expect(invokeMock).toHaveBeenLastCalledWith('get_orchestrator_agent_status', { agentId: 'ag-1' });
  });
});

describe('orchestrator reads over fetch', () => {
  afterEach(() => selectedProject.set('__all__'));

  it('is not the desktop runtime and builds an encoded project query', async () => {
    expect(isDesktop()).toBe(false);
    const fetchMock = stubFetch(() => json({ id: 'run-1' }));
    await expect(getOrchestratorRun('proj a/1')).resolves.toMatchObject({ id: 'run-1' });
    expect(fetchMock).toHaveBeenLastCalledWith('/api/run?project=proj%20a%2F1');

    await getOrchestratorAgentStatus('ag-1', 'proj-a');
    expect(fetchMock).toHaveBeenLastCalledWith('/api/agents/ag-1/status?project=proj-a');
  });

  it('omits the query for all projects and defaults to the selected project', async () => {
    const fetchMock = stubFetch(() => json({}));
    await getOrchestratorRun('__all__');
    expect(fetchMock).toHaveBeenLastCalledWith('/api/run');

    selectedProject.set('proj-b');
    await getOrchestratorRun();
    expect(fetchMock).toHaveBeenLastCalledWith('/api/run?project=proj-b');
  });

  it('maps an empty object and a failed response to null', async () => {
    stubFetch(() => json({}));
    await expect(getOrchestratorRun('proj-a')).resolves.toBeNull();
    await expect(getOrchestratorAgentStatus('ag-1', 'proj-a')).resolves.toBeNull();
    stubFetch(() => json({ error: 'invalid project' }, false, 400));
    await expect(getOrchestratorRun('proj-a')).resolves.toBeNull();
  });
});
