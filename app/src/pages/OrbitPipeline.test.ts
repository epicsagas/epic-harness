// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import OrbitPipeline from './OrbitPipeline.svelte';

const api = vi.hoisted(() => ({ list: vi.fn(), dismiss: vi.fn(), desktop: vi.fn(() => false) }));
vi.mock('../lib/harness.js', () => ({ getOrbitPipelines: api.list, dismissOrbitPipeline: api.dismiss, isDesktop: api.desktop }));

const pipeline = (phase: string, extra: Record<string, unknown> = {}) => ({
  id: 'sid-1-fixture', goal_slug: 'fixture-goal', phase, status: 'running', mode: 'direct', branch: null,
  audit_fail_count: 0, started_at: '2026-11-09T12:00:00Z', updated_at: '2026-11-09T12:01:00Z',
  deadline: null, phase_history: [], ...extra,
});

const runningLabel = (c: HTMLElement) => c.querySelector('.pipeline-stage.running .stage-label')?.textContent;

afterEach(() => { cleanup(); vi.clearAllMocks(); api.desktop.mockReturnValue(false); });

describe('OrbitPipeline phases', () => {
  it.each(['audit', 'eval', 'evolve'])('marks the %s phase as running', async phase => {
    api.list.mockResolvedValue([pipeline(phase)]);
    const view = render(OrbitPipeline);
    await waitFor(() => expect(runningLabel(view.container)).toBe(phase));
  });

  it('highlights audit for the legacy check phase', async () => {
    api.list.mockResolvedValue([pipeline('check')]);
    const view = render(OrbitPipeline);
    await waitFor(() => expect(runningLabel(view.container)).toBe('audit'));
  });
});

describe('OrbitPipeline dismiss control by runtime', () => {
  it('shows the dismiss control in the browser build', async () => {
    api.list.mockResolvedValue([pipeline('go')]);
    render(OrbitPipeline);
    await screen.findByText('fixture-goal');
    expect(screen.getAllByTitle('Dismiss').length).toBeGreaterThan(0);
  });

  it('hides the dismiss control in the desktop app', async () => {
    api.desktop.mockReturnValue(true);
    api.list.mockResolvedValue([pipeline('go')]);
    render(OrbitPipeline);
    await screen.findByText('fixture-goal');
    expect(screen.queryByTitle('Dismiss')).toBeNull();
  });
});

describe('OrbitPipeline dismiss', () => {
  it('keeps the row and shows the error when dismiss fails', async () => {
    api.list.mockResolvedValue([pipeline('go')]);
    api.dismiss.mockRejectedValue(new Error('dismiss failed (404)'));
    render(OrbitPipeline);
    await screen.findByText('fixture-goal');
    await fireEvent.click(screen.getByTitle('Dismiss'));
    await screen.findByText('dismiss failed (404)');
    expect(screen.getByText('fixture-goal')).toBeTruthy();
  });

  it('removes the row and clears the error on success', async () => {
    api.list.mockResolvedValue([pipeline('go'), pipeline('go', { id: 'sid-2-fixture', goal_slug: 'other-goal' })]);
    api.dismiss.mockRejectedValueOnce(new Error('dismiss failed (500)')).mockResolvedValueOnce({ ok: true });
    render(OrbitPipeline);
    await screen.findByText('fixture-goal');
    await fireEvent.click(screen.getAllByTitle('Dismiss')[0]);
    await screen.findByText('dismiss failed (500)');
    await fireEvent.click(screen.getAllByTitle('Dismiss')[0]);
    await waitFor(() => expect(screen.queryByText('fixture-goal')).toBeNull());
    expect(screen.queryByText('dismiss failed (500)')).toBeNull();
    expect(screen.getByText('other-goal')).toBeTruthy();
  });
});
