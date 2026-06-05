//! edge.rs — Edge CRUD operations

use sqlx::{Row, AnyPool};
use std::io;

use super::types::Edge;

pub fn append_edge(edge: &Edge) -> io::Result<()> {
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::memory_pool().await?;
        append_edge_pool(&pool, edge).await
    })
}

#[allow(dead_code)] // used by integration tests (tests/mem_test.rs)
pub fn read_edges() -> Vec<Edge> {
    read_edges_limit(5000)
}

pub fn read_edges_limit(limit: usize) -> Vec<Edge> {
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::memory_pool().await?;
        read_edges_pool(&pool, limit as i64).await
    })
    .unwrap_or_else(|e| {
        eprintln!("[mem/store] read_edges: {e}");
        vec![]
    })
}

pub fn delete_edge_by_id(edge_id: &str) -> io::Result<()> {
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::memory_pool().await?;
        delete_edge_by_id_pool(&pool, edge_id).await
    })
}

pub fn remove_edges_for_node(node_id: &str) -> io::Result<()> {
    crate::store::runtime::block_on(async {
        let pool = crate::store::pool::memory_pool().await?;
        remove_edges_for_node_pool(&pool, node_id).await
    })
}

// ── Async pool functions ─────────────────────────────

pub async fn append_edge_pool(pool: &AnyPool, edge: &Edge) -> io::Result<()> {
    sqlx::query(
        "INSERT INTO edges (id, source, target, relation, weight, ts)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(&edge.id)
    .bind(&edge.source)
    .bind(&edge.target)
    .bind(&edge.relation)
    .bind(edge.weight)
    .bind(&edge.ts)
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;
    Ok(())
}

pub async fn read_edges_pool(pool: &AnyPool, limit: i64) -> io::Result<Vec<Edge>> {
    let rows = sqlx::query("SELECT id, source, target, relation, weight, ts FROM edges LIMIT $1")
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    rows.iter()
        .map(|r| {
            Ok(Edge {
                id: r.try_get(0).map_err(crate::store::sqlx_err)?,
                source: r.try_get(1).map_err(crate::store::sqlx_err)?,
                target: r.try_get(2).map_err(crate::store::sqlx_err)?,
                relation: r.try_get(3).map_err(crate::store::sqlx_err)?,
                weight: r.try_get(4).map_err(crate::store::sqlx_err)?,
                ts: r.try_get(5).map_err(crate::store::sqlx_err)?,
            })
        })
        .collect()
}

pub async fn delete_edge_by_id_pool(pool: &AnyPool, edge_id: &str) -> io::Result<()> {
    sqlx::query("DELETE FROM edges WHERE id = $1")
        .bind(edge_id)
        .execute(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    Ok(())
}

pub async fn remove_edges_for_node_pool(pool: &AnyPool, node_id: &str) -> io::Result<()> {
    sqlx::query("DELETE FROM edges WHERE source = $1 OR target = $2")
        .bind(node_id)
        .bind(node_id)
        .execute(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    Ok(())
}
