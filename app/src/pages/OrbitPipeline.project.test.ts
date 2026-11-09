// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import { selectedProject } from '../lib/stores/project.js';
import OrbitPipeline from './OrbitPipeline.svelte';
const api = vi.hoisted(() => ({ list: vi.fn(), dismiss: vi.fn() }));
vi.mock('../lib/harness.js', () => ({ getOrbitPipelines: api.list, dismissOrbitPipeline: api.dismiss }));
afterEach(() => { cleanup(); vi.clearAllMocks(); selectedProject.set('__all__'); });
const pipeline = (name: string) => ({ id: name, goal_slug: name, phase: 'evolve', status: 'complete', mode: 'direct', branch: null, check_fail_count: 0, started_at: '2026-11-09T12:00:00Z', updated_at: '2026-11-09T12:01:00Z', deadline: null, phase_history: [] });
it('reloads pipelines for a project switch and ignores a late previous-project response', async () => {
  let release!: (value: unknown) => void;
  api.list.mockImplementationOnce(() => new Promise(resolve => { release = resolve; })).mockResolvedValue([pipeline('beta pipeline')]);
  selectedProject.set('alpha');
  render(OrbitPipeline);
  await waitFor(() => expect(release).toBeDefined());
  selectedProject.set('beta');
  expect(await screen.findByText('beta pipeline')).toBeTruthy();
  release([pipeline('alpha pipeline')]);
  await tick(); await tick();
  expect(screen.queryByText('alpha pipeline')).toBeNull();
  expect(screen.getByText('beta pipeline')).toBeTruthy();
  expect(api.list).toHaveBeenCalledWith('alpha');
  expect(api.list).toHaveBeenCalledWith('beta');
});
