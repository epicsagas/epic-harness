import { writable, derived } from 'svelte/store';
import { api } from '$lib/api/commands';
import type { GraphData, Stats, NodeDetail, SearchResult } from '$lib/api/types';

export const graphData = writable<GraphData>({ nodes: [], edges: [] });
export const stats = writable<Stats | null>(null);
export const selectedNode = writable<NodeDetail | null>(null);
export const searchResults = writable<SearchResult[]>([]);
export const lastError = writable<string | null>(null);

let loadSeq = 0;
let selectSeq = 0;
let statsSeq = 0;
let searchSeq = 0;

export const typeFilters = writable<Record<string, boolean>>({});

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

function handleError(msg: string, e: unknown) {
  const detail = e instanceof Error ? e.message : String(e);
  lastError.set(`${msg}: ${detail}`);
  console.error(msg, e);
}

export function clearError() {
  lastError.set(null);
}

export async function loadGraph() {
  const seq = ++loadSeq;
  try {
    const data = await api.getGraph();
    if (seq !== loadSeq) return;
    graphData.set(data);
  } catch (e) {
    if (seq !== loadSeq) return;
    handleError('Failed to load graph', e);
  }
}

export async function loadStats() {
  const seq = ++statsSeq;
  try {
    const s = await api.getStats();
    if (seq !== statsSeq) return;
    stats.set(s);
  } catch (e) {
    if (seq !== statsSeq) return;
    handleError('Failed to load stats', e);
  }
}

export async function selectNodeById(id: string) {
  const seq = ++selectSeq;
  try {
    const detail = await api.getNode(id);
    if (seq !== selectSeq) return;
    selectedNode.set(detail);
  } catch (e) {
    if (seq !== selectSeq) return;
    handleError('Failed to load node', e);
    selectedNode.set(null);
  }
}

export async function search(query: string) {
  const seq = ++searchSeq;
  if (query.trim().length < 2) {
    searchResults.set([]);
    return;
  }
  try {
    const results = await api.searchNodes(query, 20);
    if (seq !== searchSeq) return;
    searchResults.set(results);
  } catch {
    if (seq !== searchSeq) return;
    searchResults.set([]);
  }
}

export function clearSelection() {
  selectedNode.set(null);
}

export async function addNode(input: { type: string; title: string; body: string; tags?: string[]; projects?: string[]; importance?: number }) {
  const id = await api.createNode(input);
  await Promise.all([loadGraph(), loadStats()]);
  return id;
}

export async function updateNode(id: string, input: { title?: string; body?: string; tags?: string[]; importance?: number }) {
  await api.updateNode(id, input);
  await Promise.all([loadGraph(), selectNodeById(id)]);
}

export async function removeNode(id: string) {
  await api.deleteNode(id);
  selectedNode.set(null);
  await Promise.all([loadGraph(), loadStats()]);
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
