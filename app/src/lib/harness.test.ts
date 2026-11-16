// @vitest-environment jsdom
// The binary-served dashboard has no Tauri runtime, so every call goes through
// browserFallback. A backend failure must reject (pages render their error
// state), never resolve with invented numbers.

import { afterEach, describe, expect, it, vi } from 'vitest';
import { getEvolvedSkills, getHarnessMetrics, getObsSummary, getOrbitPipelines } from './harness.js';
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
