//! search.rs — FTS search and dynamic filter queries

use rusqlite::{Connection, params};

use super::types::Node;
use super::util::{NODE_COLUMNS, NODE_COLUMNS_PREFIXED};

pub fn search_nodes(query: &str, limit: usize) -> Vec<Node> {
    let conn = match super::open_db() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    search_nodes_conn(&conn, query, limit)
}

pub fn search_nodes_conn(conn: &Connection, query: &str, limit: usize) -> Vec<Node> {
    let sql = format!(
        "SELECT n.{NODE_COLUMNS_PREFIXED}
         FROM nodes n
         JOIN nodes_fts ON n.rowid = nodes_fts.rowid
         WHERE nodes_fts MATCH ?1
         ORDER BY n.importance DESC
         LIMIT ?2"
    );
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map(params![query, limit as i64], super::util::row_to_node)
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

/// Dynamic filter query using an existing connection.
pub fn query_nodes_conn(
    conn: &Connection,
    tag: Option<&str>,
    node_type: Option<&str>,
    project: Option<&str>,
    limit: usize,
) -> Vec<Node> {
    // Cap limit to prevent oversized result sets.
    let limit = limit.min(200);

    // Build parameterized conditions — no format!() interpolation of user values.
    let mut condition_strs: Vec<&str> = vec![];
    let mut param_vals: Vec<Box<dyn rusqlite::ToSql>> = vec![];

    if let Some(t) = tag {
        condition_strs.push("(',' || tags || ',' LIKE '%,' || ? || ',%')");
        param_vals.push(Box::new(t.to_string()));
    }
    if let Some(nt) = node_type {
        condition_strs.push("type = ?");
        param_vals.push(Box::new(nt.to_string()));
    }
    if let Some(p) = project {
        condition_strs.push("(',' || projects || ',' LIKE '%,' || ? || ',%')");
        param_vals.push(Box::new(p.to_string()));
    }

    let where_clause = if condition_strs.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", condition_strs.join(" AND "))
    };

    // `limit` is not user-controlled; safe to format as i64.
    let sql = format!(
        "SELECT {NODE_COLUMNS}
         FROM nodes {where_clause} ORDER BY updated DESC LIMIT {}",
        limit as i64,
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let refs: Vec<&dyn rusqlite::ToSql> = param_vals.iter().map(|b| b.as_ref()).collect();
    stmt.query_map(refs.as_slice(), super::util::row_to_node)
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

/// Dynamic filter query.
pub fn query_nodes(
    tag: Option<&str>,
    node_type: Option<&str>,
    project: Option<&str>,
    limit: usize,
) -> Vec<Node> {
    let conn = match super::open_db() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    query_nodes_conn(&conn, tag, node_type, project, limit)
}
