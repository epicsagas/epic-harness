//! edge.rs — Edge CRUD operations via llm-kernel graph

use std::io;

use super::conn::memory_conn;
use super::types::{Edge, edge_to_graph, graph_to_edge};

pub fn append_edge(edge: &Edge) -> io::Result<()> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    llm_kernel::graph::store::append_edge(&guard, &edge_to_graph(edge.clone()))
        .map_err(|e| io::Error::other(e.to_string()))
}

#[allow(dead_code)]
pub fn read_edges() -> Vec<Edge> {
    read_edges_limit(5000)
}

pub fn read_edges_limit(limit: usize) -> Vec<Edge> {
    let conn = match memory_conn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[mem/store] read_edges: {e}");
            return vec![];
        }
    };
    let guard = match conn.lock() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("[mem/store] read_edges: {e}");
            return vec![];
        }
    };
    llm_kernel::graph::store::read_edges(&guard, limit)
        .map(|edges| edges.into_iter().map(graph_to_edge).collect())
        .unwrap_or_else(|_| vec![])
}

pub fn delete_edge_by_id(edge_id: &str) -> io::Result<()> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    llm_kernel::graph::store::delete_edge(&guard, edge_id)
        .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}

pub fn remove_edges_for_node(node_id: &str) -> io::Result<()> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    llm_kernel::graph::store::remove_edges_for_node(&guard, node_id)
        .map_err(|e| io::Error::other(e.to_string()))
}

// ── Pool-compatible wrappers ─────────────────────────────

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
