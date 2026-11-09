// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import Memory from './Memory.svelte';
const api = vi.hoisted(() => ({ graph: vi.fn(), stats: vi.fn(), nodes: vi.fn(), search: vi.fn(), detail: vi.fn() }));
vi.mock('../lib/harness.js', () => ({ getGraph: api.graph, getGraphStats: api.stats, getMemoryNodes: api.nodes, searchMemory: api.search, getMemoryNode: api.detail }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
it('opens memory details using the keyboard-focusable title button', async () => {
  const node = { id: 'one', title: 'Keyboard fixture', type: 'concept', tags: [], projects: [], updated: '' };
  api.graph.mockResolvedValue({ nodes: [], edges: [] }); api.stats.mockResolvedValue(null); api.nodes.mockResolvedValue([node]);
  api.detail.mockResolvedValue({ ...node, body: 'Keyboard accessible detail' });
  render(Memory);
  const button = await screen.findByRole('button', { name: 'Keyboard fixture' });
  expect(button.tabIndex).toBe(0);
  button.focus(); expect(document.activeElement).toBe(button);
  await fireEvent.click(button);
  expect(await screen.findByText('Keyboard accessible detail')).toBeTruthy();
  expect(api.detail).toHaveBeenCalledWith('one');
});
