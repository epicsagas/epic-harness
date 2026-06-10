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
    runtime::block_on(append_edge_pool_async(&pool, &ge))
}

async fn append_edge_pool_async(pool: &sqlx::AnyPool, ge: &super::types::GraphEdge) -> io::Result<()> {
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
}

pub fn delete_edge_by_id(edge_id: &str) -> io::Result<()> {
    let pool = memory_pool_sync()?;
    runtime::block_on(delete_edge_by_id_async(&pool, edge_id))
}

async fn delete_edge_by_id_async(pool: &sqlx::AnyPool, edge_id: &str) -> io::Result<()> {
    sqlx::query("DELETE FROM edges WHERE source = ? OR target = ? OR label = ?")
        .bind(edge_id)
        .bind(edge_id)
        .bind(edge_id)
        .execute(pool)
        .await
        .map_err(io::Error::other)?;
    Ok(())
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

#[allow(dead_code)]
pub async fn delete_edge_by_id_pool(pool: &sqlx::AnyPool, edge_id: &str) -> io::Result<()> {
    delete_edge_by_id_async(pool, edge_id).await
}

#[allow(dead_code)]
pub async fn remove_edges_for_node_pool(pool: &sqlx::AnyPool, node_id: &str) -> io::Result<()> {
    remove_edges_for_node_async(pool, node_id).await
}
