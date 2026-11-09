// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { getOrbitPipelines } from './harness.js';
afterEach(() => vi.unstubAllGlobals());
it('includes the selected project when listing browser pipelines', async () => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => [] }));
  await getOrbitPipelines('fixture project');
  const query = new URL(String(vi.mocked(fetch).mock.calls[0][0]), 'http://fixture').searchParams;
  expect(query.get('project')).toBe('fixture project');
});
