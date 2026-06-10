//! edge.rs — Edge CRUD operations via sqlx

use std::io;

use sqlx::Row;

use super::conn::memory_pool_sync;
use super::schema::append_graph_edge;
use super::types::Edge;
use super::types::edge_to_graph;
use crate::store::runtime;

pub fn append_edge(edge: &Edge) -> io::Result<()> {
    let pool = memory_pool_sync()?;
    let ge = edge_to_graph(edge);
    runtime::block_on(append_graph_edge(&pool, &ge))
}

#[allow(dead_code)]
pub fn read_edges() -> Vec<Edge> {
    read_edges_limit(5000)
}

pub fn read_edges_limit(limit: usize) -> Vec<Edge> {
    let pool = match memory_pool_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[mem/store] read_edges: {e}");
            return vec![];
        }
    };
    runtime::block_on(async {
        sqlx::query("SELECT source, target, label, created FROM edges ORDER BY created DESC LIMIT ?")
            .bind(limit as i64)
            .fetch_all(&pool)
            .await
            .map(|rows| {
                rows.iter()
                    .map(|r| Edge {
                        id: String::new(), // edges table doesn't store id
                        source: r.get::<String, _>(0),
                        target: r.get::<String, _>(1),
                        relation: r.get::<String, _>(2),
                        weight: 1.0, // edges table doesn't store weight
                        ts: r.get::<String, _>(3),
                    })
                    .collect()
            })
            .unwrap_or_default()
    })
}

pub fn delete_edge_by_id(edge_id: &str) -> io::Result<()> {
    let pool = memory_pool_sync()?;
    runtime::block_on(async {
        // The edges table PK is (source, target, label), not a single id.
        // For the simple delete-by-id API, we try deleting by source=target=edge_id
        // or by source/target match. In practice, callers use remove_edges_for_node.
        // For a single edge identified by a label/id:
        sqlx::query("DELETE FROM edges WHERE source = ? OR target = ? OR label = ?")
            .bind(edge_id)
            .bind(edge_id)
            .bind(edge_id)
            .execute(&pool)
            .await
            .map_err(io::Error::other)?;
        Ok(())
    })
}

pub fn remove_edges_for_node(node_id: &str) -> io::Result<()> {
    let pool = memory_pool_sync()?;
    runtime::block_on(async {
        sqlx::query("DELETE FROM edges WHERE source = ? OR target = ?")
            .bind(node_id)
            .bind(node_id)
            .execute(&pool)
            .await
            .map_err(io::Error::other)?;
        Ok(())
    })
}

// ── Pool-compatible async wrappers ────────────────────────────

#[allow(dead_code)]
pub async fn append_edge_pool(_pool: &sqlx::AnyPool, edge: &Edge) -> io::Result<()> {
    append_edge(edge)
}

#[allow(dead_code)]
pub async fn read_edges_pool(_pool: &sqlx::AnyPool, limit: i64) -> io::Result<Vec<Edge>> {
    Ok(read_edges_limit(limit as usize))
}

#[allow(dead_code)]
pub async fn delete_edge_by_id_pool(_pool: &sqlx::AnyPool, edge_id: &str) -> io::Result<()> {
    delete_edge_by_id(edge_id)
}

#[allow(dead_code)]
pub async fn remove_edges_for_node_pool(_pool: &sqlx::AnyPool, node_id: &str) -> io::Result<()> {
    remove_edges_for_node(node_id)
}
