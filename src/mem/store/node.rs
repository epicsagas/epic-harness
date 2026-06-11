//! node.rs — Node CRUD operations via sqlx

use std::io;

use sqlx::Row;

use super::conn::memory_pool_sync;
use super::schema::upsert_graph_node;
use super::types::{Node, NodeFrontmatter, graph_to_node, node_to_graph};
use super::util::{NODE_COLUMNS, row_to_graph_node};
use crate::store::runtime;

pub fn write_node(node: &Node) -> io::Result<()> {
    let pool = memory_pool_sync()?;
    runtime::block_on(write_node_async(&pool, node))
}

/// Async core — shared by sync wrapper and pool variant.
async fn write_node_async(pool: &sqlx::AnyPool, node: &Node) -> io::Result<()> {
    // Preserve monotonically-increasing fields from existing node
    let mut gn = node_to_graph(node.clone());
    let existing: Option<super::types::GraphNode> = sqlx::query(&format!(
        "SELECT {NODE_COLUMNS} FROM nodes WHERE id = ?"
    ))
        .bind(&gn.id)
        .fetch_optional(pool)
        .await
        .map_err(io::Error::other)?
        .map(|r| row_to_graph_node(&r));

    if let Some(ex) = existing {
        gn.access_count = gn.access_count.max(ex.access_count);
        if ex.accessed_at > gn.accessed_at {
            gn.accessed_at = ex.accessed_at;
        }
    }

    upsert_graph_node(pool, &gn).await
}

pub fn read_node(id: &str) -> io::Result<Node> {
    let pool = memory_pool_sync()?;
    runtime::block_on(read_node_async(&pool, id))
}

async fn read_node_async(pool: &sqlx::AnyPool, id: &str) -> io::Result<Node> {
    sqlx::query(&format!("SELECT {NODE_COLUMNS} FROM nodes WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(io::Error::other)?
        .map(|r| graph_to_node(row_to_graph_node(&r)))
        .ok_or_else(|| io::Error::other(format!("node not found: {id}")))
}

pub fn delete_node_file(id: &str) -> io::Result<()> {
    let pool = memory_pool_sync()?;
    runtime::block_on(async {
        sqlx::query("DELETE FROM nodes WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .map_err(io::Error::other)?;
        Ok(())
    })
}

pub fn list_node_ids() -> io::Result<Vec<String>> {
    let pool = memory_pool_sync()?;
    runtime::block_on(async {
        sqlx::query("SELECT id FROM nodes ORDER BY updated DESC")
            .fetch_all(&pool)
            .await
            .map_err(io::Error::other)?
            .iter()
            .map(|r| r.try_get::<String, _>(0).map_err(io::Error::other))
            .collect()
    })
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

// ── Pool-compatible async wrappers ────────────────────────────

pub async fn write_node_pool(pool: &sqlx::AnyPool, node: &Node) -> io::Result<()> {
    write_node_async(pool, node).await
}

pub async fn read_node_pool(pool: &sqlx::AnyPool, id: &str) -> io::Result<Node> {
    read_node_async(pool, id).await
}

pub async fn read_nodes_pool(pool: &sqlx::AnyPool, ids: &[&str]) -> io::Result<Vec<Node>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    // Build parameterized IN clause
    let placeholders: Vec<&str> = ids.iter().map(|_| "?").collect();
    let sql = format!(
        "SELECT {NODE_COLUMNS} FROM nodes WHERE id IN ({})",
        placeholders.join(",")
    );
    let mut query = sqlx::query(&sql);
    for id in ids {
        query = query.bind(*id);
    }
    query
        .fetch_all(pool)
        .await
        .map_err(io::Error::other)?
        .iter()
        .map(|r| Ok(graph_to_node(row_to_graph_node(r))))
        .collect()
}

#[allow(dead_code)]
pub async fn delete_node_pool(pool: &sqlx::AnyPool, id: &str) -> io::Result<()> {
    sqlx::query("DELETE FROM nodes WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(io::Error::other)?;
    Ok(())
}

#[allow(dead_code)]
pub async fn node_exists_pool(pool: &sqlx::AnyPool, id: &str) -> bool {
    read_node_async(pool, id).await.is_ok()
}

#[allow(dead_code)]
pub async fn read_all_nodes_pool(pool: &sqlx::AnyPool) -> io::Result<Vec<Node>> {
    sqlx::query(&format!("SELECT {NODE_COLUMNS} FROM nodes ORDER BY updated DESC LIMIT 1000000"))
        .fetch_all(pool)
        .await
        .map_err(io::Error::other)?
        .iter()
        .map(|r| Ok(graph_to_node(row_to_graph_node(r))))
        .collect()
}

pub async fn read_nodes_limited_pool(pool: &sqlx::AnyPool, limit: i64) -> io::Result<Vec<Node>> {
    sqlx::query(&format!("SELECT {NODE_COLUMNS} FROM nodes ORDER BY updated DESC LIMIT ?"))
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(io::Error::other)?
        .iter()
        .map(|r| Ok(graph_to_node(row_to_graph_node(r))))
        .collect()
}

#[allow(dead_code)]
pub async fn list_node_ids_pool(pool: &sqlx::AnyPool) -> io::Result<Vec<String>> {
    sqlx::query("SELECT id FROM nodes ORDER BY updated DESC")
        .fetch_all(pool)
        .await
        .map_err(io::Error::other)?
        .iter()
        .map(|r| r.try_get::<String, _>(0).map_err(io::Error::other))
        .collect()
}
