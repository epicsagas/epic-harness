export interface GraphNode {
  id: string;
  title: string;
  type: string;
  tags: string[];
  importance: number;
}

export interface GraphEdge {
  source: string;
  target: string;
  relation: string;
  weight: number;
}

export interface GraphData {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export interface NodeResponse {
  id: string;
  type: string;
  title: string;
  tags: string[];
  projects: string[];
  updated: string;
}

export interface NodeDetail {
  id: string;
  type: string;
  title: string;
  tags: string[];
  projects: string[];
  agents: string[];
  created: string;
  updated: string;
  importance: number;
  access_count: number;
  body: string;
}

export interface EdgeResponse {
  id: string;
  source: string;
  target: string;
  relation: string;
  weight: number;
}

export interface SearchResult {
  id: string;
  title: string;
  type: string;
  snippet: string;
}

export interface ScoredNode {
  id: string;
  title: string;
  type: string;
  score: number;
  body: string;
  tags: string[];
  importance: number;
}

export interface Stats {
  total_nodes: number;
  total_edges: number;
  avg_importance: number;
  by_type: Record<string, number>;
}

export interface NeighborResponse {
  id: string;
  weight: number;
}

export interface CreateNodeInput {
  title: string;
  type?: string;
  body?: string;
  tags?: string[];
  projects?: string[];
  importance?: number;
}

export interface UpdateNodeInput {
  title?: string;
  type?: string;
  body?: string;
  tags?: string[];
  importance?: number;
}

export interface CreateEdgeInput {
  source: string;
  target: string;
  relation?: string;
  weight?: number;
}
