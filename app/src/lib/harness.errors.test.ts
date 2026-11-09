// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { getHarnessMetrics } from './harness.js';

afterEach(() => vi.unstubAllGlobals());
describe('production harness errors', () => {
  it.each([null, { error: 'fixture unavailable' }])('never replaces absent data with invented sessions (%j)', async data => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => data }));
    await expect(getHarnessMetrics()).rejects.toThrow();
  });
  it('propagates HTTP failures instead of returning demo metrics', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 503 }));
    await expect(getHarnessMetrics()).rejects.toThrow();
  });
  it('propagates a network failure', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('offline')));
    await expect(getHarnessMetrics()).rejects.toThrow('offline');
  });
  it('retains genuine empty metrics', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => ({ total_sessions: 0, score_history: [] }) }));
    expect((await getHarnessMetrics()).session_count).toBe(0);
  });
});
