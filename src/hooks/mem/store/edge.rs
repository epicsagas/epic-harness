//! edge.rs — Edge CRUD operations

use rusqlite::{Connection, params};
use std::io;

use super::types::Edge;

pub fn append_edge(edge: &Edge) -> io::Result<()> {
    let conn = super::open_db()?;
    conn.execute(
        "INSERT OR IGNORE INTO edges (id, source, target, relation, weight, ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![edge.id, edge.source, edge.target, edge.relation, edge.weight, edge.ts],
    )
    .map_err(io::Error::other)?;
    Ok(())
}

/// Append an edge using an existing connection (for batch/transaction use).
pub fn append_edge_conn(conn: &Connection, edge: &Edge) -> io::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO edges (id, source, target, relation, weight, ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![edge.id, edge.source, edge.target, edge.relation, edge.weight, edge.ts],
    )
    .map_err(io::Error::other)?;
    Ok(())
}

/// Read all edges using an existing connection (no LIMIT — callers should paginate for large graphs).
pub fn read_edges_conn(conn: &Connection) -> Vec<Edge> {
    let mut stmt = match conn.prepare(
        "SELECT id, source, target, relation, weight, ts FROM edges",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[mem/store] read_edges_conn: prepare failed: {e}");
            return vec![];
        }
    };
    stmt.query_map([], |row| {
        Ok(Edge {
            id: row.get(0)?,
            source: row.get(1)?,
            target: row.get(2)?,
            relation: row.get(3)?,
            weight: row.get(4)?,
            ts: row.get(5)?,
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

#[allow(dead_code)] // used by integration tests (tests/mem_test.rs)
pub fn read_edges() -> Vec<Edge> {
    match super::open_db() {
        Ok(conn) => read_edges_conn(&conn),
        Err(e) => {
            eprintln!("[mem/store] read_edges: open_db failed: {e}");
            vec![]
        }
    }
}

pub fn delete_edge_by_id(edge_id: &str) -> io::Result<()> {
    let conn = super::open_db()?;
    conn.execute("DELETE FROM edges WHERE id = ?1", params![edge_id])
        .map_err(io::Error::other)?;
    Ok(())
}

pub fn remove_edges_for_node(node_id: &str) -> io::Result<()> {
    let conn = super::open_db()?;
    conn.execute(
        "DELETE FROM edges WHERE source = ?1 OR target = ?1",
        params![node_id],
    )
    .map_err(io::Error::other)?;
    Ok(())
}
