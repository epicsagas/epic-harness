// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import { selectedProject } from '../lib/stores/project.js';
import Agents from './Agents.svelte';

const api = vi.hoisted(() => ({ activity: vi.fn(), summary: vi.fn() }));
vi.mock('../lib/harness.js', () => ({ getLiveActivity: api.activity, getObsSummary: api.summary }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
const summary = { recent_sessions: [], tool_stats: [], total_tool_calls: 0, avg_score: 0, failure_categories: [], active_agents: [] };
const activity = (id: string) => ({ runs: [], sessions: [{ session_id: id, project: 'test', last_seen: new Date().toISOString(), tool_calls: 2 }], recent_sessions: [], activity_window_seconds: 300 });

describe('Live Agents activity', () => {
  it('keeps same-ID subagents in different projects visible', async () => {
    const runs = ['project-one', 'project-two'].map(project => ({
      project, id: 'same-run', status: 'running', dependency_graph: {}, agent_statuses: {},
      agents: [{ id: 'same-id', role: project + '-tester', task: 'synthetic task', status: 'running', satisfies: [], started_at: new Date().toISOString() }],
    }));
    api.activity.mockResolvedValue({ ...activity('coding-session'), runs });
    api.summary.mockResolvedValue(summary);
    render(Agents);
    expect(await screen.findByText('project-one-tester')).toBeTruthy();
    expect(screen.getByText('project-two-tester')).toBeTruthy();
    expect(screen.getAllByText('same-id')).toHaveLength(2);
  });
  it('shows ordinary coding-session activity without an orchestration run', async () => {
    selectedProject.set('__all__');
    api.activity.mockResolvedValue(activity('active-session'));
    api.summary.mockResolvedValue(summary);
    render(Agents);
    expect(await screen.findByText('active-session')).toBeTruthy();
    expect(screen.getByText('Recently active coding sessions')).toBeTruthy();
    expect(screen.queryByText('Start a pipeline with /orbit to see live agent activity here')).toBeNull();
  });

  it('does not overwrite the new project with a late previous response', async () => {
    let resolveOld!: (value: unknown) => void;
    const old = new Promise(resolve => { resolveOld = resolve; });
    api.activity.mockImplementation((project: string) => project === 'one' ? old : Promise.resolve(activity('new-project-session')));
    api.summary.mockResolvedValue(summary);
    selectedProject.set('one');
    render(Agents);
    await waitFor(() => expect(api.activity).toHaveBeenCalledWith('one'));
    selectedProject.set('two');
    expect(await screen.findByText('new-project-session')).toBeTruthy();
    resolveOld(activity('old-project-session'));
    await new Promise(resolve => setTimeout(resolve, 10));
    expect(screen.queryByText('old-project-session')).toBeNull();
    expect(screen.getByText('new-project-session')).toBeTruthy();
  });

  it('shows a load error instead of pretending no agents exist', async () => {
    api.activity.mockRejectedValue(new Error('Live activity unavailable'));
    api.summary.mockResolvedValue(summary);
    render(Agents);
    expect(await screen.findByText(/Live activity unavailable/)).toBeTruthy();
  });
});
