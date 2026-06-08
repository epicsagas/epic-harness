//! node.rs — Node CRUD operations via llm-kernel graph

use std::io;

use super::conn::memory_conn;
use super::types::{Node, NodeFrontmatter, graph_to_node, node_to_graph};

pub fn write_node(node: &Node) -> io::Result<()> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;

    // Preserve monotonically-increasing fields from existing node
    let mut gn = node_to_graph(node.clone());
    if let Ok(Some(existing)) = llm_kernel::graph::store::read_node(&guard, &gn.id) {
        gn.access_count = gn.access_count.max(existing.access_count);
        if existing.accessed_at > gn.accessed_at {
            gn.accessed_at = existing.accessed_at;
        }
    }

    llm_kernel::graph::store::upsert_node(&guard, &gn).map_err(|e| io::Error::other(e.to_string()))
}

pub fn read_node(id: &str) -> io::Result<Node> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    llm_kernel::graph::store::read_node(&guard, id)
        .map_err(|e| io::Error::other(e.to_string()))?
        .map(graph_to_node)
        .ok_or_else(|| io::Error::other(format!("node not found: {id}")))
}

pub fn delete_node_file(id: &str) -> io::Result<()> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    llm_kernel::graph::store::delete_node(&guard, id)
        .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}

pub fn list_node_ids() -> io::Result<Vec<String>> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    llm_kernel::graph::store::list_node_ids(&guard).map_err(|e| io::Error::other(e.to_string()))
}

// ── Node serialization (kept for legacy migration import) ──────

pub fn serialize_node(node: &Node) -> String {
    let fm = serde_yaml::to_string(&node.frontmatter).unwrap_or_default();
    format!("---\n{}---\n{}", fm, node.body)
}

pub fn parse_node(content: &str) -> Option<Node> {
    let content = content.strip_prefix("---\n").unwrap_or(content);
    let (fm_str, body) = content.split_once("\n---\n")?;
    let frontmatter: NodeFrontmatter = serde_yaml::from_str(fm_str).ok()?;
    Some(Node {
        frontmatter,
        body: body.to_string(),
    })
}

// ── Pool-compatible wrappers (keep signatures for callers) ─────

pub async fn write_node_pool(_pool: &sqlx::AnyPool, node: &Node) -> io::Result<()> {
    write_node(node)
}

pub async fn read_node_pool(_pool: &sqlx::AnyPool, id: &str) -> io::Result<Node> {
    read_node(id)
}

pub async fn read_nodes_pool(_pool: &sqlx::AnyPool, ids: &[&str]) -> io::Result<Vec<Node>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    llm_kernel::graph::store::read_nodes(&guard, ids)
        .map(|nodes| nodes.into_iter().map(graph_to_node).collect())
        .map_err(|e| io::Error::other(e.to_string()))
}

#[allow(dead_code)]
pub async fn delete_node_pool(_pool: &sqlx::AnyPool, id: &str) -> io::Result<()> {
    delete_node_file(id)
}

#[allow(dead_code)]
pub async fn node_exists_pool(_pool: &sqlx::AnyPool, id: &str) -> bool {
    read_node(id).is_ok()
}

#[allow(dead_code)]
pub async fn read_all_nodes_pool(_pool: &sqlx::AnyPool) -> io::Result<Vec<Node>> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    // Read all by using a very large limit
    llm_kernel::graph::store::read_nodes_limited(&guard, 1_000_000)
        .map(|nodes| nodes.into_iter().map(graph_to_node).collect())
        .map_err(|e| io::Error::other(e.to_string()))
}

pub async fn read_nodes_limited_pool(_pool: &sqlx::AnyPool, limit: i64) -> io::Result<Vec<Node>> {
    let conn = memory_conn()?;
    let guard = conn.lock().map_err(|e| io::Error::other(e.to_string()))?;
    llm_kernel::graph::store::read_nodes_limited(&guard, limit as usize)
        .map(|nodes| nodes.into_iter().map(graph_to_node).collect())
        .map_err(|e| io::Error::other(e.to_string()))
}

#[allow(dead_code)]
pub async fn list_node_ids_pool(_pool: &sqlx::AnyPool) -> io::Result<Vec<String>> {
    list_node_ids()
}
