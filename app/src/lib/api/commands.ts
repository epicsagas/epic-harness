import { invoke } from '@tauri-apps/api/core';
import type {
  GraphData,
  NodeResponse,
  NodeDetail,
  EdgeResponse,
  SearchResult,
  ScoredNode,
  Stats,
  NeighborResponse,
  CreateNodeInput,
  UpdateNodeInput,
  CreateEdgeInput,
} from './types';

const isTauri = typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__;

/**
 * Shared request helper for browser mode (HTTP fetch).
 * Maps Tauri commands to REST endpoints.
 */
async function request<T>(path: string, method: string = 'GET', body?: any): Promise<T> {
  const url = `/api${path}`;
  const options: RequestInit = {
    method,
    headers: { 'Content-Type': 'application/json' },
  };
  if (body) {
    options.body = JSON.stringify(body);
  }

  const res = await fetch(url, options);
  if (!res.ok) {
    const errorData = await res.json().catch(() => ({}));
    throw new Error(errorData.error || `HTTP error! status: ${res.status}`);
  }
  
  // Handle empty responses (like 201 Created or 204 No Content)
  if (res.status === 204 || res.headers.get('content-length') === '0') {
    return {} as T;
  }

  return res.json();
}

export const api = {
  // Graph
  getGraph: (): Promise<GraphData> =>
    isTauri ? invoke('get_graph') : request<GraphData>('/graph'),
    
  getNeighbors: (ids: string[]): Promise<NeighborResponse[]> =>
    isTauri ? invoke('get_neighbors', { ids }) : request<NeighborResponse[]>(`/graph/neighbors?ids=${ids.join(',')}`),

  // Nodes
  getNodes: (): Promise<NodeResponse[]> =>
    isTauri ? invoke('get_nodes') : request<NodeResponse[]>('/nodes'),
    
  getNode: (id: string): Promise<NodeDetail> =>
    isTauri ? invoke('get_node', { id }) : request<NodeDetail>(`/nodes/${id}`),
    
  createNode: (input: CreateNodeInput): Promise<string> =>
    isTauri ? invoke('create_node', { input }) : request<{id: string}>('/nodes', 'POST', input).then(r => r.id),
    
  updateNode: (id: string, input: UpdateNodeInput): Promise<string> =>
    isTauri ? invoke('update_node', { id, input }) : request<{id: string}>(`/nodes/${id}`, 'PUT', input).then(r => r.id),
    
  deleteNode: (id: string): Promise<string> =>
    isTauri ? invoke('delete_node', { id }) : request<{deleted: string}>(`/nodes/${id}`, 'DELETE').then(r => r.deleted),

  // Edges
  getEdges: (): Promise<EdgeResponse[]> =>
    isTauri ? invoke('get_edges') : request<EdgeResponse[]>('/edges'),
    
  createEdge: (input: CreateEdgeInput): Promise<string> =>
    isTauri ? invoke('create_edge', { input }) : request<{edge_id: string}>('/edges', 'POST', input).then(r => r.edge_id),
    
  deleteEdge: (id: string): Promise<string> =>
    isTauri ? invoke('delete_edge', { id }) : request<{deleted: string}>(`/edges/${id}`, 'DELETE').then(r => r.deleted),

  // Search
  searchNodes: (query: string, limit?: number): Promise<SearchResult[]> =>
    isTauri 
      ? invoke('search_nodes', { query, limit }) 
      : request<SearchResult[]>(`/search?q=${encodeURIComponent(query)}${limit ? `&limit=${limit}` : ''}`),
      
  recallNodes: (project?: string, hint?: string, limit?: number): Promise<ScoredNode[]> =>
    isTauri 
      ? invoke('recall_nodes', { project, hint, limit }) 
      : request<ScoredNode[]>(`/recall?project=${project || ''}&hint=${encodeURIComponent(hint || '')}${limit ? `&limit=${limit}` : ''}`),

  // Stats
  getStats: (): Promise<Stats> =>
    isTauri ? invoke('get_stats') : request<Stats>('/stats'),
};
