//! graph.rs — Graph build + traversal via llm-kernel

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    #[serde(
        rename = "virtual",
        skip_serializing_if = "std::ops::Not::not",
        default
    )]
    pub virtual_: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Maximum number of edges returned in a graph payload.
const MAX_GRAPH_EDGES: usize = 2000;
/// Maximum number of virtual (computed) cross-project edges.
const MAX_VIRTUAL_EDGES: usize = 500;

/// Build a `Graph` value from the DB.
#[allow(dead_code)]
pub async fn build_graph_pool(_pool: &sqlx::AnyPool) -> io::Result<Graph> {
    build_graph_sync(true)
}

/// Build a `Graph` value from the DB with optional virtual edges.
#[allow(dead_code)]
pub async fn build_graph_pool_virtual(
    _pool: &sqlx::AnyPool,
    include_virtual: bool,
) -> io::Result<Graph> {
    build_graph_sync(include_virtual)
}

fn build_graph_sync(include_virtual: bool) -> io::Result<Graph> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    let ids = llm_kernel::graph::store::list_node_ids(&guard)
        .map_err(|e| io::Error::other(e.to_string()))?;
    let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let nodes: Vec<GraphNode> = llm_kernel::graph::store::read_nodes(&guard, &id_refs)
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
    let mut edges: Vec<GraphEdge> = llm_kernel::graph::store::read_edges(&guard, MAX_GRAPH_EDGES)
        .map_err(|e| io::Error::other(e.to_string()))?
        .into_iter()
        .map(|e| GraphEdge {
            source: e.source,
            target: e.target,
            relation: e.relation,
            weight: e.weight,
            virtual_: false,
        })
        .collect();

    if include_virtual {
        let persisted = edges.len();
        let budget = MAX_GRAPH_EDGES
            .saturating_sub(persisted)
            .min(MAX_VIRTUAL_EDGES);
        if budget > 0 {
            let mut vedges = generate_virtual_edges(&nodes);
            vedges.truncate(budget);
            edges.extend(vedges);
        }
    }

    Ok(Graph { nodes, edges })
}

