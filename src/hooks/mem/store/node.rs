//! node.rs — Node CRUD operations

use rusqlite::{Connection, params};
use std::io;

use super::types::Node;
use super::util::{join_csv, NODE_COLUMNS};

pub fn write_node(node: &Node) -> io::Result<()> {
    let conn = super::open_db()?;
    write_node_conn(&conn, node)
}

pub(crate) fn write_node_conn(conn: &Connection, node: &Node) -> io::Result<()> {
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
pub fn read_nodes_conn(conn: &Connection, ids: &[&str]) -> Vec<Node> {
    if ids.is_empty() {
        return vec![];
    }
    let ph = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("SELECT {NODE_COLUMNS} FROM nodes WHERE id IN ({ph})");
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map(rusqlite::params_from_iter(ids.iter()), super::util::row_to_node)
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

pub fn read_node_conn(conn: &Connection, id: &str) -> io::Result<Node> {
    let sql = format!("SELECT {NODE_COLUMNS} FROM nodes WHERE id = ?1");
    conn.query_row(&sql, params![id], super::util::row_to_node)
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, format!("node not found: {id}")))
}

pub fn delete_node_file(id: &str) -> io::Result<()> {
    let conn = super::open_db()?;
    conn.execute("DELETE FROM nodes WHERE id = ?1", params![id])
        .map_err(io::Error::other)?;
    Ok(())
}

pub fn list_node_ids() -> io::Result<Vec<String>> {
    let conn = super::open_db()?;
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
