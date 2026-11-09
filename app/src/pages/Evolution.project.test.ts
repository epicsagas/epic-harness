// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import { selectedProject } from '../lib/stores/project.js';
import Evolution from './Evolution.svelte';
const api = vi.hoisted(() => ({ evolved: vi.fn(), metrics: vi.fn(), seesaw: vi.fn(), variants: vi.fn(), landscape: vi.fn() }));
vi.mock('../lib/harness.js', () => ({ getEvolvedSkills: api.evolved, getHarnessMetrics: api.metrics, getSeesawRegistry: api.seesaw, getVariantPool: api.variants, getAdaptationLandscape: api.landscape }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
const data = (name: string) => ({ evolved_skills: [{ name, origin: 'pattern', confidence: .8, project: name, active: true, skill_md: '# Description of ' + name, created_at: null, updated_at: '2026-11-09' }], evolution_history: [], total_sessions_analyzed: 0, patterns_detected: 0 });
it('keeps every Evolution panel scoped to the current project when an old response finishes late', async () => {
  let release!: (value: unknown) => void;
  api.evolved.mockImplementationOnce(() => new Promise(resolve => { release = resolve; })).mockResolvedValue(data('beta-skill'));
  for (const fn of [api.metrics, api.seesaw, api.variants, api.landscape]) fn.mockResolvedValue(null);
  selectedProject.set('alpha'); render(Evolution);
  await waitFor(() => expect(release).toBeDefined());
  selectedProject.set('beta');
  expect(await screen.findByText('beta-skill')).toBeTruthy();
  release(data('alpha-skill')); await tick(); await tick();
  expect(screen.queryByText('alpha-skill')).toBeNull();
  expect(screen.getByText('beta-skill')).toBeTruthy();
  expect(api.evolved).toHaveBeenCalledWith('alpha');
  expect(api.evolved).toHaveBeenCalledWith('beta');
  expect(api.metrics).toHaveBeenCalledWith('beta');
  expect(api.metrics).not.toHaveBeenCalledWith('alpha');
});
