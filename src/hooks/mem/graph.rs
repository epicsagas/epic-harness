//! graph.rs — Graph build + traversal (related, rebuild)

use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::io;

use super::store::{atomic_write, graph_path, list_node_ids, read_edges, read_node};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

pub fn rebuild_graph() -> io::Result<()> {
    let ids = list_node_ids()?;
    let mut nodes = vec![];
    for id in &ids {
        if let Ok(node) = read_node(id) {
            nodes.push(GraphNode {
                id: node.frontmatter.id,
                title: node.frontmatter.title,
                node_type: node.frontmatter.node_type,
                tags: node.frontmatter.tags,
            });
        }
    }
    let raw_edges = read_edges();
    let edges: Vec<GraphEdge> = raw_edges
        .into_iter()
        .map(|e| GraphEdge {
            source: e.source,
            target: e.target,
            relation: e.relation,
            weight: e.weight,
        })
        .collect();

    let graph = Graph { nodes, edges };
    let data = serde_json::to_vec_pretty(&graph)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    atomic_write(&graph_path(), &data)?;
    Ok(())
}

/// Build graph JSON string directly from DB (no file I/O).
/// Used by the web server to always return fresh data.
pub fn rebuild_graph_json() -> io::Result<String> {
    let ids = list_node_ids()?;
    let mut nodes = vec![];
    for id in &ids {
        if let Ok(node) = read_node(id) {
            nodes.push(GraphNode {
                id: node.frontmatter.id,
                title: node.frontmatter.title,
                node_type: node.frontmatter.node_type,
                tags: node.frontmatter.tags,
            });
        }
    }
    let raw_edges = read_edges();
    let edges: Vec<GraphEdge> = raw_edges
        .into_iter()
        .map(|e| GraphEdge {
            source: e.source,
            target: e.target,
            relation: e.relation,
            weight: e.weight,
        })
        .collect();

    let graph = Graph { nodes, edges };
    serde_json::to_string_pretty(&graph)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Get 1-hop neighbors for multiple seed nodes, excluding the seeds themselves.
/// Returns deduplicated neighbor IDs with their connection count (how many seeds link to them).
pub fn graph_neighbors(seed_ids: &[String]) -> Vec<(String, usize)> {
    let edges = read_edges();
    let seed_set: HashSet<&str> = seed_ids.iter().map(|s| s.as_str()).collect();
    let mut neighbor_count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for edge in &edges {
        if seed_set.contains(edge.source.as_str()) && !seed_set.contains(edge.target.as_str()) {
            *neighbor_count.entry(edge.target.clone()).or_default() += 1;
        }
        if seed_set.contains(edge.target.as_str()) && !seed_set.contains(edge.source.as_str()) {
            *neighbor_count.entry(edge.source.clone()).or_default() += 1;
        }
    }

    let mut result: Vec<(String, usize)> = neighbor_count.into_iter().collect();
    result.sort_by(|a, b| b.1.cmp(&a.1)); // most connected first
    result
}

/// BFS traversal from `start_id` up to `depth` hops using the DB edges table.
pub fn related_nodes(start_id: &str, depth: usize) -> Vec<String> {
    let edges = read_edges();

    // Build adjacency map once: O(E)
    let mut adj: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for edge in &edges {
        adj.entry(edge.source.clone())
            .or_default()
            .push(edge.target.clone());
        adj.entry(edge.target.clone())
            .or_default()
            .push(edge.source.clone());
    }

    // BFS: O(N + E) total
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    queue.push_back((start_id.to_string(), 0));
    visited.insert(start_id.to_string());
    let mut result = vec![];

    while let Some((current, d)) = queue.pop_front() {
        if d >= depth {
            continue;
        }
        if let Some(neighbors) = adj.get(&current) {
            for nb in neighbors {
                if !visited.contains(nb.as_str()) {
                    visited.insert(nb.clone());
                    result.push(nb.clone());
                    queue.push_back((nb.clone(), d + 1));
                }
            }
        }
    }
    result
}
