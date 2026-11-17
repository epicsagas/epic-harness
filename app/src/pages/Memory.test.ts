// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { t } from '$lib/i18n.js';
import Memory from './Memory.svelte';

const api = vi.hoisted(() => ({
  getGraph: vi.fn(),
  getGraphStats: vi.fn(),
  getMemoryNodes: vi.fn(),
  searchMemory: vi.fn(),
  getMemoryNode: vi.fn(),
}));
vi.mock('../lib/harness.js', () => api);

const node = (id: string, title: string) => ({
  id, type: 'concept', title, tags: [], projects: [], updated: '2026-01-01T00:00:00Z', importance: 0.5,
});

function setup() {
  api.getGraph.mockResolvedValue({ nodes: [], edges: [] });
  api.getGraphStats.mockResolvedValue({ nodes: 7, edges: 4, avg_importance: 0.5, types: {} });
  api.getMemoryNodes.mockResolvedValue([node('n1', 'Loaded alpha'), node('n2', 'Loaded beta')]);
  api.getMemoryNode.mockResolvedValue({ ...node('n1', 'Loaded alpha'), body: 'detail body' });
}

async function search(query: string) {
  const input = screen.getByPlaceholderText(t('memorySearchPlaceholder'));
  await fireEvent.input(input, { target: { value: query } });
  await fireEvent.keyDown(input, { key: 'Enter' });
}

afterEach(() => { cleanup(); vi.clearAllMocks(); });

describe('Memory page', () => {
  it('shows the noResults row and none of the loaded nodes for a zero-match search', async () => {
    setup();
    api.searchMemory.mockResolvedValue([]);
    render(Memory);
    await screen.findByText('Loaded alpha');
    await search('zzz');
    await screen.findByText(t('noResults'));
    expect(screen.queryByText('Loaded alpha')).toBeNull();
    expect(screen.queryByText('Loaded beta')).toBeNull();
  });

  it('ignores a slow first search that resolves after a second one', async () => {
    setup();
    let resolveFirst!: (v: unknown) => void;
    api.searchMemory
      .mockReturnValueOnce(new Promise(r => { resolveFirst = r; }))
      .mockResolvedValueOnce([node('n3', 'Second hit')]);
    render(Memory);
    await screen.findByText('Loaded alpha');
    await search('one');
    await search('two');
    await screen.findByText('Second hit');
    resolveFirst([node('n4', 'First hit')]);
    await new Promise(r => setTimeout(r, 0));
    expect(screen.queryByText('First hit')).toBeNull();
    expect(screen.getByText('Second hit')).toBeTruthy();
  });

  it('opens details from the title button via keyboard activation', async () => {
    setup();
    render(Memory);
    const btn = await screen.findByRole('button', { name: 'Loaded alpha' });
    expect(btn.getAttribute('type')).toBe('button');
    btn.focus();
    expect(document.activeElement).toBe(btn);
    // jsdom does not turn Enter into a click; a browser does for a native button.
    await fireEvent.click(btn);
    await waitFor(() => expect(api.getMemoryNode).toHaveBeenCalledWith('n1'));
    // stopPropagation keeps the row handler from firing a second time.
    expect(api.getMemoryNode).toHaveBeenCalledTimes(1);
    await screen.findByText('detail body');
  });

  it('shows the nodes and edges counts in the stats cards', async () => {
    setup();
    render(Memory);
    await screen.findByText('7');
    expect(screen.getByText('4')).toBeTruthy();
  });
});
