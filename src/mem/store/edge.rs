//! edge.rs — Edge CRUD operations

use rusqlite::{Connection, params};
use sqlx::{Row, SqlitePool};
use std::io;

use super::types::Edge;

pub fn append_edge(edge: &Edge) -> io::Result<()> {
    let conn = super::open_db()?;
    conn.execute(
        "INSERT OR IGNORE INTO edges (id, source, target, relation, weight, ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            edge.id,
            edge.source,
            edge.target,
            edge.relation,
            edge.weight,
            edge.ts
        ],
    )
    .map_err(io::Error::other)?;
    Ok(())
}

/// Append an edge using an existing connection (for batch/transaction use).
pub fn append_edge_conn(conn: &Connection, edge: &Edge) -> io::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO edges (id, source, target, relation, weight, ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            edge.id,
            edge.source,
            edge.target,
            edge.relation,
            edge.weight,
            edge.ts
        ],
    )
    .map_err(io::Error::other)?;
    Ok(())
}

/// Read edges using an existing connection, capped at `limit`.
pub fn read_edges_conn(conn: &Connection, limit: usize) -> io::Result<Vec<Edge>> {
    let mut stmt = conn
        .prepare("SELECT id, source, target, relation, weight, ts FROM edges LIMIT ?1")
        .map_err(io::Error::other)?;
    let edges: Vec<Edge> = stmt
        .query_map(params![limit as i64], |row| {
            Ok(Edge {
                id: row.get(0)?,
                source: row.get(1)?,
                target: row.get(2)?,
                relation: row.get(3)?,
                weight: row.get(4)?,
                ts: row.get(5)?,
            })
        })
        .map_err(io::Error::other)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(edges)
}

#[allow(dead_code)] // used by integration tests (tests/mem_test.rs)
pub fn read_edges() -> Vec<Edge> {
    read_edges_limit(5000)
}

pub fn read_edges_limit(limit: usize) -> Vec<Edge> {
    match super::open_db() {
        Ok(conn) => read_edges_conn(&conn, limit).unwrap_or_else(|e| {
            eprintln!("[mem/store] read_edges: query failed: {e}");
            vec![]
        }),
        Err(e) => {
            eprintln!("[mem/store] read_edges: open_db failed: {e}");
            vec![]
        }
    }
}

pub fn delete_edge_by_id(edge_id: &str) -> io::Result<()> {
    let conn = super::open_db()?;
    delete_edge_by_id_conn(&conn, edge_id)
}

/// Delete an edge using an existing connection (for use with shared state).
pub fn delete_edge_by_id_conn(conn: &Connection, edge_id: &str) -> io::Result<()> {
    conn.execute("DELETE FROM edges WHERE id = ?1", params![edge_id])
        .map_err(io::Error::other)?;
    Ok(())
}

pub fn remove_edges_for_node(node_id: &str) -> io::Result<()> {
    let conn = super::open_db()?;
    remove_edges_for_node_conn(&conn, node_id)
}

/// Remove edges for a node using an existing connection (for use with shared state).
pub fn remove_edges_for_node_conn(conn: &Connection, node_id: &str) -> io::Result<()> {
    conn.execute(
        "DELETE FROM edges WHERE source = ?1 OR target = ?1",
        params![node_id],
    )
    .map_err(io::Error::other)?;
    Ok(())
}

// ── Async pool functions ─────────────────────────────

// TODO: Wire up when remaining sync callers migrate to pool (R4).
#[allow(dead_code)]
pub async fn append_edge_pool(pool: &SqlitePool, edge: &Edge) -> io::Result<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO edges (id, source, target, relation, weight, ts)
         VALUES (?, ?, ?, ?, ?, ?)",
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

// TODO: Wire up when remaining sync callers migrate to pool (R4).
#[allow(dead_code)]
pub async fn read_edges_pool(pool: &SqlitePool, limit: i64) -> io::Result<Vec<Edge>> {
    let rows = sqlx::query("SELECT id, source, target, relation, weight, ts FROM edges LIMIT ?")
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

// TODO: Wire up when remaining sync callers migrate to pool (R4).
#[allow(dead_code)]
pub async fn delete_edge_by_id_pool(pool: &SqlitePool, edge_id: &str) -> io::Result<()> {
    sqlx::query("DELETE FROM edges WHERE id = ?")
        .bind(edge_id)
        .execute(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    Ok(())
}

// TODO: Wire up when remaining sync callers migrate to pool (R4).
#[allow(dead_code)]
pub async fn remove_edges_for_node_pool(pool: &SqlitePool, node_id: &str) -> io::Result<()> {
    sqlx::query("DELETE FROM edges WHERE source = ? OR target = ?")
        .bind(node_id)
        .bind(node_id)
        .execute(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    Ok(())
}
