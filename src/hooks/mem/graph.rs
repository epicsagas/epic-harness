//! graph.rs — Graph build + traversal (related, rebuild)

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io;

use rusqlite::{Connection, params_from_iter};

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

/// Get 1-hop neighbors using an existing connection.
/// Returns `(neighbor_id, total_weight)` sorted by weight descending.
pub fn graph_neighbors_conn(conn: &Connection, seed_ids: &[String]) -> Vec<(String, f64)> {
    if seed_ids.is_empty() {
        return vec![];
    }

    let ph: String = seed_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    // Sum weights per neighbor from both forward and backward edges.
    let sql = format!(
        "SELECT target AS nb, SUM(weight) AS w FROM edges WHERE source IN ({ph}) GROUP BY target \
         UNION ALL \
         SELECT source AS nb, SUM(weight) AS w FROM edges WHERE target IN ({ph}) GROUP BY source"
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    let rows: Vec<(String, f64)> = stmt
        .query_map(
            params_from_iter(seed_ids.iter().chain(seed_ids.iter())),
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)),
        )
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default();

    // Accumulate weights and exclude seeds.
    let seed_set: HashSet<&str> = seed_ids.iter().map(String::as_str).collect();
    let mut weights: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for (nid, w) in rows {
        if !seed_set.contains(nid.as_str()) {
            *weights.entry(nid).or_default() += w;
        }
    }

    let mut result: Vec<(String, f64)> = weights.into_iter().collect();
    result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    result
}

/// Get 1-hop neighbors for multiple seed nodes, excluding the seeds themselves.
/// Returns `(neighbor_id, total_weight)` — sum of edge weights to any seed node.
/// Sorted by weight descending (strongest connections first).
///
/// Uses targeted `idx_edges_source` / `idx_edges_target` index lookups — O(log N + degree).
pub fn graph_neighbors(seed_ids: &[String]) -> Vec<(String, f64)> {
    if seed_ids.is_empty() {
        return vec![];
    }
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    graph_neighbors_conn(&conn, seed_ids)
}

/// BFS traversal using an existing connection.
pub fn related_nodes_conn(conn: &Connection, start_id: &str, depth: usize) -> Vec<String> {
    let sql = "
        WITH RECURSIVE bfs(node_id, depth) AS (
            SELECT target, 1 FROM edges WHERE source = ?1
            UNION
            SELECT source, 1 FROM edges WHERE target = ?1
            UNION
            SELECT e.target, bfs.depth + 1
              FROM edges e JOIN bfs ON e.source = bfs.node_id
             WHERE bfs.depth < ?2
            UNION
            SELECT e.source, bfs.depth + 1
              FROM edges e JOIN bfs ON e.target = bfs.node_id
             WHERE bfs.depth < ?2
        )
        SELECT DISTINCT node_id FROM bfs WHERE node_id != ?1
        LIMIT 500
    ";

    conn.prepare(sql)
        .and_then(|mut stmt| {
            stmt.query_map(
                rusqlite::params![start_id, depth as i64],
                |row| row.get::<_, String>(0),
            )
            .map(|rows| rows.flatten().collect())
        })
        .unwrap_or_default()
}

/// BFS traversal from `start_id` up to `depth` hops via a SQL recursive CTE.
///
/// Uses `idx_edges_source` / `idx_edges_target` on each recursive step so only
/// reachable edges are touched — O(reachable_edges) instead of O(E) total.
/// UNION (not UNION ALL) deduplicates visited nodes, preventing re-visits in
/// cyclic graphs. Results are capped at 500.
pub fn related_nodes(start_id: &str, depth: usize) -> Vec<String> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    related_nodes_conn(&conn, start_id, depth)
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

    /// related_nodes uses recursive CTE: depth=1 returns direct neighbors only,
    /// depth=2 returns 2-hop nodes, and cycles are deduplicated.
    #[test]
    fn related_nodes_recursive_cte() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _dir = setup_temp_db();

        // Chain: A -> B -> C
        insert_edge("e1", "A", "B");
        insert_edge("e2", "B", "C");

        // Depth 1: only B
        let d1 = related_nodes("A", 1);
        assert!(d1.contains(&"B".to_string()), "depth 1 should reach B");
        assert!(!d1.contains(&"C".to_string()), "depth 1 should NOT reach C");

        // Depth 2: B and C
        let d2 = related_nodes("A", 2);
        assert!(d2.contains(&"B".to_string()), "depth 2 should reach B");
        assert!(d2.contains(&"C".to_string()), "depth 2 should reach C");
        assert!(!d2.contains(&"A".to_string()), "start node must not appear");

        // Cycle: C -> A — depth 3 should still deduplicate (no duplicates)
        insert_edge("e3", "C", "A");
        let d3 = related_nodes("A", 3);
        let unique: HashSet<_> = d3.iter().collect();
        assert_eq!(d3.len(), unique.len(), "no duplicate nodes in cyclic graph");
    }

    /// graph_neighbors returns weight sums (both seeds connect to C with weight 1.0 each → 2.0).
    #[test]
    fn graph_neighbors_connection_count() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _dir = setup_temp_db();

        // Both seeds A and B connect to C (default weight 1.0 each → total 2.0).
        insert_edge("e1", "A", "C");
        insert_edge("e2", "B", "C");

        let seeds = vec!["A".to_string(), "B".to_string()];
        let result = graph_neighbors(&seeds);
        let c_weight = result.iter().find(|(id, _)| id == "C").map(|(_, w)| *w);
        assert_eq!(c_weight, Some(2.0), "C connected to both seeds should have total weight 2.0");
    }
}
