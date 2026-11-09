// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { getGraphStats } from './harness.js';
afterEach(() => vi.unstubAllGlobals());
it('normalizes the actual backend graph statistics into displayed counters', async () => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => ({ nodes: 7, edges: 3, types: { concept: 7 }, avg_importance: .7 }) }));
  expect(await getGraphStats()).toEqual({ total_nodes: 7, total_edges: 3, by_type: { concept: 7 }, avg_importance: .7 });
});
