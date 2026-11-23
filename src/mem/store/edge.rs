//! edge.rs — Edge CRUD operations via sqlx

use std::io;

use sqlx::Row;

use super::conn::memory_pool_sync;
use super::schema::append_graph_edge;
use super::types::Edge;
use super::types::edge_to_graph;
use crate::store::runtime;

/// Longest relation (edge label) the HTTP API stores or accepts for deletion.
pub const MAX_RELATION_CHARS: usize = 64;

/// Truncate to `MAX_RELATION_CHARS` characters; a missing or blank result becomes "related".
pub fn normalize_relation(raw: Option<&str>) -> String {
    let r: String = raw.unwrap_or("").chars().take(MAX_RELATION_CHARS).collect();
    if r.trim().is_empty() {
        "related".to_string()
    } else {
        r
    }
}

pub fn append_edge(edge: &Edge) -> io::Result<()> {
    let pool = memory_pool_sync()?;
    let ge = edge_to_graph(edge);
    runtime::block_on(append_edge_pool_async(&pool, &ge))
}

async fn append_edge_pool_async(
    pool: &sqlx::AnyPool,
    ge: &super::types::GraphEdge,
) -> io::Result<()> {
    append_graph_edge(pool, ge).await
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
    runtime::block_on(read_edges_pool_async(&pool, limit))
}

async fn read_edges_pool_async(pool: &sqlx::AnyPool, limit: usize) -> Vec<Edge> {
    sqlx::query("SELECT source, target, label, created FROM edges ORDER BY created DESC LIMIT ?")
        .bind(limit as i64)
        .fetch_all(pool)
        .await
        .map(|rows| {
            rows.iter()
                .map(|r| Edge {
                    id: String::new(), // not persisted, edge identity is (source, target, label)
                    source: r.get::<String, _>(0),
                    target: r.get::<String, _>(1),
                    relation: r.get::<String, _>(2),
                    weight: 1.0, // not persisted
                    ts: r.get::<String, _>(3),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn remove_edges_for_node(node_id: &str) -> io::Result<()> {
    let pool = memory_pool_sync()?;
    runtime::block_on(remove_edges_for_node_async(&pool, node_id))
}

async fn remove_edges_for_node_async(pool: &sqlx::AnyPool, node_id: &str) -> io::Result<()> {
    sqlx::query("DELETE FROM edges WHERE source = ? OR target = ?")
        .bind(node_id)
        .bind(node_id)
        .execute(pool)
        .await
        .map_err(io::Error::other)?;
    Ok(())
}

// ── Pool-compatible async wrappers ────────────────────────────

pub async fn append_edge_pool(pool: &sqlx::AnyPool, edge: &Edge) -> io::Result<()> {
    let ge = edge_to_graph(edge);
    append_edge_pool_async(pool, &ge).await
}

#[allow(dead_code)]
pub async fn read_edges_pool(pool: &sqlx::AnyPool, limit: i64) -> io::Result<Vec<Edge>> {
    Ok(read_edges_pool_async(pool, limit as usize).await)
}

/// Delete the edge identified by its `(source, target, label)` primary key.
/// Returns the number of deleted rows (0 or 1).
pub async fn delete_edge_pool(
    pool: &sqlx::AnyPool,
    source: &str,
    target: &str,
    relation: &str,
) -> io::Result<u64> {
    sqlx::query("DELETE FROM edges WHERE source = ? AND target = ? AND label = ?")
        .bind(source)
        .bind(target)
        .bind(relation)
        .execute(pool)
        .await
        .map(|r| r.rows_affected())
        .map_err(io::Error::other)
}

#[allow(dead_code)]
pub async fn remove_edges_for_node_pool(pool: &sqlx::AnyPool, node_id: &str) -> io::Result<()> {
    remove_edges_for_node_async(pool, node_id).await
}
