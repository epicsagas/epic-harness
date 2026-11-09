// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import Settings from './Settings.svelte';
import { selectedProject } from '../lib/stores/project.js';
const api = vi.hoisted(() => ({ metrics: vi.fn(), snapshots: vi.fn(), patterns: vi.fn() }));
vi.mock('../lib/harness.js', () => ({ getHarnessMetrics: api.metrics, getSessionSnapshots: api.snapshots, getGlobalPatterns: api.patterns }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
it('clears a previous project read error when the next project reload succeeds', async () => {
  api.metrics.mockRejectedValueOnce(new Error('Previous project unavailable')).mockResolvedValue({ total_sessions: 8, avg_success_rate: 1, total_evolved_skills: 0, score_history: [], session_count: 8, avg_score: 1 });
  api.snapshots.mockResolvedValue([]); api.patterns.mockResolvedValue([]);
  selectedProject.set('alpha'); const view = render(Settings);
  expect(await screen.findByText('Error: Previous project unavailable')).toBeTruthy();
  selectedProject.set('beta');
  await waitFor(() => expect(api.metrics).toHaveBeenCalledTimes(2));
  await waitFor(() => expect(screen.queryByText('Error: Previous project unavailable')).toBeNull());
  await waitFor(() => expect(view.container.querySelectorAll('.card').length).toBeGreaterThan(0));
});
