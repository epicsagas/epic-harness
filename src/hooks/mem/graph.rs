//! graph.rs — Graph build + traversal (related, rebuild)

use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::io;

use rusqlite::params_from_iter;

use super::store::{atomic_write, graph_path, list_node_ids, open_db, read_edges, read_node};

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
///
/// Uses targeted `idx_edges_source` / `idx_edges_target` index lookups — O(log N + degree)
/// per seed — instead of loading all edges and filtering in Rust.
pub fn graph_neighbors(seed_ids: &[String]) -> Vec<(String, usize)> {
    if seed_ids.is_empty() {
        return vec![];
    }

    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    // Build "?,?,..." placeholder string for the IN clause.
    // The same list is used twice: once for forward edges (source IN seeds)
    // and once for backward edges (target IN seeds).
    let ph: String = seed_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT target FROM edges WHERE source IN ({ph}) \
         UNION ALL \
         SELECT source FROM edges WHERE target IN ({ph})"
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    // Bind seed_ids twice: first for forward edges, then for backward edges.
    let neighbor_ids: Vec<String> = stmt
        .query_map(
            params_from_iter(seed_ids.iter().chain(seed_ids.iter())),
            |row| row.get(0),
        )
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default();

    // Count occurrences per neighbor (= connection strength).
    // Exclude nodes that are themselves seeds.
    let seed_set: HashSet<&str> = seed_ids.iter().map(String::as_str).collect();
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for nid in neighbor_ids {
        if !seed_set.contains(nid.as_str()) {
            *counts.entry(nid).or_default() += 1;
        }
    }

    let mut result: Vec<(String, usize)> = counts.into_iter().collect();
    result.sort_by(|a, b| b.1.cmp(&a.1)); // most connected first
    result
}

/// BFS traversal from `start_id` up to `depth` hops using the DB edges table.
/// NOTE(perf): For depth <= 3 the full-table-scan approach is acceptable because
/// `related_nodes` is called once per `mem_related` MCP request, not on every
/// `mem_recall`. A per-hop targeted IN query would reduce I/O further but adds
/// complexity; revisit if graph size exceeds ~10k edges.
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::store::{append_edge, Edge};
    use std::env;
    use std::sync::Mutex;

    // Serialize env mutation across all tests in this module.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Set HARNESS_ROOT to a per-test temp dir so tests don't pollute the real DB.
    fn setup_temp_db() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().expect("tmp dir");
        // SAFETY: guarded by ENV_LOCK; no concurrent env reads within this module.
        unsafe { env::set_var("HARNESS_ROOT", dir.path().to_str().unwrap()) };
        // Open DB once to initialise schema.
        let _ = open_db().expect("open_db in setup");
        dir
    }

    fn insert_edge(id: &str, src: &str, tgt: &str) {
        let e = Edge {
            id: id.to_string(),
            source: src.to_string(),
            target: tgt.to_string(),
            relation: "related".to_string(),
            weight: 1.0,
            ts: "2026-01-01T00:00:00Z".to_string(),
        };
        append_edge(&e).expect("append_edge");
    }

    /// graph_neighbors returns 1-hop neighbors including backward edges.
    #[test]
    fn graph_neighbors_returns_direct_neighbors() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _dir = setup_temp_db();

        // A -> B, A -> C, D -> A (backward edge)
        insert_edge("e1", "A", "B");
        insert_edge("e2", "A", "C");
        insert_edge("e3", "D", "A");

        let seeds = vec!["A".to_string()];
        let mut result = graph_neighbors(&seeds);
        result.sort_by(|a, b| a.0.cmp(&b.0));

        let ids: Vec<&str> = result.iter().map(|r| r.0.as_str()).collect();
        assert!(ids.contains(&"B"), "B should be a neighbor of A");
        assert!(ids.contains(&"C"), "C should be a neighbor of A");
        assert!(ids.contains(&"D"), "D should be a neighbor of A (backward edge)");
        assert!(!ids.contains(&"A"), "seed A must not appear in results");
    }

    /// graph_neighbors excludes all seeds from the result set.
    #[test]
    fn graph_neighbors_excludes_seeds() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _dir = setup_temp_db();

        // Seed A -> Seed B -> C
        insert_edge("e1", "A", "B");
        insert_edge("e2", "B", "C");

        let seeds = vec!["A".to_string(), "B".to_string()];
        let result = graph_neighbors(&seeds);
        let ids: Vec<&str> = result.iter().map(|r| r.0.as_str()).collect();

        assert!(ids.contains(&"C"), "C should be reachable from B");
        assert!(!ids.contains(&"A"), "seed A must be excluded");
        assert!(!ids.contains(&"B"), "seed B must be excluded");
    }

    /// graph_neighbors with empty seed list returns empty.
    #[test]
    fn graph_neighbors_empty_seeds() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _dir = setup_temp_db();
        let result = graph_neighbors(&[]);
        assert!(result.is_empty(), "empty seeds -> empty result");
    }

    /// graph_neighbors counts connection strength (how many seeds link to the neighbor).
    #[test]
    fn graph_neighbors_connection_count() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _dir = setup_temp_db();

        // Both seeds A and B connect to C — C should have count 2.
        insert_edge("e1", "A", "C");
        insert_edge("e2", "B", "C");

        let seeds = vec!["A".to_string(), "B".to_string()];
        let result = graph_neighbors(&seeds);
        let c_count = result.iter().find(|(id, _)| id == "C").map(|(_, n)| *n);
        assert_eq!(c_count, Some(2), "C connected to both seeds should have count 2");
    }
}
