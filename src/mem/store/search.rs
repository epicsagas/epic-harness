//! search.rs — FTS search and dynamic filter queries

use std::io;

use rusqlite::{Connection, params};
use sqlx::SqlitePool;

use super::node::row_to_node_pool;
use super::types::Node;
use super::util::{NODE_COLUMNS, NODE_COLUMNS_PREFIXED};

pub fn search_nodes(query: &str, limit: usize) -> Vec<Node> {
    let conn = match super::open_db() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    search_nodes_conn(&conn, query, limit).unwrap_or_else(|_| vec![])
}

pub fn search_nodes_conn(conn: &Connection, query: &str, limit: usize) -> io::Result<Vec<Node>> {
    let sql = format!(
        "SELECT n.{NODE_COLUMNS_PREFIXED}
         FROM nodes n
         JOIN nodes_fts ON n.rowid = nodes_fts.rowid
         WHERE nodes_fts MATCH ?1
         ORDER BY n.importance DESC
         LIMIT ?2"
    );
    let mut stmt = conn.prepare(&sql).map_err(io::Error::other)?;
    let nodes: Vec<Node> = stmt
        .query_map(params![query, limit as i64], super::util::row_to_node)
        .map_err(io::Error::other)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(nodes)
}

/// Dynamic filter query using an existing connection.
pub fn query_nodes_conn(
    conn: &Connection,
    tag: Option<&str>,
    node_type: Option<&str>,
    project: Option<&str>,
    limit: usize,
) -> io::Result<Vec<Node>> {
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

    let mut stmt = conn.prepare(&sql).map_err(io::Error::other)?;
    let refs: Vec<&dyn rusqlite::ToSql> = param_vals.iter().map(|b| b.as_ref()).collect();
    let nodes: Vec<Node> = stmt
        .query_map(refs.as_slice(), super::util::row_to_node)
        .map_err(io::Error::other)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(nodes)
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
    query_nodes_conn(&conn, tag, node_type, project, limit).unwrap_or_else(|_| vec![])
}

// ── Async pool functions ─────────────────────────────

pub async fn search_nodes_pool(pool: &SqlitePool, query: &str, limit: i64) -> io::Result<Vec<Node>> {
    let sql = format!(
        "SELECT n.{NODE_COLUMNS_PREFIXED}
         FROM nodes n
         JOIN nodes_fts ON n.rowid = nodes_fts.rowid
         WHERE nodes_fts MATCH ?
         ORDER BY n.importance DESC
         LIMIT ?"
    );
    let rows = sqlx::query(&sql)
        .bind(query)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;
    rows.iter().map(|r| row_to_node_pool(r)).collect()
}

/// Async dynamic filter query using QueryBuilder.
pub async fn query_nodes_pool(
    pool: &SqlitePool,
    tag: Option<&str>,
    node_type: Option<&str>,
    project: Option<&str>,
    limit: usize,
) -> io::Result<Vec<Node>> {
    let limit = limit.min(200) as i64;

    let mut qb = sqlx::QueryBuilder::new(format!("SELECT {NODE_COLUMNS} FROM nodes WHERE 1=1"));

    if let Some(t) = tag {
        qb.push(" AND (',' || tags || ',' LIKE '%,' || ");
        qb.push_bind(t);
        qb.push(" || ',%')");
    }
    if let Some(nt) = node_type {
        qb.push(" AND type = ");
        qb.push_bind(nt);
    }
    if let Some(p) = project {
        qb.push(" AND (',' || projects || ',' LIKE '%,' || ");
        qb.push_bind(p);
        qb.push(" || ',%')");
    }

    qb.push(" ORDER BY updated DESC LIMIT ");
    qb.push_bind(limit);

    let rows = qb
        .build()
        .fetch_all(pool)
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;
    rows.iter().map(|r| row_to_node_pool(r)).collect()
}
