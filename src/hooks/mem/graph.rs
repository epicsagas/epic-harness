/// graph.rs — Graph build + traversal (related, rebuild)

use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io;

use super::store::{
    graph_path, list_node_ids, node_path, read_edges, read_node, atomic_write,
};

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

pub fn read_graph() -> Graph {
    let path = graph_path();
    if !path.exists() {
        return Graph::default();
    }
    let content = fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&content).unwrap_or_default()
}

/// BFS traversal from `start_id` up to `depth` hops using edges.jsonl
pub fn related_nodes(start_id: &str, depth: usize) -> Vec<String> {
    let edges = read_edges();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    queue.push_back((start_id.to_string(), 0));
    visited.insert(start_id.to_string());
    let mut result = vec![];

    while let Some((current, d)) = queue.pop_front() {
        if d >= depth {
            continue;
        }
        for edge in &edges {
            let neighbor = if edge.source == current {
                Some(&edge.target)
            } else if edge.target == current {
                Some(&edge.source)
            } else {
                None
            };
            if let Some(nb) = neighbor {
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

pub fn node_path_exists(id: &str) -> bool {
    node_path(id).exists()
}
