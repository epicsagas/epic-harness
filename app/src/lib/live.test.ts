import { afterEach, describe, expect, it, vi } from 'vitest';
import { getLiveActivity } from './harness.js';

afterEach(() => vi.unstubAllGlobals());
const data = { runs: [], sessions: [], recent_sessions: [], activity_window_seconds: 300 };
describe('live activity transport', () => {
  it('sends project filters and omits the all-projects sentinel', async () => {
    const fetcher = vi.fn().mockResolvedValue({ ok: true, json: async () => data });
    vi.stubGlobal('fetch', fetcher);
    await getLiveActivity('project-one');
    expect(fetcher.mock.calls[0][0]).toContain('project=project-one');
    await getLiveActivity('__all__');
    expect(fetcher.mock.calls[1][0]).not.toContain('project=');
  });
  it('rejects failed and malformed responses without demo agents', async () => {
    const fetcher = vi.fn().mockResolvedValueOnce({ ok: false }).mockResolvedValueOnce({ ok: true, json: async () => null });
    vi.stubGlobal('fetch', fetcher);
    await expect(getLiveActivity()).rejects.toThrow('Live activity unavailable');
    await expect(getLiveActivity()).rejects.toThrow('Live activity unavailable');
  });
});
