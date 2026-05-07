//! graph.rs — Graph build + traversal (related, rebuild)

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io;

use rusqlite::{Connection, params_from_iter};

use super::store::{
    atomic_write, graph_path, list_node_ids, open_db, read_edges_conn, read_nodes_conn,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub tags: Vec<String>,
    #[serde(default = "default_graph_importance")]
    pub importance: f64,
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
/// Prevents unbounded memory growth on dense graphs.
const MAX_GRAPH_EDGES: usize = 2000;

/// Build a `Graph` value from an existing connection.
/// Reuses the caller's connection — no additional `open_db` call.
fn build_graph_conn(conn: &Connection) -> io::Result<Graph> {
    let ids = list_node_ids()?; // list_node_ids opens its own connection — that's OK
    let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let nodes = read_nodes_conn(conn, &id_refs)
        .into_iter()
        .map(|node| GraphNode {
            id: node.frontmatter.id,
            title: node.frontmatter.title,
            node_type: node.frontmatter.node_type,
            tags: node.frontmatter.tags,
            importance: node.frontmatter.importance,
        })
        .collect();
    let edges = read_edges_conn(conn)
        .into_iter()
        .take(MAX_GRAPH_EDGES) // LIMIT 2000
        .map(|e| GraphEdge {
            source: e.source,
            target: e.target,
            relation: e.relation,
            weight: e.weight,
        })
        .collect();
    Ok(Graph { nodes, edges })
}

/// Build a `Graph` value from the current DB state.
/// Opens a fresh connection and delegates to `build_graph_conn`.
fn build_graph() -> io::Result<Graph> {
    let conn = open_db()?;
    build_graph_conn(&conn)
}

/// Build graph JSON string from an existing connection (no open_db call).
/// Used by the web server to always return fresh data via its shared connection.
pub fn rebuild_graph_json_conn(conn: &Connection) -> io::Result<String> {
    serde_json::to_string_pretty(&build_graph_conn(conn)?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn rebuild_graph() -> io::Result<()> {
    let data = serde_json::to_vec_pretty(&build_graph()?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    atomic_write(&graph_path(), &data)
}

/// Build graph JSON string directly from DB (no file I/O).
/// Used by the web server to always return fresh data.
#[allow(dead_code)]
pub fn rebuild_graph_json() -> io::Result<String> {
    serde_json::to_string_pretty(&build_graph()?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Get 1-hop neighbors using an existing connection.
/// Returns `(neighbor_id, total_weight)` sorted by weight descending.
/// Maximum seeds accepted per call — keeps `WHERE IN (?, ...)` well below
/// SQLite's `SQLITE_LIMIT_VARIABLE_NUMBER` default of 999 (2 copies are bound).
const MAX_SEED_IDS: usize = 100;

pub fn graph_neighbors_conn(conn: &Connection, seed_ids: &[String]) -> Vec<(String, f64)> {
    if seed_ids.is_empty() {
        return vec![];
    }
    let seed_ids = if seed_ids.len() > MAX_SEED_IDS {
        eprintln!(
            "[mem/graph] graph_neighbors_conn: seed_ids.len()={} exceeds MAX_SEED_IDS={}, truncating",
            seed_ids.len(),
            MAX_SEED_IDS
        );
        &seed_ids[..MAX_SEED_IDS]
    } else {
        seed_ids
    };

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

/// BFS traversal using an existing connection.
///
/// `depth` is accepted for API compatibility but is not used as a hop limit.
/// The single-column `UNION` CTE deduplicates on `node_id` alone, so each node
/// enters the working set at most once. This prevents re-expansion of visited
/// nodes in cyclic or dense graphs. Results are capped at 500.
pub fn related_nodes_conn(conn: &Connection, start_id: &str, _depth: usize) -> Vec<String> {
    let sql = "
        WITH RECURSIVE bfs(node_id) AS (
            SELECT target FROM edges WHERE source = ?1
            UNION SELECT source FROM edges WHERE target = ?1
            UNION SELECT e.target FROM edges e JOIN bfs ON e.source = bfs.node_id WHERE e.target != ?1
            UNION SELECT e.source FROM edges e JOIN bfs ON e.target = bfs.node_id WHERE e.source != ?1
        )
        SELECT node_id FROM bfs
        LIMIT 500
    ";

    conn.prepare(sql)
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params![start_id], |row| row.get::<_, String>(0))
                .map(|rows| rows.flatten().collect())
        })
        .unwrap_or_default()
}

/// Compute aggregate stats for the `/api/stats` endpoint.
pub fn compute_stats() -> io::Result<serde_json::Value> {
    let conn = open_db()?;
    let total_nodes: i64 = conn
        .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
        .unwrap_or(0);
    let total_edges: i64 = conn
        .query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0))
        .unwrap_or(0);
    let avg_importance: f64 = conn
        .query_row("SELECT AVG(importance) FROM nodes", [], |r| r.get(0))
        .unwrap_or(0.0);

    let mut stmt = conn
        .prepare("SELECT type, COUNT(*) FROM nodes GROUP BY type")
        .map_err(io::Error::other)?;
    let by_type: serde_json::Map<String, serde_json::Value> = stmt
        .query_map([], |row| {
            let t: String = row.get(0)?;
            let c: i64 = row.get(1)?;
            Ok((t, serde_json::Value::from(c)))
        })
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default();

    Ok(serde_json::json!({
        "total_nodes": total_nodes,
        "total_edges": total_edges,
        "avg_importance": (avg_importance * 100.0).round() / 100.0,
        "by_type": serde_json::Value::Object(by_type),
    }))
}

/// BFS traversal from `start_id` to all reachable nodes via a SQL recursive CTE.
///
/// Uses `idx_edges_source` / `idx_edges_target` on each recursive step so only
/// reachable edges are touched — O(reachable_edges) instead of O(E) total.
/// Single-column `UNION` deduplicates on `node_id` alone, so each node enters
/// the working set exactly once — preventing re-expansion in cyclic or dense
/// graphs. The `depth` argument is kept for API compatibility but is ignored;
/// traversal reaches all reachable nodes up to the LIMIT 500 safety cap.
pub fn related_nodes(start_id: &str, depth: usize) -> Vec<String> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[mem/graph] related_nodes: open_db failed: {e}");
            return vec![];
        }
    };
    related_nodes_conn(&conn, start_id, depth)
}

