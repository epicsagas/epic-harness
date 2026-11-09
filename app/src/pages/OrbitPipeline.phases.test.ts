// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, render, waitFor } from '@testing-library/svelte';
import OrbitPipeline from './OrbitPipeline.svelte';
const api = vi.hoisted(() => ({ list: vi.fn(), dismiss: vi.fn() }));
vi.mock('../lib/harness.js', () => ({ getOrbitPipelines: api.list, dismissOrbitPipeline: api.dismiss }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
it.each(['audit', 'eval', 'evolve'])('marks the persisted %s phase as active in the pipeline timeline', async phase => {
  api.list.mockResolvedValue([{ id: 'phase-fixture', goal_slug: 'Phase fixture', phase, status: 'running', mode: 'direct', branch: null, check_fail_count: 0, started_at: '2026-11-09T12:00:00Z', updated_at: '2026-11-09T12:01:00Z', deadline: null, phase_history: [] }]);
  const view = render(OrbitPipeline);
  await waitFor(() => expect(view.container.querySelector('.pipeline-stage.running .stage-label')?.textContent).toBe(phase));
});

it.each([['check', 'audit'], ['complete', 'evolve']])('retains legacy %s phase compatibility as %s', async (phase, label) => {
  api.list.mockResolvedValue([{ id: 'legacy-fixture', goal_slug: 'Legacy phase fixture', phase, status: 'running', mode: 'direct', branch: null, check_fail_count: 0, started_at: '2026-11-09T12:00:00Z', updated_at: '2026-11-09T12:01:00Z', deadline: null, phase_history: [{ phase: 'spec' }, { phase: 'go' }] }]);
  const view = render(OrbitPipeline);
  await waitFor(() => expect(view.container.querySelector('.pipeline-stage.running .stage-label')?.textContent).toBe(label));
  expect(view.container.querySelectorAll('.pipeline-stage.completed').length).toBe(2);
});