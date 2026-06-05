//! graph.rs — Graph build + traversal (related, rebuild)

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;
use std::io;

use super::store::{list_node_ids_pool, read_edges_pool, read_nodes_pool};
use crate::store::pool::memory_pool;
use crate::store::runtime::block_on;

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

/// Build a `Graph` value using a sqlx pool.
pub async fn build_graph_pool(pool: &SqlitePool) -> io::Result<Graph> {
    let ids = list_node_ids_pool(pool).await?;
    let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let nodes = read_nodes_pool(pool, &id_refs)
        .await?
        .into_iter()
        .map(|node| GraphNode {
            id: node.frontmatter.id,
            title: node.frontmatter.title,
            node_type: node.frontmatter.node_type,
            tags: node.frontmatter.tags,
            importance: node.frontmatter.importance,
        })
        .collect();
    let edges = read_edges_pool(pool, MAX_GRAPH_EDGES as i64)
        .await?
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

/// Build graph JSON string using a sqlx pool.
pub async fn rebuild_graph_json_pool(pool: &SqlitePool) -> io::Result<String> {
    serde_json::to_string_pretty(&build_graph_pool(pool).await?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Build graph JSON string — sync wrapper that acquires pool internally.
/// Used by server.rs and other sync callers for the `/api/graph` endpoint.
pub fn rebuild_graph_json() -> io::Result<String> {
    block_on(async {
        let pool = memory_pool().await?;
        rebuild_graph_json_pool(&pool).await
    })
}

/// Write the graph JSON to the graph file on disk.
pub fn rebuild_graph() -> io::Result<()> {
    block_on(async {
        let pool = memory_pool().await?;
        let graph = build_graph_pool(&pool).await?;
        let data = serde_json::to_vec_pretty(&graph)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        use super::store::atomic_write;
        use super::store::graph_path;
        atomic_write(&graph_path(), &data)
    })
}

/// Maximum seeds accepted per call — keeps `WHERE IN (?, ...)` well below
/// SQLite's `SQLITE_LIMIT_VARIABLE_NUMBER` default of 999 (2 copies are bound).
const MAX_SEED_IDS: usize = 100;

/// Async 1-hop neighbors using QueryBuilder for the IN clause.
pub async fn graph_neighbors_pool(pool: &SqlitePool, seed_ids: &[String]) -> Vec<(String, f64)> {
    if seed_ids.is_empty() {
        return vec![];
    }
    let seed_ids = if seed_ids.len() > MAX_SEED_IDS {
        eprintln!(
            "[mem/graph] graph_neighbors_pool: seed_ids.len()={} exceeds MAX_SEED_IDS={}, truncating",
            seed_ids.len(),
            MAX_SEED_IDS
        );
        &seed_ids[..MAX_SEED_IDS]
    } else {
        seed_ids
    };

    let mut qb = sqlx::QueryBuilder::new(
        "SELECT target AS nb, SUM(weight) AS w FROM edges WHERE source IN (",
    );
    let mut separated = qb.separated(", ");
    for id in seed_ids {
        separated.push_bind(id);
    }
    qb.push(") GROUP BY target UNION ALL SELECT source AS nb, SUM(weight) AS w FROM edges WHERE target IN (");
    let mut separated2 = qb.separated(", ");
    for id in seed_ids {
        separated2.push_bind(id);
    }
    qb.push(") GROUP BY source");

    let rows = match qb.build().fetch_all(pool).await {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    let raw: Vec<(String, f64)> = rows
        .iter()
        .filter_map(|r| {
            let nid: String = r.try_get(0).ok()?;
            let w: f64 = r.try_get(1).ok()?;
            Some((nid, w))
        })
        .collect();

    let seed_set: HashSet<&str> = seed_ids.iter().map(String::as_str).collect();
    let mut weights: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for (nid, w) in raw {
        if !seed_set.contains(nid.as_str()) {
            *weights.entry(nid).or_default() += w;
        }
    }

    let mut result: Vec<(String, f64)> = weights.into_iter().collect();
    result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    result
}

/// Get 1-hop neighbors — sync wrapper that acquires pool internally.
#[allow(dead_code)]
pub fn graph_neighbors(seed_ids: &[String]) -> Vec<(String, f64)> {
    block_on(async {
        let pool = memory_pool().await.unwrap_or_else(|e| {
            eprintln!("[mem/graph] graph_neighbors: pool failed: {e}");
            panic!("memory_pool unavailable");
        });
        graph_neighbors_pool(&pool, seed_ids).await
    })
}

/// Async BFS traversal using a sqlx pool.
pub async fn related_nodes_pool(pool: &SqlitePool, start_id: &str, _depth: usize) -> Vec<String> {
    let sql = "
        WITH RECURSIVE bfs(node_id) AS (
            SELECT target FROM edges WHERE source = ?
            UNION SELECT source FROM edges WHERE target = ?
            UNION SELECT e.target FROM edges e JOIN bfs ON e.source = bfs.node_id WHERE e.target != ?
            UNION SELECT e.source FROM edges e JOIN bfs ON e.target = bfs.node_id WHERE e.source != ?
        )
        SELECT node_id FROM bfs
        LIMIT 500
    ";
    sqlx::query(sql)
        .bind(start_id)
        .bind(start_id)
        .bind(start_id)
        .bind(start_id)
        .fetch_all(pool)
        .await
        .map(|rows| {
            rows.iter()
                .filter_map(|r| r.try_get::<String, _>(0).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// BFS traversal from `start_id` — sync wrapper that acquires pool internally.
pub fn related_nodes(start_id: &str, depth: usize) -> Vec<String> {
    block_on(async {
        let pool = match memory_pool().await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[mem/graph] related_nodes: pool failed: {e}");
                return vec![];
            }
        };
        related_nodes_pool(&pool, start_id, depth).await
    })
}

/// Async compute aggregate stats using a sqlx pool.
pub async fn compute_stats_pool(pool: &SqlitePool) -> io::Result<serde_json::Value> {
    let total_nodes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM nodes")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let total_edges: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM edges")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let avg_importance: f64 = sqlx::query_scalar("SELECT AVG(importance) FROM nodes")
        .fetch_one(pool)
        .await
        .unwrap_or(0.0);

    let rows = sqlx::query("SELECT type, COUNT(*) FROM nodes GROUP BY type")
        .fetch_all(pool)
        .await
        .map_err(crate::store::sqlx_err)?;

    let by_type: serde_json::Map<String, serde_json::Value> = rows
        .iter()
        .filter_map(|r| {
            let t: String = r.try_get(0).ok()?;
            let c: i64 = r.try_get(1).ok()?;
            Some((t, serde_json::Value::from(c)))
        })
        .collect();

    Ok(serde_json::json!({
        "total_nodes": total_nodes,
        "total_edges": total_edges,
        "avg_importance": (avg_importance * 100.0).round() / 100.0,
        "by_type": serde_json::Value::Object(by_type),
    }))
}

/// Compute aggregate stats — sync wrapper that acquires pool internally.
pub fn compute_stats() -> io::Result<serde_json::Value> {
    block_on(async {
        let pool = memory_pool().await?;
        compute_stats_pool(&pool).await
    })
}

#[cfg(test)]
mod tests {
    use super::super::store::{Edge, append_edge_pool, init_schema_pool};
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Open a fresh in-memory SQLite pool with the full schema applied.
    async fn mem_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory pool");
        init_schema_pool(&pool).await.expect("init_schema_pool");
        pool
    }

    async fn insert_edge(pool: &SqlitePool, id: &str, src: &str, tgt: &str) {
        let e = Edge {
            id: id.to_string(),
            source: src.to_string(),
            target: tgt.to_string(),
            relation: "related".to_string(),
            weight: 1.0,
            ts: "2026-01-01T00:00:00Z".to_string(),
        };
        append_edge_pool(pool, &e).await.expect("append_edge_pool");
    }

    /// graph_neighbors returns 1-hop neighbors including backward edges.
    #[tokio::test]
    async fn graph_neighbors_returns_direct_neighbors() {
        let pool = mem_pool().await;

        // A -> B, A -> C, D -> A (backward edge)
        insert_edge(&pool, "e1", "A", "B").await;
        insert_edge(&pool, "e2", "A", "C").await;
        insert_edge(&pool, "e3", "D", "A").await;

        let seeds = vec!["A".to_string()];
        let mut result = graph_neighbors_pool(&pool, &seeds).await;
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
    #[tokio::test]
    async fn graph_neighbors_excludes_seeds() {
        let pool = mem_pool().await;

        // Seed A -> Seed B -> C
        insert_edge(&pool, "e1", "A", "B").await;
        insert_edge(&pool, "e2", "B", "C").await;

        let seeds = vec!["A".to_string(), "B".to_string()];
        let result = graph_neighbors_pool(&pool, &seeds).await;
        let ids: Vec<&str> = result.iter().map(|r| r.0.as_str()).collect();

        assert!(ids.contains(&"C"), "C should be reachable from B");
        assert!(!ids.contains(&"A"), "seed A must be excluded");
        assert!(!ids.contains(&"B"), "seed B must be excluded");
    }

    /// graph_neighbors with empty seed list returns empty.
    #[tokio::test]
    async fn graph_neighbors_empty_seeds() {
        let pool = mem_pool().await;
        let result = graph_neighbors_pool(&pool, &[]).await;
        assert!(result.is_empty(), "empty seeds -> empty result");
    }

    /// related_nodes uses a single-column recursive CTE that traverses all
    /// reachable nodes. Each node appears at most once (UNION deduplicates on
    /// node_id alone). Cycles must not produce duplicate results.
    #[tokio::test]
    async fn related_nodes_recursive_cte() {
        let pool = mem_pool().await;

        // Chain: A -> B -> C
        insert_edge(&pool, "e1", "A", "B").await;
        insert_edge(&pool, "e2", "B", "C").await;

        // All reachable nodes from A should include B and C; start node excluded.
        let result = related_nodes_pool(&pool, "A", 2).await;
        assert!(result.contains(&"B".to_string()), "should reach B");
        assert!(result.contains(&"C".to_string()), "should reach C (2-hop)");
        assert!(
            !result.contains(&"A".to_string()),
            "start node must not appear"
        );

        // Cycle: C -> A — results must still be deduplicated (no duplicates).
        insert_edge(&pool, "e3", "C", "A").await;
        let cyclic = related_nodes_pool(&pool, "A", 3).await;
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
    #[tokio::test]
    async fn graph_neighbors_connection_count() {
        let pool = mem_pool().await;

        // Both seeds A and B connect to C (default weight 1.0 each → total 2.0).
        insert_edge(&pool, "e1", "A", "C").await;
        insert_edge(&pool, "e2", "B", "C").await;

        let seeds = vec!["A".to_string(), "B".to_string()];
        let result = graph_neighbors_pool(&pool, &seeds).await;
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