/// Generate virtual cross-project edges based on shared tags.
///
/// For each pair of nodes in different projects sharing at least one tag,
/// creates a virtual edge with weight = Jaccard similarity of their tag sets.
pub fn generate_virtual_edges(nodes: &[GraphNode]) -> Vec<GraphEdge> {
    // Build inverted index: tag -> list of node indices
    let mut tag_index: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        for tag in &n.tags {
            tag_index.entry(tag.as_str()).or_default().push(i);
        }
    }

    // Accumulate shared tag counts per cross-project pair
    let mut pair_shared: HashMap<(String, String), usize> = HashMap::new();
    for indices in tag_index.values() {
        if indices.len() < 2 {
            continue;
        }
        for i in 0..indices.len() {
            for j in (i + 1)..indices.len() {
                let a = &nodes[indices[i]];
                let b = &nodes[indices[j]];
                // Only cross-project pairs
                if a.projects.iter().any(|p| b.projects.contains(p)) {
                    continue;
                }
                let key = if a.id <= b.id {
                    (a.id.clone(), b.id.clone())
                } else {
                    (b.id.clone(), a.id.clone())
                };
                *pair_shared.entry(key).or_default() += 1;
            }
        }
    }

    // Build per-node tag sets for Jaccard
    let tag_sets: Vec<std::collections::HashSet<&str>> = nodes
        .iter()
        .map(|n| n.tags.iter().map(|t| t.as_str()).collect())
        .collect();
    let node_idx: HashMap<&str, usize> = nodes.iter().map(|n| (n.id.as_str(), n.id.as_str())).fold(
        HashMap::new(),
        |mut m, (id, _)| {
            if let Some(idx) = nodes.iter().position(|n| n.id == id) {
                m.insert(id, idx);
            }
            m
        },
    );

    let mut result: Vec<GraphEdge> = pair_shared
        .into_iter()
        .filter_map(|((a_id, b_id), shared)| {
            let ai = node_idx.get(a_id.as_str())?;
            let bi = node_idx.get(b_id.as_str())?;
            let union_size = tag_sets[*ai].union(&tag_sets[*bi]).count();
            if union_size == 0 {
                return None;
            }
            let weight = (shared as f64 / union_size as f64).clamp(0.1, 1.0);
            Some(GraphEdge {
                source: a_id,
                target: b_id,
                relation: "shared_tag".to_string(),
                weight,
                virtual_: true,
            })
        })
        .collect();

    result.sort_by(|a, b| {
        b.weight
            .partial_cmp(&a.weight)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    result.truncate(MAX_VIRTUAL_EDGES);
    result
}

/// Build graph JSON string — sync wrapper.
#[allow(dead_code)]
pub async fn rebuild_graph_json_pool(_pool: &sqlx::AnyPool) -> io::Result<String> {
    rebuild_graph_json()
}

/// Build graph JSON string — sync wrapper with virtual edges control.
pub async fn rebuild_graph_json_pool_virtual(
    _pool: &sqlx::AnyPool,
    include_virtual: bool,
) -> io::Result<String> {
    let graph = build_graph_sync(include_virtual)?;
    serde_json::to_string_pretty(&graph).map_err(io::Error::other)
}

/// Build graph JSON string — sync wrapper.
pub fn rebuild_graph_json() -> io::Result<String> {
    let graph = build_graph_sync(true)?;
    serde_json::to_string_pretty(&graph).map_err(io::Error::other)
}

/// Write the graph JSON to the graph file on disk.
pub fn rebuild_graph() -> io::Result<()> {
    let graph = build_graph_sync(true)?;
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

    // ── Virtual edge tests ──────────────────────────────────────

    fn make_node(id: &str, tags: Vec<&str>, projects: Vec<&str>) -> GraphNode {
        GraphNode {
            id: id.to_string(),
            title: id.to_string(),
            node_type: "concept".to_string(),
            tags: tags.into_iter().map(|t| t.to_string()).collect(),
            importance: 0.5,
            projects: projects.into_iter().map(|p| p.to_string()).collect(),
            accessed_at: String::new(),
        }
    }

    #[test]
    fn generate_virtual_edges_cross_project_only() {
        let nodes = vec![
            make_node("a", vec!["rust"], vec!["proj1"]),
            make_node("b", vec!["rust"], vec!["proj2"]),
            make_node("c", vec!["rust"], vec!["proj1"]),
        ];
        let edges = generate_virtual_edges(&nodes);
        // a-b: cross-project (proj1 vs proj2) -> virtual edge
        assert!(edges.iter().any(|e| e.source == "a" && e.target == "b"));
        // a-c: same project (proj1) -> no edge
        assert!(
            !edges
                .iter()
                .any(|e| (e.source == "a" && e.target == "c")
                    || (e.source == "c" && e.target == "a"))
        );
    }

    #[test]
    fn generate_virtual_edges_weight_jaccard() {
        let nodes = vec![
            make_node("a", vec!["x", "y"], vec!["p1"]),
            make_node("b", vec!["x", "y", "z"], vec!["p2"]),
            make_node("c", vec!["x"], vec!["p3"]),
        ];
        let edges = generate_virtual_edges(&nodes);
        let ab = edges
            .iter()
            .find(|e| e.source == "a" && e.target == "b")
            .unwrap();
        let ac = edges
            .iter()
            .find(|e| e.source == "a" && e.target == "c")
            .unwrap();
        // Jaccard(a,b) = 2/3 ≈ 0.67, Jaccard(a,c) = 1/2 = 0.5
        assert!(
            ab.weight > ac.weight,
            "ab weight ({}) should be > ac weight ({})",
            ab.weight,
            ac.weight
        );
    }

    #[test]
    fn generate_virtual_edges_no_tags() {
        let nodes = vec![
            make_node("a", vec![], vec!["p1"]),
            make_node("b", vec![], vec!["p2"]),
        ];
        let edges = generate_virtual_edges(&nodes);
        assert!(edges.is_empty(), "no tags -> no virtual edges");
    }

    #[test]
    fn generate_virtual_edges_dedup() {
        let nodes = vec![
            make_node("a", vec!["x", "y", "z"], vec!["p1"]),
            make_node("b", vec!["x", "y", "z"], vec!["p2"]),
        ];
        let edges = generate_virtual_edges(&nodes);
        // Only one edge between a and b despite 3 shared tags
        let count = edges
            .iter()
            .filter(|e| {
                (e.source == "a" && e.target == "b") || (e.source == "b" && e.target == "a")
            })
            .count();
        assert_eq!(count, 1, "should have exactly 1 edge between a and b");
    }

    #[test]
    fn graph_edge_virtual_field_serialization() {
        let persisted = GraphEdge {
            source: "a".to_string(),
            target: "b".to_string(),
            relation: "related".to_string(),
            weight: 1.0,
            virtual_: false,
        };
        let json = serde_json::to_string(&persisted).unwrap();
        assert!(
            !json.contains("virtual"),
            "persisted edge should not have virtual field: {json}"
        );

        let virt = GraphEdge {
            source: "a".to_string(),
            target: "b".to_string(),
            relation: "shared_tag".to_string(),
            weight: 0.5,
            virtual_: true,
        };
        let vjson = serde_json::to_string(&virt).unwrap();
        assert!(
            vjson.contains("\"virtual\":true"),
            "virtual edge should have virtual: true: {vjson}"
        );
    }
}
