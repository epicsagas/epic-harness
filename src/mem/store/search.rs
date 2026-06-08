//! search.rs — FTS search and dynamic filter queries via llm-kernel

use std::io;

use super::conn::memory_conn;
use super::types::{Node, graph_to_node};

pub fn search_nodes(query: &str, limit: usize) -> Vec<Node> {
    let conn = match memory_conn() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let guard = match conn.lock() {
        Ok(g) => g,
        Err(_) => return vec![],
    };
    llm_kernel::graph::search::search_nodes(&guard, query, limit)
        .map(|nodes| nodes.into_iter().map(graph_to_node).collect())
        .unwrap_or_else(|_| vec![])
}

pub fn query_nodes(
    tag: Option<&str>,
    node_type: Option<&str>,
    project: Option<&str>,
    limit: usize,
) -> Vec<Node> {
    let conn = match memory_conn() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let guard = match conn.lock() {
        Ok(g) => g,
        Err(_) => return vec![],
    };
    llm_kernel::graph::search::query_nodes(&guard, tag, node_type, project, limit)
        .map(|nodes| nodes.into_iter().map(graph_to_node).collect())
        .unwrap_or_else(|_| vec![])
}

// ── Pool-compatible wrappers ─────────────────────────────

pub async fn search_nodes_pool(
    _pool: &sqlx::AnyPool,
    query: &str,
    limit: i64,
) -> io::Result<Vec<Node>> {
    Ok(search_nodes(query, limit as usize))
}

pub async fn query_nodes_pool(
    _pool: &sqlx::AnyPool,
    tag: Option<&str>,
    node_type: Option<&str>,
    project: Option<&str>,
    limit: usize,
) -> io::Result<Vec<Node>> {
    Ok(query_nodes(tag, node_type, project, limit))
}
