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

export const api = {
  // Graph
  getGraph: (): Promise<GraphData> => invoke('get_graph'),
  getNeighbors: (ids: string[]): Promise<NeighborResponse[]> => invoke('get_neighbors', { ids }),

  // Nodes
  getNodes: (): Promise<NodeResponse[]> => invoke('get_nodes'),
  getNode: (id: string): Promise<NodeDetail> => invoke('get_node', { id }),
  createNode: (input: CreateNodeInput): Promise<string> => invoke('create_node', { input }),
  updateNode: (id: string, input: UpdateNodeInput): Promise<string> => invoke('update_node', { id, input }),
  deleteNode: (id: string): Promise<string> => invoke('delete_node', { id }),

  // Edges
  getEdges: (): Promise<EdgeResponse[]> => invoke('get_edges'),
  createEdge: (input: CreateEdgeInput): Promise<string> => invoke('create_edge', { input }),
  deleteEdge: (id: string): Promise<string> => invoke('delete_edge', { id }),

  // Search
  searchNodes: (query: string, limit?: number): Promise<SearchResult[]> =>
    invoke('search_nodes', { query, limit }),
  recallNodes: (project?: string, hint?: string, limit?: number): Promise<ScoredNode[]> =>
    invoke('recall_nodes', { project, hint, limit }),

  // Stats
  getStats: (): Promise<Stats> => invoke('get_stats'),
};
