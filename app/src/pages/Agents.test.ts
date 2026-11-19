// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import { selectedProject } from '$lib/stores/project.js';
import Agents from './Agents.svelte';

const api = vi.hoisted(() => ({
  getOrchestratorRun: vi.fn(),
  getOrchestratorAgentStatus: vi.fn(),
  getObsSummary: vi.fn(),
}));
vi.mock('../lib/harness.js', () => api);

const run = (id: string) => ({
  id, status: 'running', dependency_graph: {}, created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z',
  agents: [{ id: `ag-${id}`, role: 'builder', task: `task-${id}`, satisfies: [], status: 'running' }],
});

afterEach(() => { cleanup(); vi.clearAllMocks(); selectedProject.set('__all__'); });

describe('Agents page', () => {
  it('passes the selected project to both reads', async () => {
    selectedProject.set('proj-a');
    api.getObsSummary.mockResolvedValue(null);
    api.getOrchestratorRun.mockResolvedValue(run('r1'));
    api.getOrchestratorAgentStatus.mockResolvedValue(null);
    render(Agents);
    await waitFor(() => expect(api.getOrchestratorAgentStatus).toHaveBeenCalledWith('ag-r1', 'proj-a'));
    expect(api.getOrchestratorRun).toHaveBeenCalledWith('proj-a');
  });

  it('ignores a slow first load that resolves after a project switch', async () => {
    selectedProject.set('proj-a');
    api.getObsSummary.mockResolvedValue(null);
    api.getOrchestratorAgentStatus.mockResolvedValue(null);
    let resolveFirst!: (v: unknown) => void;
    api.getOrchestratorRun.mockImplementation((project: string) =>
      project === 'proj-a' ? new Promise(r => { resolveFirst = r; }) : Promise.resolve(run('second')));
    render(Agents);
    await waitFor(() => expect(api.getOrchestratorRun).toHaveBeenCalledWith('proj-a'));
    selectedProject.set('proj-b');
    await screen.findByText('task-second');
    resolveFirst(run('first'));
    await new Promise(r => setTimeout(r, 0));
    expect(screen.queryByText('task-first')).toBeNull();
    expect(screen.getByText('task-second')).toBeTruthy();
  });
  it('does not render the previous project statuses while the new request is pending', async () => {
    const shared = (task: string) => ({ ...run('x'), agents: [{ ...run('x').agents[0], id: 'ag-shared', task }] });
    selectedProject.set('proj-a');
    api.getObsSummary.mockResolvedValue(null);
    api.getOrchestratorRun.mockImplementation((project: string) =>
      Promise.resolve(shared(project === 'proj-a' ? 'task-a' : 'task-b')));
    api.getOrchestratorAgentStatus.mockImplementation((_id: string, project: string) =>
      project === 'proj-a'
        ? Promise.resolve({ progress: 0.5, status: 'running', last_heartbeat: new Date().toISOString() })
        : new Promise(() => {}));
    render(Agents);
    await screen.findByText(/heartbeat/);
    selectedProject.set('proj-b');
    await waitFor(() => expect(api.getOrchestratorAgentStatus).toHaveBeenCalledWith('ag-shared', 'proj-b'));
    expect(screen.queryByText(/heartbeat/)).toBeNull();
  });
  it('shows the loading skeleton instead of the empty panel while the new project loads', async () => {
    selectedProject.set('proj-a');
    api.getObsSummary.mockResolvedValue(null);
    api.getOrchestratorAgentStatus.mockResolvedValue(null);
    api.getOrchestratorRun.mockImplementation((project: string) =>
      project === 'proj-a' ? Promise.resolve(run('a')) : new Promise(() => {}));
    const { container } = render(Agents);
    await screen.findByText('task-a');
    selectedProject.set('proj-b');
    await waitFor(() => expect(container.querySelector('.skeleton')).toBeTruthy());
    expect(screen.queryByText('task-a')).toBeNull();
  });

  it('clears the statuses when a poll returns no run', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    try {
      const shared = { ...run('x'), agents: [{ ...run('x').agents[0], id: 'ag-shared' }] };
      selectedProject.set('proj-a');
      api.getObsSummary.mockResolvedValue(null);
      api.getOrchestratorRun
        .mockResolvedValueOnce(shared)
        .mockResolvedValueOnce(null)
        .mockResolvedValue(shared);
      api.getOrchestratorAgentStatus
        .mockResolvedValueOnce({ progress: 0.5, status: 'running', last_heartbeat: new Date().toISOString() })
        .mockImplementation(() => new Promise(() => {}));
      render(Agents);
      await screen.findByText(/heartbeat/);
      await vi.advanceTimersByTimeAsync(5000);
      await screen.findByText('No active agents');
      await vi.advanceTimersByTimeAsync(5000);
      await screen.findByText('task-x');
      expect(screen.queryByText(/heartbeat/)).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});
