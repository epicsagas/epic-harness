// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import OrbitPipeline from './OrbitPipeline.svelte';
import { dismissOrbitPipeline } from '../lib/harness.js';
import { selectedProject } from '../lib/stores/project.js';
afterEach(() => { cleanup(); vi.unstubAllGlobals(); selectedProject.set('__all__'); });
const pipeline = { id: 'fixture-id', goal_slug: 'Keep completed fixture', phase: 'evolve', status: 'complete', mode: 'direct', branch: null, check_fail_count: 0, started_at: '2026-11-09T12:00:00Z', updated_at: '2026-11-09T12:01:00Z', deadline: null, phase_history: [] };
it.each([[false, { error: 'not deleted' }], [true, { ok: false }]])('rejects failed dismissal transport (%j)', async (ok, body) => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok, status: 500, json: async () => body }));
  await expect(dismissOrbitPipeline('fixture-id')).rejects.toThrow();
});
it('keeps a pipeline visible and reports the error after the server rejects dismissal', async () => {
  vi.stubGlobal('fetch', vi.fn((url: string) => Promise.resolve({ ok: !String(url).startsWith('/api/orbit/'), status: 500, json: async () => String(url).startsWith('/api/orbit/') ? { error: 'not deleted' } : [pipeline] })));
  const view = render(OrbitPipeline);
  expect(await screen.findByText('Keep completed fixture')).toBeTruthy();
  const button = view.container.querySelector('tbody button')!;
  await fireEvent.click(button);
  await waitFor(() => expect(view.container.textContent).toContain('not deleted'));
  expect(screen.getByText('Keep completed fixture')).toBeTruthy();
});

it.each([['fixture project', '?project=fixture%20project'], ['__all__', '']])('scopes browser dismissal to the selected project (%s)', async (project, query) => {
  selectedProject.set(project);
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => ({ ok: true }) }));
  await expect(dismissOrbitPipeline('same/id')).resolves.toEqual({ ok: true });
  expect(fetch).toHaveBeenCalledWith(`/api/orbit/same%2Fid${query}`, { method: 'DELETE' });
});