#[cfg(test)]
mod tests {
    use super::super::store::{Edge, append_edge_conn, init_schema};
    use super::*;
    use rusqlite::Connection;

    /// Open a fresh in-memory SQLite DB with the full schema applied.
    /// Each call returns an independent connection — no shared state, no env var mutation.
    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        init_schema(&conn).expect("init_schema");
        conn
    }

    fn insert_edge(conn: &Connection, id: &str, src: &str, tgt: &str) {
        let e = Edge {
            id: id.to_string(),
            source: src.to_string(),
            target: tgt.to_string(),
            relation: "related".to_string(),
            weight: 1.0,
            ts: "2026-01-01T00:00:00Z".to_string(),
        };
        append_edge_conn(conn, &e).expect("append_edge_conn");
    }

    /// graph_neighbors returns 1-hop neighbors including backward edges.
    #[test]
    fn graph_neighbors_returns_direct_neighbors() {
        let conn = mem_db();

        // A -> B, A -> C, D -> A (backward edge)
        insert_edge(&conn, "e1", "A", "B");
        insert_edge(&conn, "e2", "A", "C");
        insert_edge(&conn, "e3", "D", "A");

        let seeds = vec!["A".to_string()];
        let mut result = graph_neighbors_conn(&conn, &seeds);
        result.sort_by(|a, b| a.0.cmp(&b.0));

        let ids: Vec<&str> = result.iter().map(|r| r.0.as_str()).collect();
        assert!(ids.contains(&"B"), "B should be a neighbor of A");
        assert!(ids.contains(&"C"), "C should be a neighbor of A");
        assert!(
            ids.contains(&"D"),
            "D should be a neighbor of A (backward edge)"
        );
        assert!(!ids.contains(&"A"), "seed A must not appear in results");
    }

    /// graph_neighbors excludes all seeds from the result set.
    #[test]
    fn graph_neighbors_excludes_seeds() {
        let conn = mem_db();

        // Seed A -> Seed B -> C
        insert_edge(&conn, "e1", "A", "B");
        insert_edge(&conn, "e2", "B", "C");

        let seeds = vec!["A".to_string(), "B".to_string()];
        let result = graph_neighbors_conn(&conn, &seeds);
        let ids: Vec<&str> = result.iter().map(|r| r.0.as_str()).collect();

        assert!(ids.contains(&"C"), "C should be reachable from B");
        assert!(!ids.contains(&"A"), "seed A must be excluded");
        assert!(!ids.contains(&"B"), "seed B must be excluded");
    }

    /// graph_neighbors with empty seed list returns empty.
    #[test]
    fn graph_neighbors_empty_seeds() {
        let conn = mem_db();
        let result = graph_neighbors_conn(&conn, &[]);
        assert!(result.is_empty(), "empty seeds -> empty result");
    }

    /// related_nodes uses a single-column recursive CTE that traverses all
    /// reachable nodes. Each node appears at most once (UNION deduplicates on
    /// node_id alone). Cycles must not produce duplicate results.
    #[test]
    fn related_nodes_recursive_cte() {
        let conn = mem_db();

        // Chain: A -> B -> C
        insert_edge(&conn, "e1", "A", "B");
        insert_edge(&conn, "e2", "B", "C");

        // All reachable nodes from A should include B and C; start node excluded.
        let result = related_nodes_conn(&conn, "A", 2);
        assert!(result.contains(&"B".to_string()), "should reach B");
        assert!(result.contains(&"C".to_string()), "should reach C (2-hop)");
        assert!(
            !result.contains(&"A".to_string()),
            "start node must not appear"
        );

        // Cycle: C -> A — results must still be deduplicated (no duplicates).
        insert_edge(&conn, "e3", "C", "A");
        let cyclic = related_nodes_conn(&conn, "A", 3);
        let unique: HashSet<_> = cyclic.iter().collect();
        assert_eq!(
            cyclic.len(),
            unique.len(),
            "no duplicate nodes in cyclic graph"
        );
        assert!(
            !cyclic.contains(&"A".to_string()),
            "start node must not appear even in cycle"
        );
    }

    /// graph_neighbors returns weight sums (both seeds connect to C with weight 1.0 each → 2.0).
    #[test]
    fn graph_neighbors_connection_count() {
        let conn = mem_db();

        // Both seeds A and B connect to C (default weight 1.0 each → total 2.0).
        insert_edge(&conn, "e1", "A", "C");
        insert_edge(&conn, "e2", "B", "C");

        let seeds = vec!["A".to_string(), "B".to_string()];
        let result = graph_neighbors_conn(&conn, &seeds);
        let c_weight = result.iter().find(|(id, _)| id == "C").map(|(_, w)| *w);
        assert_eq!(
            c_weight,
            Some(2.0),
            "C connected to both seeds should have total weight 2.0"
        );
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
        };
        let json = serde_json::to_string(&n).unwrap();
        assert!(
            json.contains("\"importance\":0.85"),
            "importance should appear in JSON: {json}"
        );
        // Deserialize back
        let parsed: GraphNode = serde_json::from_str(&json).unwrap();
        assert!((parsed.importance - 0.85).abs() < f64::EPSILON);
    }
}
