//! node.rs — Node CRUD operations

use rusqlite::{Connection, params};
use std::io;

use super::types::Node;
use super::util::{NODE_COLUMNS, join_csv};

pub fn write_node(node: &Node) -> io::Result<()> {
    let conn = super::open_db()?;
    write_node_conn(&conn, node)
}

pub fn write_node_conn(conn: &Connection, node: &Node) -> io::Result<()> {
    let fm = &node.frontmatter;
    conn.execute(
        "INSERT OR REPLACE INTO nodes (id, type, title, tags, projects, agents, created, updated, body, importance, access_count, accessed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            fm.id,
            fm.node_type,
            fm.title,
            join_csv(&fm.tags),
            join_csv(&fm.projects),
            join_csv(&fm.agents),
            fm.created,
            fm.updated,
            node.body,
            fm.importance,
            fm.access_count,
            fm.accessed_at,
        ],
    )
    .map_err(io::Error::other)?;
    Ok(())
}

pub fn read_node(id: &str) -> io::Result<Node> {
    let conn = super::open_db()?;
    read_node_conn(&conn, id)
}

/// Batch-read multiple nodes by ID in a single `WHERE id IN (...)` query.
pub fn read_nodes_conn(conn: &Connection, ids: &[&str]) -> io::Result<Vec<Node>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let ph = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("SELECT {NODE_COLUMNS} FROM nodes WHERE id IN ({ph})");
    let mut stmt = conn.prepare(&sql).map_err(io::Error::other)?;
    let nodes: Vec<Node> = stmt
        .query_map(
            rusqlite::params_from_iter(ids.iter()),
            super::util::row_to_node,
        )
        .map_err(io::Error::other)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(nodes)
}

pub fn read_node_conn(conn: &Connection, id: &str) -> io::Result<Node> {
    let sql = format!("SELECT {NODE_COLUMNS} FROM nodes WHERE id = ?1");
    conn.query_row(&sql, params![id], super::util::row_to_node)
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, format!("node not found: {id}")))
}

pub fn delete_node_file(id: &str) -> io::Result<()> {
    let conn = super::open_db()?;
    delete_node_file_conn(&conn, id)
}

/// Delete a node using an existing connection (for use with shared state).
/// Check if a node with the given ID exists.
#[allow(dead_code)]
pub fn node_exists_conn(conn: &Connection, id: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM nodes WHERE id = ?1)",
        params![id],
        |row| row.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

pub fn delete_node_file_conn(conn: &Connection, id: &str) -> io::Result<()> {
    conn.execute("DELETE FROM nodes WHERE id = ?1", params![id])
        .map_err(io::Error::other)?;
    Ok(())
}

/// Read all nodes ordered by updated DESC in a single query.
#[allow(dead_code)] // used by graphos-desktop (Tauri) through public API
pub fn read_all_nodes_conn(conn: &Connection) -> io::Result<Vec<Node>> {
    let sql = format!("SELECT {NODE_COLUMNS} FROM nodes ORDER BY updated DESC");
    let mut stmt = conn.prepare(&sql).map_err(io::Error::other)?;
    let nodes: Vec<Node> = stmt
        .query_map([], super::util::row_to_node)
        .map_err(io::Error::other)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(nodes)
}

/// Read nodes with a SQL-level LIMIT (avoids loading all rows into memory).
pub fn read_nodes_limited_conn(conn: &Connection, limit: usize) -> io::Result<Vec<Node>> {
    // format! is safe here — NODE_COLUMNS is a compile-time const, not user input.
    let sql = format!("SELECT {NODE_COLUMNS} FROM nodes ORDER BY updated DESC LIMIT ?");
    let mut stmt = conn.prepare(&sql).map_err(io::Error::other)?;
    let nodes: Vec<Node> = stmt
        .query_map(rusqlite::params![limit as i64], super::util::row_to_node)
        .map_err(io::Error::other)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(nodes)
}

pub fn list_node_ids() -> io::Result<Vec<String>> {
    let conn = super::open_db()?;
    list_node_ids_conn(&conn)
}

/// List all node IDs using an existing connection.
pub fn list_node_ids_conn(conn: &Connection) -> io::Result<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT id FROM nodes")
        .map_err(io::Error::other)?;
    let ids: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(io::Error::other)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(ids)
}

// ── Node serialization (kept for migrate import) ──────

pub fn serialize_node(node: &Node) -> String {
    let fm = serde_yaml::to_string(&node.frontmatter).unwrap_or_default();
    format!("---\n{}---\n{}", fm, node.body)
}

pub fn parse_node(content: &str) -> Option<Node> {
    use super::types::NodeFrontmatter;
    let content = content.strip_prefix("---\n").unwrap_or(content);
    let (fm_str, body) = content.split_once("\n---\n")?;
    let frontmatter: NodeFrontmatter = serde_yaml::from_str(fm_str).ok()?;
    Some(Node {
        frontmatter,
        body: body.to_string(),
    })
}
