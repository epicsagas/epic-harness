// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Memory from './Memory.svelte';
const api = vi.hoisted(() => ({ graph: vi.fn(), stats: vi.fn(), nodes: vi.fn(), search: vi.fn(), detail: vi.fn() }));
vi.mock('../lib/harness.js', () => ({ getGraph: api.graph, getGraphStats: api.stats, getMemoryNodes: api.nodes, searchMemory: api.search, getMemoryNode: api.detail }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
it('shows zero results after a completed empty search and restores the full list only when search is cleared', async () => {
  api.graph.mockResolvedValue({ nodes: [], edges: [] }); api.stats.mockResolvedValue(null);
  api.nodes.mockResolvedValue([{ id: 'one', title: 'Unrelated fixture', type: 'concept', tags: [], projects: [], updated: '' }]);
  api.search.mockResolvedValue([]);
  const view = render(Memory);
  expect(await screen.findByText('Unrelated fixture')).toBeTruthy();
  const input = view.container.querySelector('input')!;
  await fireEvent.input(input, { target: { value: 'no-match' } });
  await fireEvent.keyDown(input, { key: 'Enter' });
  await waitFor(() => expect(api.search).toHaveBeenCalledWith('no-match'));
  await waitFor(() => expect(screen.queryByText('Unrelated fixture')).toBeNull());
  await fireEvent.input(input, { target: { value: '' } });
  await fireEvent.keyDown(input, { key: 'Enter' });
  expect(await screen.findByText('Unrelated fixture')).toBeTruthy();
});
