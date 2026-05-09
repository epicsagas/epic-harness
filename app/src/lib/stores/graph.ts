import { writable, derived } from 'svelte/store';
import { api } from '$lib/api/commands';
import type { GraphData, Stats, NodeDetail, SearchResult } from '$lib/api/types';

export const graphData = writable<GraphData>({ nodes: [], edges: [] });
export const stats = writable<Stats | null>(null);
export const selectedNode = writable<NodeDetail | null>(null);
export const searchResults = writable<SearchResult[]>([]);

export const typeFilters = writable<Record<string, boolean>>({});
export const tagFilters = writable<string[]>([]);
export const projectFilter = writable<string>('');

export const filteredNodes = derived(
  [graphData, typeFilters],
  ([$graphData, $typeFilters]) => {
    const activeTypes = Object.entries($typeFilters)
      .filter(([, v]) => v)
      .map(([k]) => k);

    if (activeTypes.length === 0) return $graphData.nodes;
    return $graphData.nodes.filter((n) => activeTypes.includes(n.type));
  }
);

export const filteredEdges = derived(
  [graphData, filteredNodes],
  ([$graphData, $filteredNodes]) => {
    const nodeIds = new Set($filteredNodes.map((n) => n.id));
    return $graphData.edges.filter(
      (e) => nodeIds.has(e.source) && nodeIds.has(e.target)
    );
  }
);

export async function loadGraph() {
  try {
    const data = await api.getGraph();
    graphData.set(data);
  } catch (e) {
    console.error('Failed to load graph:', e);
  }
}

export async function loadStats() {
  try {
    const s = await api.getStats();
    stats.set(s);
  } catch (e) {
    console.error('Failed to load stats:', e);
  }
}

export async function selectNodeById(id: string) {
  try {
    const detail = await api.getNode(id);
    selectedNode.set(detail);
  } catch (e) {
    console.error('Failed to load node:', e);
    selectedNode.set(null);
  }
}

export async function search(query: string) {
  if (query.trim().length < 2) {
    searchResults.set([]);
    return;
  }
  try {
    const results = await api.searchNodes(query, 20);
    searchResults.set(results);
  } catch {
    searchResults.set([]);
  }
}

export function clearSelection() {
  selectedNode.set(null);
}

export async function addNode(input: { type: string; title: string; body: string; tags?: string[]; projects?: string[]; importance?: number }) {
  const id = await api.createNode(input);
  await loadGraph();
  await loadStats();
  return id;
}

export async function updateNode(id: string, input: { title?: string; body?: string; tags?: string[]; importance?: number }) {
  await api.updateNode(id, input);
  await loadGraph();
  await selectNodeById(id);
}

export async function removeNode(id: string) {
  await api.deleteNode(id);
  selectedNode.set(null);
  await loadGraph();
  await loadStats();
}

export async function addEdge(input: { source: string; target: string; relation: string }) {
  const id = await api.createEdge(input);
  await loadGraph();
  return id;
}

export async function removeEdge(id: string) {
  await api.deleteEdge(id);
  await loadGraph();
}
