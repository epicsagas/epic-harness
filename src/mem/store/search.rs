//! search.rs — FTS search and dynamic filter queries via sqlx

use std::io;

use sqlx::Row as _;

use super::conn::memory_pool_sync;
use super::types::Node;
use super::util::{NODE_COLUMNS, row_to_node};
use crate::store::runtime;

pub fn search_nodes(query: &str, limit: usize) -> Vec<Node> {
    search_nodes_filtered(query, limit, None)
}

/// Search literal memory terms, restricting node type before the result limit.
/// Existing callers can keep using [`search_nodes`] for unfiltered results.
pub fn search_nodes_filtered(query: &str, limit: usize, node_type: Option<&str>) -> Vec<Node> {
    let pool = match memory_pool_sync() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    runtime::block_on(search_nodes_filtered_async(&pool, query, limit, node_type))
}

async fn search_nodes_filtered_async(
    pool: &sqlx::AnyPool,
    query: &str,
    limit: usize,
    node_type: Option<&str>,
) -> Vec<Node> {
    let fts_query = escape_fts(query);
    // Step 1: get matching rowids from FTS index
    let sql = if node_type.is_some() {
        "SELECT rowid FROM nodes_fts WHERE nodes_fts MATCH ? AND rowid IN (SELECT rowid FROM nodes WHERE type = ?) ORDER BY rank LIMIT ?"
    } else {
        "SELECT rowid FROM nodes_fts WHERE nodes_fts MATCH ? ORDER BY rank LIMIT ?"
    };
    let mut query = sqlx::query(sql).bind(&fts_query);
    if let Some(node_type) = node_type {
        query = query.bind(node_type);
    }
    let rowids: Vec<i64> = match query.bind(limit as i64).fetch_all(pool).await {
        Ok(rows) => rows
            .iter()
            .filter_map(|r| r.try_get::<i64, _>(0).ok())
            .collect(),
        Err(_) => return vec![],
    };

    if rowids.is_empty() {
        return vec![];
    }

    // Step 2: fetch full nodes by rowid
    let placeholders = rowids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("SELECT {NODE_COLUMNS} FROM nodes WHERE rowid IN ({placeholders})");
    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
    for rid in &rowids {
        q = q.bind(*rid);
    }
    q.fetch_all(pool)
        .await
        .map(|rows| rows.iter().map(row_to_node).collect())
        .unwrap_or_default()
}

pub fn query_nodes(
    tag: Option<&str>,
    node_type: Option<&str>,
    project: Option<&str>,
    limit: usize,
) -> Vec<Node> {
    let pool = match memory_pool_sync() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    runtime::block_on(query_nodes_async(&pool, tag, node_type, project, limit))
}

async fn query_nodes_async(
    pool: &sqlx::AnyPool,
    tag: Option<&str>,
    node_type: Option<&str>,
    project: Option<&str>,
    limit: usize,
) -> Vec<Node> {
    let mut sql = format!("SELECT {NODE_COLUMNS} FROM nodes WHERE 1=1");

    if tag.is_some() {
        sql.push_str(" AND tags LIKE ?");
    }
    if node_type.is_some() {
        sql.push_str(" AND type = ?");
    }
    if project.is_some() {
        sql.push_str(" AND projects LIKE ?");
    }
    // Cap limit at 200
    let safe_limit = (limit.min(200)) as i64;
    sql.push_str(" ORDER BY updated DESC LIMIT ?");

    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    if let Some(t) = tag {
        query = query.bind(format!("%{t}%"));
    }
    if let Some(t) = node_type {
        query = query.bind(t);
    }
    if let Some(p) = project {
        query = query.bind(format!("%{p}%"));
    }
    query = query.bind(safe_limit);

    query
        .fetch_all(pool)
        .await
        .map(|rows| rows.iter().map(row_to_node).collect())
        .unwrap_or_default()
}

/// Escape special FTS5 characters in a search query.
fn escape_fts(s: &str) -> String {
    let sanitized: String = s
        .replace(['"', '*'], "")
        .replace(':', " ")
        .replace(['^', '{', '}', '(', ')'], "")
        .trim()
        .to_string();

    if sanitized.is_empty() {
        return "*".to_string();
    }

    let words: Vec<&str> = sanitized.split_whitespace().take(5).collect();
    if words.is_empty() {
        return "*".to_string();
    }
    words
        .iter()
        .map(|w| format!("\"{}\"", w))
        .collect::<Vec<_>>()
        .join(" OR ")
}

// ── Pool-compatible wrappers ─────────────────────────────────

pub async fn search_nodes_pool(
    pool: &sqlx::AnyPool,
    query: &str,
    limit: i64,
) -> io::Result<Vec<Node>> {
    search_nodes_filtered_pool(pool, query, limit, None).await
}

/// Search on an explicit memory pool, filtering type before the result limit.
pub async fn search_nodes_filtered_pool(
    pool: &sqlx::AnyPool,
    query: &str,
    limit: i64,
    node_type: Option<&str>,
) -> io::Result<Vec<Node>> {
    Ok(search_nodes_filtered_async(pool, query, limit.max(0) as usize, node_type).await)
}

pub async fn query_nodes_pool(
    pool: &sqlx::AnyPool,
    tag: Option<&str>,
    node_type: Option<&str>,
    project: Option<&str>,
    limit: usize,
) -> io::Result<Vec<Node>> {
    Ok(query_nodes_async(pool, tag, node_type, project, limit).await)
}
