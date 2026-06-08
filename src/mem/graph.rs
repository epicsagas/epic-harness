//! graph.rs — Graph build + traversal via llm-kernel

use serde::{Deserialize, Serialize};
use std::io;

use super::store::conn::memory_conn;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub tags: Vec<String>,
    #[serde(default = "default_graph_importance")]
    pub importance: f64,
    #[serde(default)]
    pub projects: Vec<String>,
    #[serde(default)]
    pub accessed_at: String,
}

fn default_graph_importance() -> f64 {
    0.5
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

/// Maximum number of edges returned in a graph payload.
const MAX_GRAPH_EDGES: usize = 2000;

/// Build a `Graph` value from the DB.
#[allow(dead_code)]
pub async fn build_graph_pool(_pool: &sqlx::AnyPool) -> io::Result<Graph> {
    build_graph_sync()
}

fn build_graph_sync() -> io::Result<Graph> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    let ids = llm_kernel::graph::store::list_node_ids(&guard)
        .map_err(|e| io::Error::other(e.to_string()))?;
    let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let nodes = llm_kernel::graph::store::read_nodes(&guard, &id_refs)
        .map_err(|e| io::Error::other(e.to_string()))?
        .into_iter()
        .map(|n| GraphNode {
            id: n.id,
            title: n.title,
            node_type: n.node_type,
            tags: n.tags,
            importance: n.importance,
            projects: n.projects,
            accessed_at: n.accessed_at,
        })
        .collect();
    let edges = llm_kernel::graph::store::read_edges(&guard, MAX_GRAPH_EDGES)
        .map_err(|e| io::Error::other(e.to_string()))?
        .into_iter()
        .map(|e| GraphEdge {
            source: e.source,
            target: e.target,
            relation: e.relation,
            weight: e.weight,
        })
        .collect();
    Ok(Graph { nodes, edges })
}

/// Build graph JSON string — sync wrapper.
pub async fn rebuild_graph_json_pool(_pool: &sqlx::AnyPool) -> io::Result<String> {
    rebuild_graph_json()
}

/// Build graph JSON string — sync wrapper.
pub fn rebuild_graph_json() -> io::Result<String> {
    let graph = build_graph_sync()?;
    serde_json::to_string_pretty(&graph).map_err(io::Error::other)
}

/// Write the graph JSON to the graph file on disk.
pub fn rebuild_graph() -> io::Result<()> {
    let graph = build_graph_sync()?;
    let data = serde_json::to_vec_pretty(&graph).map_err(io::Error::other)?;
    use super::store::atomic_write;
    use super::store::graph_path;
    atomic_write(&graph_path(), &data)
}

/// Maximum seeds accepted per call.
const MAX_SEED_IDS: usize = 100;

/// Async 1-hop neighbors.
pub async fn graph_neighbors_pool(
    _pool: &sqlx::AnyPool,
    seed_ids: &[String],
) -> Vec<(String, f64)> {
    graph_neighbors_sync(seed_ids)
}

fn graph_neighbors_sync(seed_ids: &[String]) -> Vec<(String, f64)> {
    if seed_ids.is_empty() {
        return vec![];
    }
    let seed_ids = if seed_ids.len() > MAX_SEED_IDS {
        eprintln!(
            "[mem/graph] graph_neighbors: seed_ids.len()={} exceeds MAX_SEED_IDS={}, truncating",
            seed_ids.len(),
            MAX_SEED_IDS
        );
        &seed_ids[..MAX_SEED_IDS]
    } else {
        seed_ids
    };
    let conn = match memory_conn() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let guard = match conn.lock() {
        Ok(g) => g,
        Err(_) => return vec![],
    };
    llm_kernel::graph::traversal::graph_neighbors(&guard, seed_ids)
}

/// Get 1-hop neighbors — sync wrapper.
#[allow(dead_code)]
pub fn graph_neighbors(seed_ids: &[String]) -> Vec<(String, f64)> {
    graph_neighbors_sync(seed_ids)
}

/// Async BFS traversal.
pub async fn related_nodes_pool(
    _pool: &sqlx::AnyPool,
    start_id: &str,
    depth: usize,
) -> Vec<String> {
    related_nodes_sync(start_id, depth)
}

fn related_nodes_sync(start_id: &str, depth: usize) -> Vec<String> {
    let conn = match memory_conn() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let guard = match conn.lock() {
        Ok(g) => g,
        Err(_) => return vec![],
    };
    llm_kernel::graph::traversal::related_nodes(&guard, start_id, depth)
}

/// BFS traversal from `start_id` — sync wrapper.
pub fn related_nodes(start_id: &str, depth: usize) -> Vec<String> {
    related_nodes_sync(start_id, depth)
}

/// Async compute aggregate stats.
pub async fn compute_stats_pool(_pool: &sqlx::AnyPool) -> io::Result<serde_json::Value> {
    compute_stats()
}

/// Compute aggregate stats — sync.
pub fn compute_stats() -> io::Result<serde_json::Value> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    let stats = llm_kernel::graph::lifecycle::compute_stats(&guard)
        .map_err(|e| io::Error::other(e.to_string()))?;
    serde_json::to_value(stats).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::super::store::conn::test_conn;
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Test graph_neighbors using a test connection.
    #[test]
    fn graph_neighbors_returns_direct_neighbors() {
        let conn = test_conn();
        let guard = conn.lock().unwrap();
        let edge = llm_kernel::graph::types::GraphEdge {
            id: "e1".to_string(),
            source: "A".to_string(),
            target: "B".to_string(),
            relation: "related".to_string(),
            weight: 1.0,
            ts: "2026-01-01T00:00:00Z".to_string(),
        };
        llm_kernel::graph::store::append_edge(&guard, &edge).unwrap();
        let result = llm_kernel::graph::traversal::graph_neighbors(&guard, &["A".to_string()]);
        let ids: Vec<&str> = result.iter().map(|r| r.0.as_str()).collect();
        assert!(ids.contains(&"B"), "B should be a neighbor of A");
    }

    /// GraphNode includes importance field serialized as JSON.
    #[test]
    fn graph_node_includes_importance() {
        let n = GraphNode {
            id: "test-1".to_string(),
            title: "Test".to_string(),
            node_type: "concept".to_string(),
            tags: vec![],
            importance: 0.85,
            projects: vec![],
            accessed_at: String::new(),
        };
        let json = serde_json::to_string(&n).unwrap();
        assert!(
            json.contains("\"importance\":0.85"),
            "importance should appear in JSON: {json}"
        );
    }
}
