// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { getOrbitPipelines } from './harness.js';
afterEach(() => vi.unstubAllGlobals());
it('maps persisted audit retry counts to the dashboard counter', async () => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => [{ id: 'pipeline', audit_fail_count: 2 }] }));
  expect((await getOrbitPipelines())[0].check_fail_count).toBe(2);
});
it('prefers the canonical persisted counter while retaining legacy-only counters', async () => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => [{ id: 'canonical', audit_fail_count: 2, check_fail_count: 0 }, { id: 'legacy', check_fail_count: 3 }] }));
  expect((await getOrbitPipelines()).map(p => p.check_fail_count)).toEqual([2, 3]);
});
