// @vitest-environment jsdom
// A slow Evolution response for the previously selected project must not
// overwrite the view of the project selected afterwards.

import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { selectedProject } from '$lib/stores/project.js';
import Evolution from './Evolution.svelte';

const h = vi.hoisted(() => ({
  evolved: [] as Array<{ project?: string; resolve: (v: unknown) => void }>,
  followUps: [] as Array<string | undefined>,
}));

vi.mock('../lib/harness.js', () => {
  const followUp = (project?: string) => {
    h.followUps.push(project);
    return Promise.reject(new Error('not available'));
  };
  return {
    getEvolvedSkills: (project?: string) =>
      new Promise((resolve) => h.evolved.push({ project, resolve })),
    getHarnessMetrics: followUp,
    getSeesawRegistry: followUp,
    getVariantPool: followUp,
    getAdaptationLandscape: followUp,
  };
});

const evolution = (sessions: number) => ({
  evolved_skills: [],
  evolution_history: [],
  total_sessions_analyzed: sessions,
  patterns_detected: 0,
});

const flush = async () => {
  for (let i = 0; i < 5; i++) {
    await tick();
    await Promise.resolve();
  }
};

describe('Evolution project switch', () => {
  it('ignores a late response for the previous project', async () => {
    selectedProject.set('proj-a');
    const { container } = render(Evolution);
    await flush();
    selectedProject.set('proj-b');
    await flush();
    expect(h.evolved.map((c) => c.project)).toEqual(['proj-a', 'proj-b']);

    // The selected project's response lands first, then the stale one.
    h.evolved[1].resolve(evolution(222));
    await flush();
    h.evolved[0].resolve(evolution(111));
    await flush();

    expect(container.textContent).toContain('222');
    expect(container.textContent).not.toContain('111');
    // Follow-up requests use the project captured at load start.
    expect(new Set(h.followUps)).toEqual(new Set(['proj-b']));
  });
});
